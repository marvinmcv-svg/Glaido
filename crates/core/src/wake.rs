//! Voice activation inside a dictation (spec 4.1 "Voice activation").
//!
//! If the wake word is the first or second word of a dictation (case-insensitive,
//! punctuation ignored), the dictation is routed to Commands with everything after
//! the wake word as the request. A dictation that is only the wake word (optionally
//! preceded by one word such as "hey") opens the command window empty.

use crate::text::is_edge_punct;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WakeMatch {
    /// Route to Commands with this request (non-empty).
    Command(String),
    /// Only the wake word was said: open the command window and wait.
    NameOnly,
}

/// Check a transcript for the wake word within its first two words.
pub fn detect(transcript: &str, wake_word: &str) -> Option<WakeMatch> {
    let wake = wake_word.trim().to_lowercase();
    if wake.is_empty() {
        return None;
    }
    // Byte spans of the first two whitespace-separated tokens.
    let mut spans = Vec::with_capacity(2);
    let mut start = None;
    for (i, c) in transcript.char_indices() {
        match (c.is_whitespace(), start) {
            (false, None) => start = Some(i),
            (true, Some(s)) => {
                spans.push((s, i));
                start = None;
                if spans.len() == 2 {
                    break;
                }
            }
            _ => {}
        }
    }
    if let (Some(s), true) = (start, spans.len() < 2) {
        spans.push((s, transcript.len()));
    }

    for &(s, e) in &spans {
        let token = transcript[s..e].trim_matches(is_edge_punct).to_lowercase();
        if token == wake || strip_possessive(&token) == wake {
            let rest = transcript[e..].trim_start_matches(|c: char| c.is_whitespace() || is_edge_punct(c));
            let rest = rest.trim_end();
            return Some(if rest.chars().any(char::is_alphanumeric) {
                WakeMatch::Command(rest.to_owned())
            } else {
                WakeMatch::NameOnly
            });
        }
    }
    None
}

fn strip_possessive(t: &str) -> &str {
    t.strip_suffix("'s").or_else(|| t.strip_suffix("’s")).unwrap_or(t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use WakeMatch::*;

    const W: &str = "Vocalis";

    #[test]
    fn first_or_second_word() {
        assert_eq!(
            detect("Vocalis, translate this to French.", W),
            Some(Command("translate this to French.".into()))
        );
        assert_eq!(
            detect("hey vocalis what's 17% of 849", W),
            Some(Command("what's 17% of 849".into()))
        );
        assert_eq!(detect("OK VOCALIS: summarize", W), Some(Command("summarize".into())));
    }

    #[test]
    fn not_beyond_second_word() {
        assert_eq!(detect("I told Vocalis to wait", W), None);
        assert_eq!(detect("I love vocalis", W), None);
    }

    #[test]
    fn name_only() {
        assert_eq!(detect("Vocalis.", W), Some(NameOnly));
        assert_eq!(detect("Hey, Vocalis?", W), Some(NameOnly));
        assert_eq!(detect("  hey vocalis ...  ", W), Some(NameOnly));
    }

    #[test]
    fn no_partial_word_match() {
        assert_eq!(detect("Vocalist needed", W), None);
        assert_eq!(detect("", W), None);
    }
}
