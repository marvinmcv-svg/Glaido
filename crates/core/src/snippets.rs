//! Snippets: a trigger phrase that expands to saved content (spec 4.5.2).
//!
//! A snippet fires only when the *entire* dictation equals its trigger,
//! ignoring case and punctuation at either end. The first matching snippet wins.
//! Snippets never apply to commands (the caller routes commands first).

use serde::{Deserialize, Serialize};

use crate::tags::{self, TagContext};
use crate::text::normalize_phrase;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snippet {
    pub trigger: String,
    pub content: String,
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
    fn renders_tags() {
        let ctx = TagContext {
            datetime: "today".into(),
            ..Default::default()
        };
        assert_eq!(render(&s("stamp", "Sent {datetime}"), &ctx), "Sent today");
    }
}
