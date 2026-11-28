//! Importers for dictionary entries and snippets (spec 4.5.4).
//!
//! Each importer reads a file the user exported (or prepared) themselves from
//! another tool and turns it into [`Entry`] and [`Snippet`] lists for Vocalis,
//! with an [`ImportReport`] that says what will be added, what was skipped and
//! why, and what was converted on the way (for example a date placeholder with
//! a custom format becoming `{datetime}`).
//!
//! Supported files (see `docs/research/finding-ledger.md` rows IMP-01..IMP-06 for
//! where each format comes from and how sure we are about it):
//!
//! | [`Format`] | File |
//! |---|---|
//! | `Espanso` | an Espanso match file (`*.yml`): `matches:` with `trigger`/`triggers` and `replace` |
//! | `TextExpander` | a TextExpander group exported as CSV: `abbreviation,content,label` |
//! | `Alfred` | an Alfred snippet collection (`*.alfredsnippets`, a zip of JSON files) |
//! | `Raycast` | a Raycast snippets export: a JSON array of `{name, text, keyword}` |
//! | `WisprFlow` | Wispr Flow's documented bulk-import files: dictionary CSV or snippets JSON |
//! | `Superwhisper` | a plain vocabulary CSV (the vendor's own file layout is UNKNOWN to us) |
//!
//! The parsers never execute anything from the file and never read beyond it.
//! Everything here is pure: the desktop reads the file and writes the result.

mod alfred;
mod dict_csv;
mod espanso;
mod placeholders;
mod raycast;
mod textexpander;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use vocalis_core::dictionary::{self, Entry, EntryError};
use vocalis_core::snippets::{self, Snippet, SnippetError};
use vocalis_core::text::normalize_phrase;

/// Largest file we read (the biggest real exports we know of are well under 1 MB).
pub const MAX_FILE_BYTES: usize = 5 * 1024 * 1024;
/// Most items one import may produce; more is almost certainly not a snippet file.
pub const MAX_ITEMS: usize = 10_000;
/// How much of an item's text a report quotes.
const LABEL_CHARS: usize = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Format {
    Espanso,
    TextExpander,
    Alfred,
    Raycast,
    WisprFlow,
    Superwhisper,
}

impl Format {
    pub const ALL: [Format; 6] = [
        Format::WisprFlow,
        Format::Superwhisper,
        Format::Raycast,
        Format::Alfred,
        Format::Espanso,
        Format::TextExpander,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Format::Espanso => "Espanso",
            Format::TextExpander => "TextExpander",
            Format::Alfred => "Alfred",
            Format::Raycast => "Raycast",
            Format::WisprFlow => "Wispr Flow",
            Format::Superwhisper => "Superwhisper",
        }
    }
}

/// Why a whole file could not be read.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ImportError {
    #[error("the file is larger than 5 MB")]
    TooLarge,
    #[error("the file is not text in UTF-8")]
    NotText,
    #[error("couldn't tell which app this file came from; pick the format")]
    UnknownFormat,
    #[error("the file isn't a valid {format} file: {detail}")]
    Malformed { format: &'static str, detail: String },
    #[error("the file has more than {MAX_ITEMS} items")]
    TooManyItems,
}

impl ImportError {
    pub(crate) fn malformed(format: Format, detail: impl Into<String>) -> Self {
        Self::Malformed {
            format: format.label(),
            detail: detail.into(),
        }
    }
}

/// Why one item in the file was not imported.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum SkipReason {
    /// Over our length limits (50 characters for dictionary words, 200 for triggers, 10,000 for content).
    TooLong,
    /// Already in Vocalis, or earlier in the same file.
    Duplicate,
    /// Missing or empty fields, or a shape we can't use.
    Invalid { detail: String },
    /// Uses a placeholder or feature Vocalis has no equivalent for.
    Unsupported { feature: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Skipped {
    /// A short quote of the item (its trigger or word) so the user can find it.
    pub item: String,
    pub reason: SkipReason,
}

/// Something that was imported, but not exactly as written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Converted {
    pub item: String,
    pub change: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub format: Format,
    /// Dictionary entries that will be (or were) added.
    pub entries: Vec<Entry>,
    /// Snippets that will be (or were) added.
    pub snippets: Vec<Snippet>,
    pub skipped: Vec<Skipped>,
    pub converted: Vec<Converted>,
}

impl ImportReport {
    pub fn imported(&self) -> usize {
        self.entries.len() + self.snippets.len()
    }
}

/// One item a parser produced, before validation and de-duplication.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Item {
    Entry(Entry),
    Snippet(Snippet),
}

/// Parser output: candidate items plus what the parser itself already skipped or converted.
#[derive(Debug, Default)]
pub(crate) struct Parsed {
    pub items: Vec<Item>,
    pub skipped: Vec<Skipped>,
    pub converted: Vec<Converted>,
}

impl Parsed {
    pub fn skip(&mut self, item: &str, reason: SkipReason) {
        self.skipped.push(Skipped {
            item: label(item),
            reason,
        });
    }

    pub fn invalid(&mut self, item: &str, detail: impl Into<String>) {
        self.skip(item, SkipReason::Invalid { detail: detail.into() });
    }

    pub fn unsupported(&mut self, item: &str, feature: impl Into<String>) {
        self.skip(
            item,
            SkipReason::Unsupported {
                feature: feature.into(),
            },
        );
    }

    pub fn convert(&mut self, item: &str, change: impl Into<String>) {
        self.converted.push(Converted {
            item: label(item),
            change: change.into(),
        });
    }

    pub fn snippet(&mut self, trigger: String, content: String) -> Result<(), ImportError> {
        self.push(Item::Snippet(Snippet { trigger, content }))
    }

    pub fn entry(&mut self, e: Entry) -> Result<(), ImportError> {
        self.push(Item::Entry(e))
    }

    fn push(&mut self, item: Item) -> Result<(), ImportError> {
        if self.items.len() >= MAX_ITEMS {
            return Err(ImportError::TooManyItems);
        }
        self.items.push(item);
        Ok(())
    }
}

/// First line of `s`, trimmed and shortened for a report.
pub(crate) fn label(s: &str) -> String {
    let line = s.trim().lines().next().unwrap_or("").trim();
    let mut out: String = line.chars().take(LABEL_CHARS).collect();
    if line.chars().count() > LABEL_CHARS || s.trim().lines().nth(1).is_some() {
        out.push('…');
    }
    out
}

/// Guess the format from the file name and its first bytes. `None` when it
/// could be several (a two-column CSV could be Wispr Flow or Superwhisper).
pub fn detect(file_name: &str, data: &[u8]) -> Option<Format> {
    let name = file_name.to_lowercase();
    if name.ends_with(".alfredsnippets") || data.starts_with(b"PK\x03\x04") {
        return Some(Format::Alfred);
    }
    if name.ends_with(".yml") || name.ends_with(".yaml") {
        return Some(Format::Espanso);
    }
    let text = std::str::from_utf8(data).ok()?.trim_start_matches('\u{feff}');
    let trimmed = text.trim_start();
    if trimmed.starts_with('[') || trimmed.starts_with('{') {
        let v: serde_json::Value = serde_json::from_str(text).ok()?;
        return match &v {
            serde_json::Value::Object(o) if o.contains_key("alfredsnippet") => Some(Format::Alfred),
            serde_json::Value::Array(a) => {
                let has = |k: &str| a.iter().any(|x| x.get(k).is_some());
                if has("keyword") {
                    Some(Format::Raycast)
                } else if has("name") && has("text") {
                    Some(Format::WisprFlow)
                } else {
                    None
                }
            }
            _ => None,
        };
    }
    if trimmed.starts_with("matches:") || text.lines().any(|l| l.trim_start().starts_with("- trigger:")) {
        return Some(Format::Espanso);
    }
    if name.ends_with(".csv") || name.ends_with(".txt") || !trimmed.is_empty() {
        return dict_csv::guess_csv(text);
    }
    None
}

/// Parse `data` as `format`, then validate and de-duplicate every item against
/// the user's current dictionary and snippets (and against earlier items in the
/// same file). The report's `entries` and `snippets` are exactly what to add.
pub fn import(
    format: Format,
    data: &[u8],
    existing_entries: &[Entry],
    existing_snippets: &[Snippet],
) -> Result<ImportReport, ImportError> {
    if data.len() > MAX_FILE_BYTES {
        return Err(ImportError::TooLarge);
    }
    let parsed = match format {
        Format::Alfred => alfred::parse(data)?,
        _ => {
            let text = std::str::from_utf8(data).map_err(|_| ImportError::NotText)?;
            let text = text.trim_start_matches('\u{feff}');
            match format {
                Format::Espanso => espanso::parse(text)?,
                Format::TextExpander => textexpander::parse(text)?,
                Format::Raycast => raycast::parse(text)?,
                Format::WisprFlow => dict_csv::parse_wispr(text)?,
                Format::Superwhisper => dict_csv::parse_superwhisper(text)?,
                Format::Alfred => unreachable!(),
            }
        }
    };
    Ok(finalize(format, parsed, existing_entries, existing_snippets))
}

/// Detect the format (unless given) and import.
pub fn import_auto(
    format: Option<Format>,
    file_name: &str,
    data: &[u8],
    existing_entries: &[Entry],
    existing_snippets: &[Snippet],
) -> Result<ImportReport, ImportError> {
    let format = format
        .or_else(|| detect(file_name, data))
        .ok_or(ImportError::UnknownFormat)?;
    import(format, data, existing_entries, existing_snippets)
}

fn tidy_entry(e: Entry) -> Entry {
    Entry {
        word: e.word.trim().to_owned(),
        misspelling: e.misspelling.map(|m| m.trim().to_owned()).filter(|m| !m.is_empty()),
    }
}

fn finalize(format: Format, parsed: Parsed, existing_entries: &[Entry], existing_snippets: &[Snippet]) -> ImportReport {
    let Parsed {
        items,
        mut skipped,
        converted,
    } = parsed;
    let mut known: Vec<Entry> = existing_entries.to_vec();
    let mut triggers: std::collections::HashSet<String> =
        existing_snippets.iter().map(|s| normalize_phrase(&s.trigger)).collect();
    let mut entries = Vec::new();
    let mut snips = Vec::new();
    for item in items {
        match item {
            Item::Entry(e) => {
                let e = tidy_entry(e);
                let what = match &e.misspelling {
                    Some(m) => format!("{m} → {}", e.word),
                    None => e.word.clone(),
                };
                match dictionary::validate(&e, &known, None) {
                    Ok(()) => {
                        known.push(e.clone());
                        entries.push(e);
                    }
                    Err(err) => skipped.push(Skipped {
                        item: label(&what),
                        reason: entry_reason(err),
                    }),
                }
            }
            Item::Snippet(s) => {
                let s = Snippet {
                    trigger: s.trigger.trim().to_owned(),
                    content: s.content,
                };
                let reason = match snippets::validate(&s) {
                    Err(err) => Some(snippet_reason(err)),
                    Ok(()) if !triggers.insert(normalize_phrase(&s.trigger)) => Some(SkipReason::Duplicate),
                    Ok(()) => None,
                };
                match reason {
                    Some(reason) => skipped.push(Skipped {
                        item: label(&s.trigger),
                        reason,
                    }),
                    None => snips.push(s),
                }
            }
        }
    }
    ImportReport {
        format,
        entries,
        snippets: snips,
        skipped,
        converted,
    }
}

fn entry_reason(e: EntryError) -> SkipReason {
    match e {
        EntryError::WordTooLong | EntryError::MisspellingTooLong => SkipReason::TooLong,
        EntryError::Duplicate => SkipReason::Duplicate,
        EntryError::EmptyWord => SkipReason::Invalid {
            detail: "the word is empty".into(),
        },
        EntryError::TagRequiresCorrection => SkipReason::Invalid {
            detail: "a word with a placeholder needs a misspelling".into(),
        },
    }
}

fn snippet_reason(e: SnippetError) -> SkipReason {
    match e {
        SnippetError::TriggerTooLong | SnippetError::ContentTooLong => SkipReason::TooLong,
        SnippetError::EmptyTrigger => SkipReason::Invalid {
            detail: "the trigger has no words".into(),
        },
        SnippetError::EmptyContent => SkipReason::Invalid {
            detail: "the content is empty".into(),
        },
    }
}

#[cfg(test)]
mod tests;
