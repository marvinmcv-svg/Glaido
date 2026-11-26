//! Personal dictionary (spec 4.5.1): vocabulary hints and corrections.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::tags::{self, TagContext};
use crate::text::char_len;

/// Maximum characters for a word and for a misspelling.
pub const MAX_ENTRY_CHARS: usize = 50;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// The correct spelling (or the replacement, which may contain tags).
    pub word: String,
    /// When set, the entry is a correction: `misspelling` → `word`.
    pub misspelling: Option<String>,
}

impl Entry {
    pub fn vocab(word: impl Into<String>) -> Self {
        Self {
            word: word.into(),
            misspelling: None,
        }
    }

    pub fn correction(misspelling: impl Into<String>, word: impl Into<String>) -> Self {
        Self {
            word: word.into(),
            misspelling: Some(misspelling.into()),
        }
    }

    pub fn is_correction(&self) -> bool {
        self.misspelling.as_deref().is_some_and(|m| !m.trim().is_empty())
    }

    /// Entries containing a tag are corrections only and never recognizer hints.
    pub fn has_tag(&self) -> bool {
        tags::contains_tag(&self.word)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EntryError {
    #[error("the word is empty")]
    EmptyWord,
    #[error("the word is longer than {MAX_ENTRY_CHARS} characters")]
    WordTooLong,
    #[error("the misspelling is longer than {MAX_ENTRY_CHARS} characters")]
    MisspellingTooLong,
    #[error("an entry containing a tag must be a correction")]
    TagRequiresCorrection,
    #[error("this entry already exists")]
    Duplicate,
}

fn key(e: &Entry) -> (String, String) {
    (
        e.word.trim().to_lowercase(),
        e.misspelling
            .as_deref()
            .map(|m| m.trim().to_lowercase())
            .unwrap_or_default(),
    )
}

/// Validate a new or edited entry against the existing list.
/// `editing` is the index of the entry being edited, which is excluded from the duplicate check.
pub fn validate(entry: &Entry, existing: &[Entry], editing: Option<usize>) -> Result<(), EntryError> {
    if entry.word.trim().is_empty() {
        return Err(EntryError::EmptyWord);
    }
    if char_len(entry.word.trim()) > MAX_ENTRY_CHARS {
        return Err(EntryError::WordTooLong);
    }
    if entry
        .misspelling
        .as_deref()
        .is_some_and(|m| char_len(m.trim()) > MAX_ENTRY_CHARS)
    {
        return Err(EntryError::MisspellingTooLong);
    }
    if entry.has_tag() && !entry.is_correction() {
        return Err(EntryError::TagRequiresCorrection);
    }
    let k = key(entry);
    if existing
        .iter()
        .enumerate()
        .any(|(i, e)| Some(i) != editing && key(e) == k)
    {
        return Err(EntryError::Duplicate);
    }
    Ok(())
}

/// Words to pass to the recognizer as vocabulary hints. Tagged entries are excluded;
/// corrections contribute their correct spelling. Deduplicated, order preserved.
pub fn vocab_hints(entries: &[Entry]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    entries
        .iter()
        .filter(|e| !e.has_tag())
        .map(|e| e.word.trim().to_owned())
        .filter(|w| !w.is_empty() && seen.insert(w.to_lowercase()))
        .collect()
}

/// Apply corrections to text: whole-word (or whole-phrase), case-insensitive matches of the
/// misspelling are replaced with the correct word. Tags in the replacement are expanded.
/// The replacement keeps its own casing, except that a match starting with a capital letter
/// capitalizes a replacement that starts lowercase (so sentence starts survive).
/// Longer misspellings are applied first so phrases win over single words.
pub fn apply_corrections(text: &str, entries: &[Entry], ctx: &TagContext) -> String {
    let mut corrections: Vec<(&str, &str)> = entries
        .iter()
        .filter(|e| e.is_correction())
        .map(|e| (e.misspelling.as_deref().unwrap().trim(), e.word.trim()))
        .filter(|(m, _)| !m.is_empty())
        .collect();
    corrections.sort_by_key(|(m, _)| std::cmp::Reverse(char_len(m)));

    // Single left-to-right scan so replaced text is never re-matched.
    let chars: Vec<char> = text.chars().collect();
    let lowered: Vec<Vec<char>> = corrections
        .iter()
        .map(|(m, _)| m.chars().flat_map(char::to_lowercase).collect())
        .collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    'scan: while i < chars.len() {
        let at_boundary = i == 0 || !is_word_char(chars[i - 1]);
        if at_boundary {
            for (ci, (_, replacement)) in corrections.iter().enumerate() {
                if let Some(len) = match_at(&chars, i, &lowered[ci]) {
                    let end = i + len;
                    if end == chars.len() || !is_word_char(chars[end]) {
                        let expanded = tags::expand(replacement, ctx);
                        out.push_str(&match_case(chars[i], &expanded));
                        i = end;
                        continue 'scan;
                    }
                }
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '\''
}

/// Case-insensitive match of `needle` (already lowercased) at `chars[start..]`.
/// Returns the number of source chars consumed.
fn match_at(chars: &[char], start: usize, needle: &[char]) -> Option<usize> {
    let mut ni = 0;
    let mut si = start;
    while ni < needle.len() {
        let c = *chars.get(si)?;
        for lc in c.to_lowercase() {
            if needle.get(ni) != Some(&lc) {
                return None;
            }
            ni += 1;
        }
        si += 1;
    }
    Some(si - start)
}

fn match_case(first_source_char: char, replacement: &str) -> String {
    let mut it = replacement.chars();
    match it.next() {
        Some(f) if first_source_char.is_uppercase() && f.is_lowercase() => f.to_uppercase().chain(it).collect(),
        _ => replacement.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> TagContext {
        TagContext {
            clipboard: Some("https://example.com".into()),
            ..Default::default()
        }
    }

    #[test]
    fn replaces_whole_words_only() {
        let e = [Entry::correction("postgres", "PostgreSQL")];
        assert_eq!(
            apply_corrections("We use postgres and postgresql.", &e, &ctx()),
            "We use PostgreSQL and postgresql."
        );
    }

    #[test]
    fn case_insensitive_and_keeps_replacement_case() {
        let e = [Entry::correction("github", "GitHub")];
        assert_eq!(apply_corrections("GITHUB or Github", &e, &ctx()), "GitHub or GitHub");
    }

    #[test]
    fn capitalizes_at_sentence_start() {
        let e = [Entry::correction("kubernetes", "k8s")];
        assert_eq!(
            apply_corrections("Kubernetes is fine. kubernetes!", &e, &ctx()),
            "K8s is fine. k8s!"
        );
    }

    #[test]
    fn phrases_win_over_words_and_expand_tags() {
        let e = [
            Entry::correction("paste", "glue"),
            Entry::correction("paste that here", "{clipboard}"),
        ];
        assert_eq!(
            apply_corrections("Link: paste that here, then paste.", &e, &ctx()),
            "Link: https://example.com, then glue."
        );
    }

    #[test]
    fn replacement_is_not_rematched() {
        let e = [Entry::correction("a", "a a")];
        assert_eq!(apply_corrections("a b", &e, &ctx()), "a a b");
    }

    #[test]
    fn handles_unicode() {
        let e = [Entry::correction("strasse", "Straße")];
        assert_eq!(
            apply_corrections("Die STRASSE ist lang", &e, &ctx()),
            "Die Straße ist lang"
        );
        let e = [Entry::correction("Über", "uber")];
        assert_eq!(apply_corrections("über alles", &e, &ctx()), "uber alles");
    }

    #[test]
    fn validation_rules() {
        let existing = [Entry::vocab("Vocalis"), Entry::correction("postgres", "PostgreSQL")];
        assert_eq!(
            validate(&Entry::vocab("vocalis "), &existing, None),
            Err(EntryError::Duplicate)
        );
        assert_eq!(validate(&Entry::vocab("vocalis"), &existing, Some(0)), Ok(()));
        assert_eq!(
            validate(&Entry::correction("POSTGRES", "postgresql"), &existing, None),
            Err(EntryError::Duplicate)
        );
        assert_eq!(
            validate(&Entry::vocab("x".repeat(51)), &[], None),
            Err(EntryError::WordTooLong)
        );
        assert_eq!(validate(&Entry::vocab("é".repeat(50)), &[], None), Ok(()));
        assert_eq!(
            validate(&Entry::correction("y".repeat(51), "x"), &[], None),
            Err(EntryError::MisspellingTooLong)
        );
        assert_eq!(
            validate(&Entry::vocab("{clipboard}"), &[], None),
            Err(EntryError::TagRequiresCorrection)
        );
        assert_eq!(validate(&Entry::vocab("  "), &[], None), Err(EntryError::EmptyWord));
    }

    #[test]
    fn hints_exclude_tags_and_dedupe() {
        let e = [
            Entry::vocab("Vocalis"),
            Entry::correction("postgres", "PostgreSQL"),
            Entry::correction("here", "{clipboard}"),
            Entry::vocab("vocalis"),
        ];
        assert_eq!(vocab_hints(&e), vec!["Vocalis", "PostgreSQL"]);
    }
}
