//! Home stats and Insights (spec 4.6), computed from the `records` table with
//! no extra schema. Formulas are in ADR-0013 (`docs/DECISIONS.md`).
//!
//! * Only successful (`ok`) records count. Failed and running records never do.
//! * Words and speed come from dictations; commands are counted separately.
//! * Days are local calendar days at a fixed UTC offset (minutes east of UTC),
//!   the caller's current offset. A DST change moves at most an hour of records
//!   across a midnight, which is acceptable for a streak.

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::{local_date, Store, StoreError};

/// Typing speed the time-saved figure compares against (words per minute).
pub const TYPING_WPM: u64 = 40;
/// Milliseconds it takes to type one word at [`TYPING_WPM`].
pub const TYPING_MS_PER_WORD: u64 = 60_000 / TYPING_WPM;

const DAY_MS: i64 = 86_400_000;

/// Local day number (days since 1970-01-01 local) for `created_at`, floored so
/// it is right for negative values too. `?1` is the offset in milliseconds.
const DAY_EXPR: &str = "(((created_at + ?1) - (((created_at + ?1) % 86400000) + 86400000) % 86400000) / 86400000)";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeStats {
    /// Words in successful dictations.
    pub total_words: u64,
    /// Successful dictations.
    pub dictations: u64,
    /// Time spent speaking in those dictations.
    pub speaking_ms: u64,
    /// Typing time at 40 wpm minus speaking time, per dictation, never negative.
    pub time_saved_ms: u64,
    /// Words-weighted mean of the dictations' own WPM; 0 until there is one.
    pub average_wpm: u32,
    /// Consecutive local days with activity, ending today or yesterday.
    pub streak_days: u32,
    /// Whether today already counts toward the streak.
    pub active_today: bool,
    /// Local days with any successful record, all time.
    pub active_days: u32,
}

/// One local day of the Insights charts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DayStats {
    /// `YYYY-MM-DD`.
    pub date: String,
    pub words: u64,
    pub dictations: u32,
    pub commands: u32,
}

/// How far back the Insights page looks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InsightsRange {
    Week,
    Month,
    Quarter,
    Year,
}

impl InsightsRange {
    pub fn days(self) -> u32 {
        match self {
            Self::Week => 7,
            Self::Month => 30,
            Self::Quarter => 90,
            Self::Year => 365,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Insights {
    /// Every day in the range, oldest first, including days with no activity.
    pub days: Vec<DayStats>,
    pub total_words: u64,
    pub active_days: u32,
    pub commands: u32,
}

fn offset_ms(utc_offset_minutes: i32) -> i64 {
    utc_offset_minutes as i64 * 60_000
}

fn local_day(ms: i64, utc_offset_minutes: i32) -> i64 {
    (ms + offset_ms(utc_offset_minutes)).div_euclid(DAY_MS)
}

/// Consecutive days ending at `today` or `today - 1`. `days` is sorted newest first.
pub fn streak(days: &[i64], today: i64) -> (u32, bool) {
    let Some(&latest) = days.first() else {
        return (0, false);
    };
    if latest != today && latest != today - 1 {
        return (0, false);
    }
    let mut n = 0u32;
    let mut expect = latest;
    for &d in days {
        if d != expect {
            break;
        }
        n += 1;
        expect -= 1;
    }
    (n, latest == today)
}

impl Store {
    /// Distinct local days with a successful record, newest first.
    fn active_day_numbers(&self, utc_offset_minutes: i32) -> Result<Vec<i64>, StoreError> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT DISTINCT {DAY_EXPR} AS d FROM records WHERE status = 'ok' ORDER BY d DESC"
        ))?;
        let rows = stmt.query_map([offset_ms(utc_offset_minutes)], |r| r.get(0))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Stat cards for Home. `now_ms` is the current time (Unix ms).
    pub fn home_stats(&self, now_ms: i64, utc_offset_minutes: i32) -> Result<HomeStats, StoreError> {
        let (total_words, dictations, speaking_ms, time_saved_ms, weighted, weight): (i64, i64, i64, i64, i64, i64) =
            self.conn.query_row(
                "SELECT COALESCE(SUM(words), 0), COUNT(*), COALESCE(SUM(duration_ms), 0),
                        COALESCE(SUM(MAX(0, words * ?1 - duration_ms)), 0),
                        COALESCE(SUM(CASE WHEN wpm > 0 THEN words * wpm ELSE 0 END), 0),
                        COALESCE(SUM(CASE WHEN wpm > 0 THEN words ELSE 0 END), 0)
                 FROM records WHERE status = 'ok' AND kind = 'dictation'",
                [TYPING_MS_PER_WORD as i64],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
            )?;
        let days = self.active_day_numbers(utc_offset_minutes)?;
        let (streak_days, active_today) = streak(&days, local_day(now_ms, utc_offset_minutes));
        Ok(HomeStats {
            total_words: total_words as u64,
            dictations: dictations as u64,
            speaking_ms: speaking_ms as u64,
            time_saved_ms: time_saved_ms as u64,
            average_wpm: if weight > 0 {
                ((weighted as f64) / (weight as f64)).round() as u32
            } else {
                0
            },
            streak_days,
            active_today,
            active_days: days.len() as u32,
        })
    }

    /// Per-day words, dictations and commands for the `days` local days ending today.
    pub fn insights(&self, days: u32, now_ms: i64, utc_offset_minutes: i32) -> Result<Insights, StoreError> {
        let days = days.clamp(1, 366) as i64;
        let today = local_day(now_ms, utc_offset_minutes);
        let first = today - days + 1;
        let off = offset_ms(utc_offset_minutes);
        let mut out: Vec<DayStats> = (first..=today)
            .map(|d| DayStats {
                date: local_date(d * DAY_MS - off, utc_offset_minutes),
                words: 0,
                dictations: 0,
                commands: 0,
            })
            .collect();
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {DAY_EXPR} AS d,
                    COALESCE(SUM(CASE WHEN kind = 'dictation' THEN words ELSE 0 END), 0),
                    SUM(kind = 'dictation'), SUM(kind = 'command')
             FROM records
             WHERE status = 'ok' AND created_at >= ?2 AND created_at < ?3
             GROUP BY d"
        ))?;
        let rows = stmt.query_map(params![off, first * DAY_MS - off, (today + 1) * DAY_MS - off], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?))
        })?;
        for row in rows {
            let (d, words, dictations, commands) = row?;
            if let Some(slot) = out.get_mut((d - first) as usize) {
                slot.words = words as u64;
                slot.dictations = dictations as u32;
                slot.commands = commands as u32;
            }
        }
        Ok(Insights {
            total_words: out.iter().map(|d| d.words).sum(),
            active_days: out.iter().filter(|d| d.dictations + d.commands > 0).count() as u32,
            commands: out.iter().map(|d| d.commands).sum(),
            days: out,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NewRecord, RecordStatus, RecordType};

    const HOUR: i64 = 3_600_000;
    /// 2025-11-26T00:00:00Z
    const T0: i64 = 1_764_115_200_000;

    fn open() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let (s, _) = Store::open(&dir.path().join("h.db"), &[3u8; 32]).unwrap();
        (dir, s)
    }

    fn dict(s: &Store, at: i64, words: u32, duration_ms: u64, wpm: u32) {
        s.insert(NewRecord {
            text: "x".into(),
            words,
            duration_ms,
            wpm,
            created_at: at,
            ..Default::default()
        })
        .unwrap();
    }

    fn other(s: &Store, at: i64, kind: RecordType, status: RecordStatus, words: u32) {
        s.insert(NewRecord {
            kind: Some(kind),
            status: Some(status),
            text: "x".into(),
            words,
            duration_ms: 1000,
            wpm: 100,
            created_at: at,
            ..Default::default()
        })
        .unwrap();
    }

    #[test]
    fn empty_store_is_all_zero() {
        let (_d, s) = open();
        assert_eq!(s.home_stats(T0, 0).unwrap(), HomeStats::default());
        let i = s.insights(7, T0, 0).unwrap();
        assert_eq!(i.days.len(), 7);
        assert_eq!((i.total_words, i.active_days, i.commands), (0, 0, 0));
        assert_eq!(i.days.last().unwrap().date, "2025-11-26");
        assert_eq!(i.days[0].date, "2025-11-20");
    }

    #[test]
    fn time_saved_and_weighted_wpm() {
        let (_d, s) = open();
        // 100 words in 30 s: typing 150 s, saved 120 s, 200 wpm.
        dict(&s, T0, 100, 30_000, 200);
        // 10 words in 20 s: typing 15 s, slower than typing, saves 0 (not -5 s).
        dict(&s, T0 + 1, 10, 20_000, 30);
        let h = s.home_stats(T0, 0).unwrap();
        assert_eq!(h.total_words, 110);
        assert_eq!(h.dictations, 2);
        assert_eq!(h.speaking_ms, 50_000);
        assert_eq!(h.time_saved_ms, 120_000);
        // (100*200 + 10*30) / 110 = 184.5 -> 185
        assert_eq!(h.average_wpm, 185);
    }

    #[test]
    fn failed_running_and_commands_excluded_from_words() {
        let (_d, s) = open();
        other(&s, T0, RecordType::Dictation, RecordStatus::Failed, 500);
        other(&s, T0, RecordType::Dictation, RecordStatus::Running, 500);
        other(&s, T0, RecordType::Command, RecordStatus::Ok, 500);
        let h = s.home_stats(T0, 0).unwrap();
        assert_eq!((h.total_words, h.dictations, h.time_saved_ms, h.average_wpm), (0, 0, 0, 0));
        // The successful command still counts as activity.
        assert_eq!((h.streak_days, h.active_today, h.active_days), (1, true, 1));
        let i = s.insights(1, T0, 0).unwrap();
        assert_eq!((i.days[0].words, i.days[0].commands, i.days[0].dictations), (0, 1, 0));
    }

    #[test]
    fn only_failed_records_mean_no_streak() {
        let (_d, s) = open();
        other(&s, T0, RecordType::Dictation, RecordStatus::Failed, 5);
        let h = s.home_stats(T0, 0).unwrap();
        assert_eq!((h.streak_days, h.active_days), (0, 0));
    }

    #[test]
    fn records_without_wpm_do_not_drag_speed_down() {
        let (_d, s) = open();
        dict(&s, T0, 50, 10_000, 300);
        dict(&s, T0, 50, 0, 0); // e.g. recovered with no timing
        assert_eq!(s.home_stats(T0, 0).unwrap().average_wpm, 300);
    }

    #[test]
    fn streak_counts_through_yesterday_and_breaks_on_a_gap() {
        let (_d, s) = open();
        let day = |n: i64| T0 + n * DAY_MS + 12 * HOUR;
        for n in [0, 1, 2, 4, 5] {
            dict(&s, day(n), 1, 1000, 60);
        }
        // Day 5 is today: 4 and 5 are a run; day 3 is the gap.
        let h = s.home_stats(day(5), 0).unwrap();
        assert_eq!((h.streak_days, h.active_today, h.active_days), (2, true, 5));
        // Day 6, nothing yet today: still ongoing from yesterday.
        let h = s.home_stats(day(6), 0).unwrap();
        assert_eq!((h.streak_days, h.active_today), (2, false));
        // Day 7: yesterday was empty, so the streak is broken.
        let h = s.home_stats(day(7), 0).unwrap();
        assert_eq!((h.streak_days, h.active_today), (0, false));
    }

    #[test]
    fn local_days_follow_the_offset() {
        let (_d, s) = open();
        // 2025-11-26 23:30 UTC and 2025-11-27 00:30 UTC.
        let late = T0 + 23 * HOUR + 30 * 60_000;
        let early = late + HOUR;
        dict(&s, late, 10, 1000, 60);
        dict(&s, early, 20, 1000, 60);
        // UTC: two days.
        let h = s.home_stats(early, 0).unwrap();
        assert_eq!((h.streak_days, h.active_days), (2, 2));
        // UTC-8: both are on the 26th local (15:30 and 16:30).
        let h = s.home_stats(early, -480).unwrap();
        assert_eq!((h.streak_days, h.active_days), (1, 1));
        let i = s.insights(2, early, -480).unwrap();
        assert_eq!(i.days[1].date, "2025-11-26");
        assert_eq!(i.days[1].words, 30);
        assert_eq!(i.days[0].words, 0);
        // UTC+5:30: both on the 27th local (05:00 and 06:00).
        let i = s.insights(2, early, 330).unwrap();
        assert_eq!(
            i.days.iter().map(|d| (d.date.as_str(), d.words)).collect::<Vec<_>>(),
            vec![("2025-11-26", 0), ("2025-11-27", 30)]
        );
        // UTC+1: 00:30 and 01:30 on the 27th.
        let i = s.insights(1, early, 60).unwrap();
        assert_eq!((i.days[0].date.as_str(), i.days[0].words), ("2025-11-27", 30));
    }

    #[test]
    fn insights_range_bounds_and_zero_fill() {
        let (_d, s) = open();
        let now = T0 + 10 * DAY_MS + HOUR;
        dict(&s, now - 7 * DAY_MS, 99, 1000, 60); // just outside a 7-day range
        dict(&s, now - 6 * DAY_MS, 5, 1000, 60); // first day of it
        dict(&s, now, 7, 1000, 60);
        other(&s, now, RecordType::Command, RecordStatus::Ok, 0);
        other(&s, now, RecordType::Command, RecordStatus::Failed, 0);
        dict(&s, now + DAY_MS, 1000, 1000, 60); // tomorrow (clock skew): not shown
        let i = s.insights(InsightsRange::Week.days(), now, 0).unwrap();
        assert_eq!(i.days.len(), 7);
        assert_eq!(i.days[0].words, 5);
        assert_eq!(i.days[6].words, 7);
        assert_eq!(i.days[6].commands, 1);
        assert_eq!(i.total_words, 12);
        assert_eq!(i.active_days, 2);
        assert_eq!(i.commands, 1);
        assert!(i.days[1..6].iter().all(|d| d.words == 0 && d.commands == 0));
    }

    #[test]
    fn streak_helper() {
        assert_eq!(streak(&[], 10), (0, false));
        assert_eq!(streak(&[10], 10), (1, true));
        assert_eq!(streak(&[9, 8, 7], 10), (3, false));
        assert_eq!(streak(&[8, 7], 10), (0, false));
        assert_eq!(streak(&[10, 9, 7], 10), (2, true));
    }

    #[test]
    fn day_numbers_floor_before_the_epoch() {
        assert_eq!(local_day(-1, 0), -1);
        assert_eq!(local_day(0, -60), -1);
        let (_d, s) = open();
        dict(&s, -1, 1, 1000, 60);
        assert_eq!(s.active_day_numbers(0).unwrap(), vec![-1]);
    }
}
