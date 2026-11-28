//! Home stats, Insights, Clear history and Report a record (spec 4.6).
//!
//! Reports: there is no support endpoint yet, so `history_report` builds the
//! payload and returns it to the window, which shows exactly what would be
//! shared. Nothing is written to disk (record text outside the encrypted store
//! would break the zero-retention rule) and nothing leaves the device. When the
//! endpoint exists, this command sends the same payload. See ADR-0013.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime, State};
use vocalis_core::dictionary::Entry;
use vocalis_store::{HomeStats, Insights, InsightsRange, PersonalError, Record};

use crate::engine::EngineHandle;

fn engine<R: Runtime>(app: &AppHandle<R>) -> Result<State<'_, EngineHandle>, String> {
    app.try_state::<EngineHandle>()
        .ok_or_else(|| "History is unavailable because the dictation engine did not start.".to_owned())
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Home stat cards. `utc_offset_minutes` is the window's offset, east positive.
#[tauri::command]
pub fn stats_home<R: Runtime>(app: AppHandle<R>, utc_offset_minutes: i32) -> Result<HomeStats, String> {
    engine(&app)?
        .history
        .lock()
        .home_stats(now_ms(), utc_offset_minutes)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn stats_insights<R: Runtime>(
    app: AppHandle<R>,
    range: InsightsRange,
    utc_offset_minutes: i32,
) -> Result<Insights, String> {
    engine(&app)?
        .history
        .lock()
        .insights(range.days(), now_ms(), utc_offset_minutes)
        .map_err(|e| e.to_string())
}

/// Settings → Clear history. Deletes every record and every recording,
/// including failed ones and their audio. The window asks for confirmation.
#[tauri::command]
pub fn history_clear_all<R: Runtime>(app: AppHandle<R>) -> Result<usize, String> {
    let engine = engine(&app)?;
    let paths = engine.history.lock().clear_all().map_err(|e| e.to_string())?;
    let n = paths.len();
    for p in paths {
        let _ = std::fs::remove_file(p);
    }
    engine.history.changed();
    Ok(n)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportCategory {
    WrongWord,
    MissingText,
    WrongFormatting,
    WrongApp,
    Other,
}

/// What a report shares: one record's text and outcome. Never audio, the
/// audio path, the app, the URL, or any other record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportPayload {
    pub record_id: String,
    pub kind: vocalis_store::RecordType,
    pub status: vocalis_store::RecordStatus,
    pub error_code: Option<String>,
    pub text: String,
    pub category: ReportCategory,
    pub correct_spelling: Option<String>,
    pub app_version: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportResult {
    pub payload: ReportPayload,
    /// `false` until a support endpoint exists: the report stayed on this device.
    pub sent: bool,
    pub added_to_dictionary: bool,
    /// Why the dictionary entry wasn't added (already there, too long, …).
    pub dictionary_note: Option<String>,
}

fn clean(s: Option<String>) -> Option<String> {
    s.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty())
}

pub fn build_report(record: &Record, category: ReportCategory, correct_spelling: Option<String>) -> ReportPayload {
    ReportPayload {
        record_id: record.id.clone(),
        kind: record.kind,
        status: record.status,
        error_code: record.error_code.clone(),
        text: record.text.clone(),
        category,
        correct_spelling: clean(correct_spelling),
        app_version: env!("CARGO_PKG_VERSION"),
    }
}

/// The dictionary entry for "Add to my dictionary": a correction when the user
/// said which word was wrong, otherwise a vocabulary word.
pub fn dictionary_entry(wrong_word: Option<String>, correct_spelling: Option<String>) -> Option<Entry> {
    let word = clean(correct_spelling)?;
    Some(match clean(wrong_word) {
        Some(w) if !w.eq_ignore_ascii_case(&word) => Entry::correction(w, word),
        _ => Entry::vocab(word),
    })
}

/// Report a record (thumbs-down).
#[tauri::command]
pub fn history_report<R: Runtime>(
    app: AppHandle<R>,
    id: String,
    category: ReportCategory,
    correct_spelling: Option<String>,
    wrong_word: Option<String>,
    add_to_dictionary: bool,
) -> Result<ReportResult, String> {
    let engine = engine(&app)?;
    let store = engine.history.lock();
    let record = store.get(&id).map_err(|e| e.to_string())?;
    let payload = build_report(&record, category, correct_spelling.clone());
    let (mut added, mut note) = (false, None);
    if add_to_dictionary {
        match dictionary_entry(wrong_word, correct_spelling) {
            None => note = Some("Type the right spelling to add it to your dictionary.".to_owned()),
            Some(entry) => match store.dictionary_add(entry, now_ms()) {
                Ok(_) => added = true,
                Err(PersonalError::Entry(e)) => note = Some(e.to_string()),
                Err(e) => return Err(e.to_string()),
            },
        }
    }
    Ok(ReportResult {
        payload,
        sent: false,
        added_to_dictionary: added,
        dictionary_note: note,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use vocalis_store::{RecordStatus, RecordType};

    fn record() -> Record {
        Record {
            id: "r1".into(),
            kind: RecordType::Dictation,
            status: RecordStatus::Ok,
            text: "meet at the cafe".into(),
            app_name: Some("Mail".into()),
            app_id: Some("com.example.mail".into()),
            url: Some("https://example.com/private".into()),
            field_kind: None,
            words: 4,
            duration_ms: 1200,
            wpm: 200,
            audio_path: Some("/data/recordings/r1.wav.enc".into()),
            error_code: None,
            created_at: 0,
        }
    }

    #[test]
    fn report_has_only_the_text_never_audio_or_app() {
        let p = build_report(&record(), ReportCategory::WrongWord, Some("  café ".into()));
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains("meet at the cafe"));
        assert_eq!(p.correct_spelling.as_deref(), Some("café"));
        for leak in ["wav", "recordings", "Mail", "com.example", "example.com", "audio"] {
            assert!(!json.contains(leak), "report leaked {leak}: {json}");
        }
    }

    #[test]
    fn dictionary_entry_shapes() {
        assert_eq!(dictionary_entry(None, None), None);
        assert_eq!(dictionary_entry(Some("cafe".into()), Some(" ".into())), None);
        assert_eq!(
            dictionary_entry(None, Some("Vocalis".into())),
            Some(Entry::vocab("Vocalis"))
        );
        assert_eq!(
            dictionary_entry(Some(" vocal is ".into()), Some("Vocalis".into())),
            Some(Entry::correction("vocal is", "Vocalis"))
        );
        // Same word in different case: nothing to correct, just a hint.
        assert_eq!(
            dictionary_entry(Some("vocalis".into()), Some("Vocalis".into())),
            Some(Entry::vocab("Vocalis"))
        );
    }
}
