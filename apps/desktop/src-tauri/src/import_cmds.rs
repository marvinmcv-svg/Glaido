//! Importing dictionary entries and snippets from other tools (spec 4.5.4),
//! and dictionary auto-suggestions (spec 4.5.1).
//!
//! Import: `import_preview` parses a file and reports what would be added,
//! skipped (with the reason) or converted; `import_apply` parses it again
//! against the current dictionary and snippets and saves the result. The file
//! comes either as a path or as bytes the webview read from a file picker.
//!
//! Suggestions: the state (`suggestions.json` in the app data directory) holds
//! only `misspelling → word` pairs and the times each fix was seen, never the
//! texts they came from, so it is a plain JSON file (see
//! `vocalis_core::suggest`). `suggestion_observe` feeds a pasted text and the
//! user's edited version (History edits today; reading the field after a
//! paste once `vocalis_context::field_text` has a reader for the platform).

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime, State};
use vocalis_core::dictionary::Entry;
use vocalis_core::snippets::Snippet;
use vocalis_core::suggest::{self, Suggestion, SuggestionState};
use vocalis_import::{Format, ImportReport, SkipReason, Skipped, MAX_FILE_BYTES};
use vocalis_store::{DictionaryEntry, Store};

use crate::engine::EngineHandle;

const SUGGESTIONS_FILE: &str = "suggestions.json";

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn engine<R: Runtime>(app: &AppHandle<R>) -> Result<State<'_, EngineHandle>, String> {
    app.try_state::<EngineHandle>()
        .ok_or_else(|| "Your dictionary is unavailable because the dictation engine did not start.".to_owned())
}

fn library(store: &Store) -> Result<(Vec<Entry>, Vec<Snippet>), String> {
    let entries = store.dictionary().map_err(|e| e.to_string())?;
    let snippets = store.snippets().map_err(|e| e.to_string())?;
    Ok((
        entries.iter().map(DictionaryEntry::entry).collect(),
        snippets.iter().map(|s| s.snippet()).collect(),
    ))
}

// ---------- import ----------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FormatInfo {
    id: Format,
    label: &'static str,
}

#[tauri::command]
pub fn import_formats() -> Vec<FormatInfo> {
    Format::ALL
        .iter()
        .map(|&f| FormatInfo {
            id: f,
            label: f.label(),
        })
        .collect()
}

/// The file's name and bytes, from a path or from the webview.
fn read_source(path: Option<String>, name: Option<String>, data: Option<Vec<u8>>) -> Result<(String, Vec<u8>), String> {
    match (path, data) {
        (Some(path), _) => {
            let p = PathBuf::from(&path);
            let len = std::fs::metadata(&p)
                .map_err(|_| "Couldn't open that file.".to_owned())?
                .len();
            if len > MAX_FILE_BYTES as u64 {
                return Err(vocalis_import::ImportError::TooLarge.to_string());
            }
            let bytes = std::fs::read(&p).map_err(|_| "Couldn't read that file.".to_owned())?;
            let name = p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            Ok((name, bytes))
        }
        (None, Some(data)) => Ok((name.unwrap_or_default(), data)),
        (None, None) => Err("Choose a file to import.".into()),
    }
}

fn parse(store: &Store, format: Option<Format>, name: &str, data: &[u8]) -> Result<ImportReport, String> {
    let (entries, snippets) = library(store)?;
    vocalis_import::import_auto(format, name, data, &entries, &snippets).map_err(|e| {
        let msg = e.to_string();
        let mut c = msg.chars();
        c.next()
            .map(|f| f.to_uppercase().chain(c).collect::<String>() + ".")
            .unwrap_or(msg)
    })
}

/// Save a report's items. Anything the store refuses (a race with another
/// edit) moves to `skipped`; the returned report lists what was saved.
fn apply(store: &Store, mut report: ImportReport, now: i64) -> ImportReport {
    let mut saved_entries = Vec::new();
    for e in std::mem::take(&mut report.entries) {
        match store.dictionary_add(e.clone(), now) {
            Ok(_) => saved_entries.push(e),
            Err(err) => report.skipped.push(Skipped {
                item: e.word.clone(),
                reason: SkipReason::Invalid {
                    detail: err.to_string(),
                },
            }),
        }
    }
    let mut saved_snippets = Vec::new();
    for s in std::mem::take(&mut report.snippets) {
        match store.snippet_add(s.clone(), now) {
            Ok(_) => saved_snippets.push(s),
            Err(err) => report.skipped.push(Skipped {
                item: s.trigger.clone(),
                reason: SkipReason::Invalid {
                    detail: err.to_string(),
                },
            }),
        }
    }
    report.entries = saved_entries;
    report.snippets = saved_snippets;
    report
}

/// What importing the file would do. Nothing is saved.
#[tauri::command]
pub fn import_preview<R: Runtime>(
    app: AppHandle<R>,
    path: Option<String>,
    name: Option<String>,
    data: Option<Vec<u8>>,
    format: Option<Format>,
) -> Result<ImportReport, String> {
    let (name, data) = read_source(path, name, data)?;
    let engine = engine(&app)?;
    let store = engine.history.lock();
    parse(&store, format, &name, &data)
}

/// Import the file. Emits `dictionary-changed` / `snippets-changed` to the main window.
#[tauri::command]
pub fn import_apply<R: Runtime>(
    app: AppHandle<R>,
    path: Option<String>,
    name: Option<String>,
    data: Option<Vec<u8>>,
    format: Option<Format>,
) -> Result<ImportReport, String> {
    let (name, data) = read_source(path, name, data)?;
    let engine = engine(&app)?;
    let report = {
        let store = engine.history.lock();
        let report = parse(&store, format, &name, &data)?;
        apply(&store, report, now_ms())
    };
    if !report.entries.is_empty() {
        let _ = app.emit_to("main", "dictionary-changed", ());
    }
    if !report.snippets.is_empty() {
        let _ = app.emit_to("main", "snippets-changed", ());
    }
    Ok(report)
}

// ---------- suggestions ----------

/// Suggestion state with its file, loaded on first use.
static SUGGESTIONS: Mutex<Option<(PathBuf, SuggestionState)>> = Mutex::new(None);

fn load_state(path: &Path) -> SuggestionState {
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|_| {
            // A damaged file only loses suggestions; keep it aside and start over.
            let _ = std::fs::rename(path, path.with_extension("json.damaged"));
            SuggestionState::new()
        }),
        Err(_) => SuggestionState::new(),
    }
}

fn save_state(path: &Path, state: &SuggestionState) -> Result<(), String> {
    let json = serde_json::to_vec(state).map_err(|e| e.to_string())?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json).map_err(|e| e.to_string())?;
    std::fs::rename(tmp, path).map_err(|e| e.to_string())
}

/// Run `f` on the state, then save it.
fn with_state<R: Runtime, T>(app: &AppHandle<R>, f: impl FnOnce(&mut SuggestionState) -> T) -> Result<T, String> {
    let path = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join(SUGGESTIONS_FILE);
    let mut guard = SUGGESTIONS.lock().map_err(|_| "suggestions lock".to_owned())?;
    if guard.as_ref().is_none_or(|(p, _)| p != &path) {
        *guard = Some((path.clone(), load_state(&path)));
    }
    let (_, state) = guard.as_mut().expect("loaded");
    let out = f(state);
    save_state(&path, state)?;
    Ok(out)
}

fn dictionary_words<R: Runtime>(app: &AppHandle<R>) -> Vec<Entry> {
    engine(app)
        .ok()
        .and_then(|e| e.history.lock().dictionary().ok())
        .map(|d| d.iter().map(DictionaryEntry::entry).collect())
        .unwrap_or_default()
}

/// Suggestions to show, most recent first.
#[tauri::command]
pub fn suggestion_list<R: Runtime>(app: AppHandle<R>) -> Result<Vec<Suggestion>, String> {
    let dict = dictionary_words(&app);
    let now = now_ms();
    with_state(&app, |s| {
        s.prune(now);
        s.pending(now, &dict)
    })
}

/// Compare a pasted text with the user's edited version. Returns suggestions
/// that just became ready, and emits `suggestions-changed` when there are any.
/// Neither text is stored or logged.
#[tauri::command]
pub fn suggestion_observe<R: Runtime>(
    app: AppHandle<R>,
    pasted: String,
    edited: String,
) -> Result<Vec<Suggestion>, String> {
    let fixes = suggest::detect_fixes(&pasted, &edited);
    if fixes.is_empty() {
        return Ok(Vec::new());
    }
    let dict = dictionary_words(&app);
    let ready = with_state(&app, |s| s.observe(&fixes, now_ms(), &dict))?;
    if !ready.is_empty() {
        let _ = app.emit_to("main", "suggestions-changed", &ready);
    }
    Ok(ready)
}

/// Add the suggested correction to the dictionary.
#[tauri::command]
pub fn suggestion_accept<R: Runtime>(app: AppHandle<R>, from: String, to: String) -> Result<DictionaryEntry, String> {
    let engine = engine(&app)?;
    let dict: Vec<Entry> = engine
        .history
        .lock()
        .dictionary()
        .map_err(|e| e.to_string())?
        .iter()
        .map(DictionaryEntry::entry)
        .collect();
    let entry = with_state(&app, |s| s.accept(&from, &to, &dict))?.map_err(|e| e.to_string())?;
    let saved = engine
        .history
        .lock()
        .dictionary_add(entry, now_ms())
        .map_err(|e| e.to_string())?;
    let _ = app.emit_to("main", "dictionary-changed", ());
    Ok(saved)
}

/// Never suggest this pair again.
#[tauri::command]
pub fn suggestion_dismiss<R: Runtime>(app: AppHandle<R>, from: String, to: String) -> Result<(), String> {
    with_state(&app, |s| s.dismiss(&from, &to))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_file_round_trip_and_damage() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(SUGGESTIONS_FILE);
        assert_eq!(load_state(&path), SuggestionState::new());
        let mut s = SuggestionState::new();
        let fixes = suggest::detect_fixes("ship kubernets", "ship Kubernetes");
        s.observe(&fixes, 1, &[]);
        save_state(&path, &s).unwrap();
        assert_eq!(load_state(&path), s);
        std::fs::write(&path, b"{not json").unwrap();
        assert_eq!(load_state(&path), SuggestionState::new());
        assert!(path.with_extension("json.damaged").exists());
    }

    #[test]
    fn reads_sources() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("words.csv");
        std::fs::write(&p, b"Vocalis\n").unwrap();
        let (name, data) = read_source(Some(p.to_string_lossy().into()), None, None).unwrap();
        assert_eq!((name.as_str(), data.as_slice()), ("words.csv", b"Vocalis\n".as_slice()));
        let (name, data) = read_source(None, Some("x.json".into()), Some(b"[]".to_vec())).unwrap();
        assert_eq!((name.as_str(), data.len()), ("x.json", 2));
        assert!(read_source(None, None, None).is_err());
        assert!(read_source(Some(dir.path().join("missing").to_string_lossy().into()), None, None).is_err());
    }

    #[test]
    fn preview_then_apply_against_a_store() {
        let dir = tempfile::tempdir().unwrap();
        let (store, _) = Store::open(&dir.path().join("h.db"), &[7u8; 32]).unwrap();
        store.dictionary_add(Entry::vocab("Vocalis"), 1).unwrap();
        let csv = b"vocalis\npostgres,PostgreSQL\n";
        let report = parse(&store, Some(Format::WisprFlow), "d.csv", csv).unwrap();
        assert_eq!(report.entries, vec![Entry::correction("postgres", "PostgreSQL")]);
        assert_eq!(report.skipped.len(), 1);
        let done = apply(&store, report, 2);
        assert_eq!(done.entries.len(), 1);
        assert_eq!(store.dictionary().unwrap().len(), 2);
        // Importing again adds nothing.
        let again = parse(&store, Some(Format::WisprFlow), "d.csv", csv).unwrap();
        assert!(again.entries.is_empty());
        let snips = br#"[{"name":"sig","text":"Best, Sam","keyword":"!sig"}]"#;
        let done = apply(&store, parse(&store, None, "s.json", snips).unwrap(), 3);
        assert_eq!(done.snippets.len(), 1);
        assert_eq!(store.snippets().unwrap().len(), 1);
        assert!(parse(&store, None, "d.csv", b"a,b\n").unwrap_err().ends_with('.'));
    }
}
