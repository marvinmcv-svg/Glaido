/** Home stats and Insights: types mirroring `vocalis_store::stats` and pure copy helpers (spec 4.6). */

export type HomeStats = {
  totalWords: number;
  dictations: number;
  speakingMs: number;
  timeSavedMs: number;
  averageWpm: number;
  streakDays: number;
  activeToday: boolean;
  activeDays: number;
};

export type DayStats = { date: string; words: number; dictations: number; commands: number };
export type InsightsRange = "week" | "month" | "quarter" | "year";
export type Insights = {
  days: DayStats[];
  totalWords: number;
  activeDays: number;
  commands: number;
};

/** The typing speed time saved and "×N faster" compare against (ADR-0013). */
export const TYPING_WPM = 40;

export const RANGES: { value: InsightsRange; label: string; days: number }[] = [
  { value: "week", label: "7 days", days: 7 },
  { value: "month", label: "30 days", days: 30 },
  { value: "quarter", label: "90 days", days: 90 },
  { value: "year", label: "12 months", days: 365 },
];

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const WORK_DAY = 8 * HOUR;
const WORK_WEEK = 5 * WORK_DAY;

/** "0m", "45m", "3h 12m", "123h 46m". Rounds to the nearest minute. */
export function formatDuration(ms: number): string {
  const minutes = Math.max(0, Math.round(ms / MINUTE));
  const h = Math.floor(minutes / 60);
  const m = minutes % 60;
  return h === 0 ? `${m}m` : `${h}h ${m}m`;
}

/** A human comparison for time saved, e.g. "about a full work day". */
export function timeSavedComparison(ms: number): string {
  if (ms < MINUTE) return "Your first dictation starts the clock";
  if (ms < 15 * MINUTE) return "about a coffee break";
  if (ms < HOUR) return "about a lunch break";
  if (ms < 2.5 * HOUR) return "about a feature film";
  if (ms < 6 * HOUR) return "about an afternoon";
  if (ms < 1.5 * WORK_DAY) return "about a full work day";
  if (ms < 4.5 * WORK_DAY) return `about ${Math.round(ms / WORK_DAY)} work days`;
  if (ms < 1.5 * WORK_WEEK) return "about a full work week";
  if (ms < 8.5 * WORK_WEEK) return `about ${Math.round(ms / WORK_WEEK)} work weeks`;
  const months = Math.round(ms / (WORK_WEEK * (52 / 12)));
  return months <= 1 ? "about a month of work days" : `about ${months} months of work days`;
}

/** "×4.5 faster than typing" against a 40 wpm typist. */
export function speedComparison(wpm: number): string {
  if (wpm <= 0) return "Measured after your first dictation";
  const ratio = wpm / TYPING_WPM;
  if (ratio < 1.1) return "about as fast as typing";
  const n = ratio < 10 ? Math.round(ratio * 10) / 10 : Math.round(ratio);
  return `×${n} faster than typing`;
}

export function streakValue(days: number): string {
  return days === 1 ? "1 day" : `${days} days`;
}

/** A friendly line under the streak. */
export function streakLabel(days: number, activeToday: boolean): string {
  if (days === 0) return "Dictate today to start one";
  if (!activeToday) return "Dictate today to keep it going";
  if (days === 1) return "Day one. See you tomorrow";
  if (days < 7) return "Building a habit";
  if (days < 14) return "A whole week and counting";
  if (days < 30) return "Weeks in a row. Nice rhythm";
  if (days < 100) return "A month or more. Impressive";
  return "A true regular";
}

/** 1,284 / 12.9K / 4.2M. */
export function compactNumber(n: number, locale?: string): string {
  if (Math.abs(n) < 10_000) return Math.round(n).toLocaleString(locale ?? "en-US");
  return new Intl.NumberFormat(locale ?? "en-US", {
    notation: "compact",
    maximumFractionDigits: 1,
  }).format(n);
}

/** Round chart maximum and evenly spaced ticks (0 included), e.g. 0/500/1,000/1,500. */
export function niceTicks(max: number, count = 4): number[] {
  if (max <= 0) return [0, 1];
  const raw = max / count;
  const mag = 10 ** Math.floor(Math.log10(raw));
  const step = ([1, 2, 2.5, 5, 10].find((s) => s * mag >= raw) ?? 10) * mag;
  const ticks: number[] = [];
  for (let v = 0; v < max + step - 1e-9; v += step) ticks.push(Math.round(v * 1e6) / 1e6);
  return ticks;
}

export type Bucket = DayStats & { label: string; days: number; activeDays: number };

/**
 * Long ranges are drawn as weekly columns so each stays wide enough to read and
 * hover. Buckets keep the order of `days` (oldest first); the last bucket may be short.
 */
export function bucketDays(days: DayStats[], maxColumns = 92): Bucket[] {
  const size = days.length > maxColumns ? 7 : 1;
  const out: Bucket[] = [];
  // Align weeks so the final bucket ends today.
  const start = days.length % size;
  if (start > 0) out.push(sum(days.slice(0, start)));
  for (let i = start; i < days.length; i += size) out.push(sum(days.slice(i, i + size)));
  return out;
}

function sum(ds: DayStats[]): Bucket {
  const first = ds[0]?.date ?? "";
  const last = ds.at(-1)?.date ?? first;
  return {
    date: first,
    label: ds.length > 1 ? `${first} – ${last}` : first,
    days: ds.length,
    words: ds.reduce((n, d) => n + d.words, 0),
    dictations: ds.reduce((n, d) => n + d.dictations, 0),
    commands: ds.reduce((n, d) => n + d.commands, 0),
    activeDays: ds.filter((d) => d.dictations + d.commands > 0).length,
  };
}

/** "Wed, Nov 26" for a `YYYY-MM-DD` key. */
export function shortDate(date: string, locale?: string): string {
  const [y = 1970, m = 1, d = 1] = date.split("-").map(Number);
  return new Date(y, m - 1, d).toLocaleDateString(locale, {
    weekday: "short",
    month: "short",
    day: "numeric",
  });
}
