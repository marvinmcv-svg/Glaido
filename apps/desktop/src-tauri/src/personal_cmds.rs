//! Dictionary and Snippets commands for the main window (spec 4.5.1, 4.5.2),
//! and the personalization part of every dictation's start settings.
//!
//! Entries live in the engine's encrypted store; validation is the core's, so
//! the page, the store and the gateway agree on the rules.

use chrono::{DateTime, Local, Offset, TimeZone};
use tauri::{AppHandle, Emitter, Manager, Runtime, State};
use vocalis_core::dictionary::{Entry, EntryError};
use vocalis_core::snippets::{Snippet, SnippetError};
use vocalis_core::tags::{TagContext, TAG_CLIPBOARD};
use vocalis_store::{DictionaryEntry, PersonalError, SnippetEntry, Store, StoreError};

use crate::engine::EngineHandle;

/// Event sent to the main window after any dictionary or snippet change.
pub const CHANGED_EVENT: &str = "personal-changed";

fn engine<R: Runtime>(app: &AppHandle<R>) -> Result<State<'_, EngineHandle>, String> {
    app.try_state::<EngineHandle>().ok_or_else(|| {
        "Your dictionary and snippets are unavailable because the dictation engine did not start.".to_owned()
    })
}

fn now_ms() -> i64 {
    Local::now().timestamp_millis()
}

/// A short sentence for the page. Database details go to the log, not the user.
pub fn error_message(e: &PersonalError) -> String {
    match e {
        PersonalError::Entry(e) => match e {
            EntryError::EmptyWord => "Type a word first.".into(),
            EntryError::WordTooLong => "Keep the word to 50 characters or fewer.".into(),
            EntryError::MisspellingTooLong => "Keep the misspelling to 50 characters or fewer.".into(),
            EntryError::TagRequiresCorrection => "Tags only work in corrections. Add what you say on the left.".into(),
            EntryError::Duplicate => "Duplicate entry".into(),
        },
        PersonalError::Snippet(e) => match e {
            SnippetError::EmptyTrigger => "Give the snippet a trigger phrase.".into(),
            SnippetError::TriggerTooLong => "Keep the trigger to 200 characters or fewer.".into(),
            SnippetError::EmptyContent => "Add the text the snippet should insert.".into(),
            SnippetError::ContentTooLong => "Snippets can hold up to 10,000 characters.".into(),
        },
        PersonalError::Store(e) => store_message(e),
    }
}

fn store_message(e: &StoreError) -> String {
    match e {
        StoreError::NotFound => "That item was already deleted.".into(),
        other => {
            tracing::error!(target: "personal", error = %other, "personalization store error");
            "Your change couldn't be saved. Please try again.".into()
        }
    }
}

/// An empty or blank misspelling means a vocabulary word.
fn entry(word: String, misspelling: Option<String>) -> Entry {
    Entry {
        word,
        misspelling: misspelling.filter(|m| !m.trim().is_empty()),
    }
}

/// Run a change against the store, then tell the window.
fn change<R: Runtime, T>(
    app: &AppHandle<R>,
    f: impl FnOnce(&Store, i64) -> Result<T, PersonalError>,
) -> Result<T, String> {
    let engine = engine(app)?;
    let out = f(&engine.history.lock(), now_ms()).map_err(|e| error_message(&e))?;
    let _ = app.emit_to("main", CHANGED_EVENT, ());
    Ok(out)
}

#[tauri::command]
pub fn dictionary_list<R: Runtime>(app: AppHandle<R>) -> Result<Vec<DictionaryEntry>, String> {
    engine(&app)?.history.lock().dictionary().map_err(|e| store_message(&e))
}

#[tauri::command]
pub fn dictionary_add<R: Runtime>(
    app: AppHandle<R>,
    word: String,
    misspelling: Option<String>,
) -> Result<DictionaryEntry, String> {
    change(&app, |s, now| s.dictionary_add(entry(word, misspelling), now))
}

#[tauri::command]
pub fn dictionary_update<R: Runtime>(
    app: AppHandle<R>,
    id: String,
    word: String,
    misspelling: Option<String>,
) -> Result<DictionaryEntry, String> {
    change(&app, |s, now| s.dictionary_update(&id, entry(word, misspelling), now))
}

#[tauri::command]
pub fn dictionary_delete<R: Runtime>(app: AppHandle<R>, id: String) -> Result<(), String> {
    change(&app, |s, now| Ok(s.dictionary_delete(&id, now)?))
}

/// "Delete all words". Returns how many were removed.
#[tauri::command]
pub fn dictionary_delete_all<R: Runtime>(app: AppHandle<R>) -> Result<usize, String> {
    change(&app, |s, now| Ok(s.dictionary_delete_all(now)?))
}

#[tauri::command]
pub fn snippets_list<R: Runtime>(app: AppHandle<R>) -> Result<Vec<SnippetEntry>, String> {
    engine(&app)?.history.lock().snippets().map_err(|e| store_message(&e))
}

#[tauri::command]
pub fn snippets_add<R: Runtime>(app: AppHandle<R>, trigger: String, content: String) -> Result<SnippetEntry, String> {
    change(&app, |s, now| s.snippet_add(Snippet { trigger, content }, now))
}

#[tauri::command]
pub fn snippets_update<R: Runtime>(
    app: AppHandle<R>,
    id: String,
    trigger: String,
    content: String,
) -> Result<SnippetEntry, String> {
    change(&app, |s, now| s.snippet_update(&id, Snippet { trigger, content }, now))
}

#[tauri::command]
pub fn snippets_delete<R: Runtime>(app: AppHandle<R>, id: String) -> Result<(), String> {
    change(&app, |s, now| Ok(s.snippet_delete(&id, now)?))
}

/// The personalization part of a dictation's start settings.
#[derive(Debug, Default, PartialEq)]
pub struct Personal {
    pub dictionary: Vec<Entry>,
    pub snippets: Vec<Snippet>,
    pub tags: TagContext,
}

/// `{datetime}` is the local date and time as `YYYY-MM-DD HH:MM` (24-hour),
/// readable in every locale. `{timezone}` is the IANA zone name when the OS
/// reports one, otherwise the UTC offset.
pub fn tag_values<Tz: TimeZone>(now: &DateTime<Tz>, zone: Option<String>) -> (String, String)
where
    Tz::Offset: std::fmt::Display,
{
    let datetime = now.format("%Y-%m-%d %H:%M").to_string();
    let timezone = zone.filter(|z| !z.trim().is_empty()).unwrap_or_else(|| {
        let secs = now.offset().fix().local_minus_utc();
        let (sign, secs) = if secs < 0 { ('-', -secs) } else { ('+', secs) };
        format!("UTC{sign}{:02}:{:02}", secs / 3600, secs % 3600 / 60)
    });
    (datetime, timezone)
}

/// True when some correction or snippet would expand `{clipboard}`.
pub fn needs_clipboard(dictionary: &[Entry], snippets: &[Snippet]) -> bool {
    dictionary
        .iter()
        .any(|e| e.is_correction() && e.word.contains(TAG_CLIPBOARD))
        || snippets.iter().any(|s| s.content.contains(TAG_CLIPBOARD))
}

/// Assemble the dictionary, snippets and tag values. The clipboard is read only
/// when an entry or snippet actually uses `{clipboard}`, so dictations never
/// touch it otherwise.
pub fn assemble(
    dictionary: Vec<Entry>,
    snippets: Vec<Snippet>,
    datetime: String,
    timezone: String,
    read_clipboard: impl FnOnce() -> Option<String>,
) -> Personal {
    let clipboard = if needs_clipboard(&dictionary, &snippets) {
        read_clipboard()
    } else {
        None
    };
    Personal {
        dictionary,
        snippets,
        tags: TagContext {
            clipboard,
            datetime,
            timezone,
        },
    }
}

/// Read the store and the clock for a dictation that starts now. A store
/// error leaves the lists empty: dictation still works, just without them.
pub fn load(store: &Store, read_clipboard: impl FnOnce() -> Option<String>) -> Personal {
    let dictionary = store
        .dictionary()
        .map(|v| v.iter().map(DictionaryEntry::entry).collect())
        .unwrap_or_else(|e| {
            tracing::warn!(target: "personal", error = %e, "dictionary not loaded");
            Vec::new()
        });
    let snippets = store
        .snippets()
        .map(|v| v.iter().map(SnippetEntry::snippet).collect())
        .unwrap_or_else(|e| {
            tracing::warn!(target: "personal", error = %e, "snippets not loaded");
            Vec::new()
        });
    let (datetime, timezone) = tag_values(&Local::now(), iana_time_zone::get_timezone().ok());
    assemble(dictionary, snippets, datetime, timezone, read_clipboard)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::FixedOffset;
    use std::cell::Cell;

    fn sn(trigger: &str, content: &str) -> Snippet {
        Snippet {
            trigger: trigger.into(),
            content: content.into(),
        }
    }

    #[test]
    fn duplicate_and_validation_messages() {
        assert_eq!(error_message(&EntryError::Duplicate.into()), "Duplicate entry");
        assert!(error_message(&EntryError::WordTooLong.into()).contains("50"));
        assert!(error_message(&SnippetError::TriggerTooLong.into()).contains("200"));
        assert_eq!(
            error_message(&PersonalError::Store(StoreError::NotFound)),
            "That item was already deleted."
        );
        // Database internals never reach the page.
        let io = StoreError::Io(std::io::Error::other("disk /secret/path full"));
        assert!(!error_message(&PersonalError::Store(io)).contains("secret"));
    }

    #[test]
    fn blank_misspelling_is_vocabulary() {
        assert_eq!(entry("Vocalis".into(), Some("  ".into())), Entry::vocab("Vocalis"));
        assert_eq!(
            entry("PostgreSQL".into(), Some("postgres".into())),
            Entry::correction("postgres", "PostgreSQL")
        );
    }

    #[test]
    fn tag_values_format_and_fallback() {
        let berlin = FixedOffset::east_opt(3600).unwrap();
        let t = berlin.with_ymd_and_hms(2026, 11, 28, 9, 5, 0).unwrap();
        assert_eq!(
            tag_values(&t, Some("Europe/Berlin".into())),
            ("2026-11-28 09:05".into(), "Europe/Berlin".into())
        );
        assert_eq!(tag_values(&t, None).1, "UTC+01:00");
        let india = FixedOffset::west_opt(-(5 * 3600 + 1800)).unwrap();
        let t = india.with_ymd_and_hms(2026, 1, 2, 23, 59, 0).unwrap();
        assert_eq!(tag_values(&t, Some(" ".into())).1, "UTC+05:30");
        let ny = FixedOffset::west_opt(5 * 3600).unwrap();
        assert_eq!(
            tag_values(&ny.with_ymd_and_hms(2026, 1, 2, 0, 0, 0).unwrap(), None).1,
            "UTC-05:00"
        );
    }

    #[test]
    fn clipboard_is_read_only_when_used() {
        let reads = Cell::new(0);
        let read = || {
            reads.set(reads.get() + 1);
            Some("copied".to_owned())
        };
        let p = assemble(
            vec![Entry::vocab("Vocalis"), Entry::correction("now", "{datetime}")],
            vec![sn("stamp", "At {timezone}")],
            "2026-11-28 09:05".into(),
            "Europe/Berlin".into(),
            read,
        );
        assert_eq!(reads.get(), 0);
        assert_eq!(p.tags.clipboard, None);
        assert_eq!(p.tags.datetime, "2026-11-28 09:05");
        assert_eq!(p.tags.timezone, "Europe/Berlin");
        assert_eq!(p.dictionary.len(), 2);
        assert_eq!(p.snippets.len(), 1);

        // A correction that pastes the clipboard.
        let p = assemble(
            vec![Entry::correction("paste that here", "{clipboard}")],
            vec![],
            String::new(),
            String::new(),
            read,
        );
        assert_eq!((reads.get(), p.tags.clipboard.as_deref()), (1, Some("copied")));

        // A snippet that uses it.
        let p = assemble(
            vec![],
            vec![sn("link", "See {clipboard}")],
            String::new(),
            String::new(),
            read,
        );
        assert_eq!((reads.get(), p.tags.clipboard.as_deref()), (2, Some("copied")));
    }

    #[test]
    fn tag_in_trigger_or_misspelling_does_not_read_clipboard() {
        // Only replacements and snippet content are expanded.
        assert!(!needs_clipboard(
            &[Entry::correction("{clipboard}", "word")],
            &[sn("{clipboard}", "text")]
        ));
        // A tagged vocabulary word is invalid and never expanded.
        assert!(!needs_clipboard(&[Entry::vocab("{clipboard}")], &[]));
    }

    #[test]
    fn load_reads_store_in_match_order() {
        let dir = tempfile::tempdir().unwrap();
        let (store, _) = Store::open(&dir.path().join("h.db"), &[3; 32]).unwrap();
        store.dictionary_add(Entry::vocab("Vocalis"), 1).unwrap();
        store
            .dictionary_add(Entry::correction("paste it", "{clipboard}"), 2)
            .unwrap();
        store.snippet_add(sn("sig", "first"), 1).unwrap();
        store.snippet_add(sn("sig", "second"), 2).unwrap();
        let p = load(&store, || Some("clip".into()));
        assert_eq!(p.dictionary.len(), 2);
        assert_eq!(p.snippets[0].content, "first");
        assert_eq!(p.tags.clipboard.as_deref(), Some("clip"));
        assert!(!p.tags.datetime.is_empty());
        assert!(!p.tags.timezone.is_empty());
    }
}
