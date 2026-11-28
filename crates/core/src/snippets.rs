//! Snippets: a trigger phrase that expands to saved content (spec 4.5.2).
//!
//! A snippet fires only when the *entire* dictation equals its trigger,
//! ignoring case and punctuation at either end. The first matching snippet wins.
//! Snippets never apply to commands (the caller routes commands first).

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::tags::{self, TagContext};
use crate::text::{char_len, normalize_phrase};

/// Maximum characters in a trigger (matches the server's column check).
pub const MAX_TRIGGER_CHARS: usize = 200;
/// Maximum characters in a snippet's content.
pub const MAX_CONTENT_CHARS: usize = 10_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snippet {
    pub trigger: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SnippetError {
    #[error("the trigger has no words")]
    EmptyTrigger,
    #[error("the trigger is longer than {MAX_TRIGGER_CHARS} characters")]
    TriggerTooLong,
    #[error("the content is empty")]
    EmptyContent,
    #[error("the content is longer than {MAX_CONTENT_CHARS} characters")]
    ContentTooLong,
}

/// Check a new or edited snippet. Shared triggers are allowed (the first wins),
/// so there is no duplicate check; the UI warns instead.
pub fn validate(s: &Snippet) -> Result<(), SnippetError> {
    if normalize_phrase(&s.trigger).is_empty() {
        return Err(SnippetError::EmptyTrigger);
    }
    if char_len(s.trigger.trim()) > MAX_TRIGGER_CHARS {
        return Err(SnippetError::TriggerTooLong);
    }
    if s.content.trim().is_empty() {
        return Err(SnippetError::EmptyContent);
    }
    if char_len(&s.content) > MAX_CONTENT_CHARS {
        return Err(SnippetError::ContentTooLong);
    }
    Ok(())
}

/// Find the first snippet whose trigger equals the whole transcript.
pub fn find_match<'a>(transcript: &str, snippets: &'a [Snippet]) -> Option<&'a Snippet> {
    let needle = normalize_phrase(transcript);
    if needle.is_empty() {
        return None;
    }
    snippets.iter().find(|s| normalize_phrase(&s.trigger) == needle)
}

/// Expand a matched snippet's content, including dynamic tags.
pub fn render(snippet: &Snippet, ctx: &TagContext) -> String {
    tags::expand(&snippet.content, ctx)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(t: &str, c: &str) -> Snippet {
        Snippet {
            trigger: t.into(),
            content: c.into(),
        }
    }

    #[test]
    fn exact_match_ignoring_case_and_edge_punctuation() {
        let list = [s("my address", "1 Main St\nSpringfield")];
        assert_eq!(
            find_match("My address.", &list).unwrap().content,
            "1 Main St\nSpringfield"
        );
        assert!(find_match("send my address", &list).is_none());
        assert!(find_match("my address please", &list).is_none());
    }

    #[test]
    fn first_wins_on_duplicate_triggers() {
        let list = [s("sig", "first"), s("SIG!", "second")];
        assert_eq!(find_match("sig", &list).unwrap().content, "first");
    }

    #[test]
    fn empty_transcript_never_matches() {
        assert!(find_match("  ...  ", &[s("...", "x")]).is_none());
    }

    #[test]
    fn validation_rules() {
        assert_eq!(validate(&s("my address", "1 Main St")), Ok(()));
        assert_eq!(validate(&s(" ?! ", "x")), Err(SnippetError::EmptyTrigger));
        assert_eq!(validate(&s(&"t".repeat(201), "x")), Err(SnippetError::TriggerTooLong));
        assert_eq!(validate(&s("sig", " \n ")), Err(SnippetError::EmptyContent));
        assert_eq!(
            validate(&s("sig", &"c".repeat(10_001))),
            Err(SnippetError::ContentTooLong)
        );
    }

    #[test]
    fn renders_tags() {
        let ctx = TagContext {
            datetime: "today".into(),
            ..Default::default()
        };
        assert_eq!(render(&s("stamp", "Sent {datetime}"), &ctx), "Sent today");
    }
}
