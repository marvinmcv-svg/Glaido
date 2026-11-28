//! Small text helpers shared by the formatting stages.

/// Characters treated as "edge punctuation" when normalizing triggers and wake words.
pub fn is_edge_punct(c: char) -> bool {
    c.is_ascii_punctuation()
        || matches!(
            c,
            '…' | '“' | '”' | '‘' | '’' | '¡' | '¿' | '«' | '»' | '。' | '、' | '，' | '！' | '？'
        )
}

/// Trim whitespace, lowercase, and strip punctuation at both ends.
/// Used for exact-trigger snippet matching (spec 4.5.2).
pub fn normalize_phrase(s: &str) -> String {
    s.trim()
        .trim_matches(|c: char| c.is_whitespace() || is_edge_punct(c))
        .to_lowercase()
}

/// Count words the way quotas and stats do: whitespace-separated tokens that
/// contain at least one alphanumeric character.
pub fn count_words(s: &str) -> u32 {
    s.split_whitespace()
        .filter(|w| w.chars().any(char::is_alphanumeric))
        .count() as u32
}

/// Words per minute, rounded. Returns 0 for empty or zero-length recordings.
pub fn words_per_minute(words: u32, duration_ms: u64) -> u32 {
    if duration_ms == 0 || words == 0 {
        return 0;
    }
    ((words as f64) * 60_000.0 / duration_ms as f64).round() as u32
}

/// Number of Unicode scalar values, which is what length limits count.
pub fn char_len(s: &str) -> usize {
    s.chars().count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_edges_only() {
        assert_eq!(normalize_phrase("  My Address. "), "my address");
        assert_eq!(normalize_phrase("¿Sign-off?"), "sign-off");
        assert_eq!(normalize_phrase("e.g. this"), "e.g. this");
    }

    #[test]
    fn counts_words() {
        assert_eq!(count_words("Hello, world — again!"), 3);
        assert_eq!(count_words("   "), 0);
    }

    #[test]
    fn wpm() {
        assert_eq!(words_per_minute(30, 15_000), 120);
        assert_eq!(words_per_minute(0, 1_000), 0);
        assert_eq!(words_per_minute(5, 0), 0);
    }
}
