//! Dictionary auto-suggestions (spec 4.5.1, ledger L-010), on every OS.
//!
//! After a dictation is pasted, the desktop may later learn what the user left
//! in the field (by reading the focused field, or from an edit the user makes
//! in History). [`detect_fixes`] compares the two at word level and returns
//! the one-word replacements that look like spelling fixes. [`SuggestionState`]
//! counts them per `misspelling → word` pair; once the same pair has been
//! fixed [`FIXES_TO_SUGGEST`] times within [`OBSERVATION_TTL_MS`] (and the
//! word has at least [`MIN_WORD_CHARS`] characters), it becomes a suggestion
//! to add a correction to the dictionary.
//!
//! **Privacy.** The state keeps only the two words of each pair and the times
//! the fix was seen. No transcript, sentence, app name or surrounding text is
//! kept, so the state can be stored as a plain JSON file. Callers must not log
//! the texts passed to [`detect_fixes`].

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::dictionary::{self, Entry, MAX_ENTRY_CHARS};
use crate::text::char_len;

/// Fixes of the same pair needed before we suggest it.
pub const FIXES_TO_SUGGEST: usize = 2;
/// The corrected word must be at least this long.
pub const MIN_WORD_CHARS: usize = 4;
/// Observations older than this are forgotten.
pub const OBSERVATION_TTL_MS: i64 = 30 * 24 * 60 * 60 * 1000;
/// Longest text compared; longer fields are ignored rather than diffed slowly.
const MAX_WORDS: usize = 2_000;
/// Fewer than this share of the pasted words left unchanged means the user
/// rewrote the text rather than fixing words in it.
const MIN_KEPT_SHARE: f64 = 0.5;
/// Pairs remembered at most (oldest dropped first).
const MAX_PAIRS: usize = 500;
/// Times kept per pair.
const MAX_TIMES: usize = 20;

/// One word the user changed: `from` (what we pasted) → `to` (what they typed).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Fix {
    pub from: String,
    pub to: String,
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '\''
}

fn words(s: &str) -> Vec<&str> {
    s.split(|c: char| !is_word_char(c))
        .map(|w| w.trim_matches('\''))
        .filter(|w| !w.is_empty())
        .collect()
}

/// Levenshtein distance over chars (lowercased by the caller).
fn distance(a: &[char], b: &[char]) -> usize {
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let sub = prev[j] + usize::from(ca != cb);
            cur[j + 1] = sub.min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// A plausible spelling fix: a case-only change, or close in spelling
/// (at most 40% of the longer word's characters differ; at least one edit is allowed).
fn looks_like_fix(from: &str, to: &str) -> bool {
    if from == to || char_len(to) < MIN_WORD_CHARS || char_len(to) > MAX_ENTRY_CHARS || char_len(from) > MAX_ENTRY_CHARS
    {
        return false;
    }
    if to.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    let a: Vec<char> = from.chars().flat_map(char::to_lowercase).collect();
    let b: Vec<char> = to.chars().flat_map(char::to_lowercase).collect();
    let longest = a.len().max(b.len());
    let allowed = ((longest as f64) * 0.4).floor().max(1.0) as usize;
    distance(&a, &b) <= allowed
}

/// Word-level fixes between the text we pasted and the text the user left.
///
/// Only replacements count: in each changed run of at most three pasted
/// words, a removed word is paired (in order) with an added word that is
/// spelled like it. Purely inserted or deleted words, rewrites (less than
/// half of the pasted words kept) and changes to unrelated words are ignored.
/// `edited` may contain other text around the pasted part. Each pair is
/// reported once per call.
pub fn detect_fixes(pasted: &str, edited: &str) -> Vec<Fix> {
    let a = words(pasted);
    let b = words(edited);
    if a.is_empty() || b.is_empty() || a.len() > MAX_WORDS || b.len() > MAX_WORDS {
        return Vec::new();
    }
    // LCS table over exact word matches.
    let (n, m) = (a.len(), b.len());
    let mut lcs = vec![0u32; (n + 1) * (m + 1)];
    let at = |i: usize, j: usize| i * (m + 1) + j;
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[at(i, j)] = if a[i] == b[j] {
                lcs[at(i + 1, j + 1)] + 1
            } else {
                lcs[at(i + 1, j)].max(lcs[at(i, j + 1)])
            };
        }
    }
    let kept = lcs[at(0, 0)] as f64;
    if kept < (n as f64) * MIN_KEPT_SHARE {
        return Vec::new();
    }
    // Walk the alignment, collecting hunks of (removed, added) words.
    let mut fixes = Vec::new();
    let mut seen = HashSet::new();
    let (mut i, mut j) = (0, 0);
    let mut removed: Vec<&str> = Vec::new();
    let mut added: Vec<&str> = Vec::new();
    let mut flush = |removed: &mut Vec<&str>, added: &mut Vec<&str>, fixes: &mut Vec<Fix>| {
        if !removed.is_empty() && removed.len() <= 3 {
            // Pair removed words with added ones in order; extra added words
            // (text the user typed around the paste) are passed over.
            let mut next = 0;
            for f in removed.iter() {
                if let Some(k) = (next..added.len()).find(|&k| looks_like_fix(f, added[k])) {
                    next = k + 1;
                    let fix = Fix {
                        from: (*f).to_owned(),
                        to: added[k].to_owned(),
                    };
                    if seen.insert(fix.clone()) {
                        fixes.push(fix);
                    }
                }
            }
        }
        removed.clear();
        added.clear();
    };
    while i < n || j < m {
        if i < n && j < m && a[i] == b[j] {
            flush(&mut removed, &mut added, &mut fixes);
            i += 1;
            j += 1;
        } else if j < m && (i == n || lcs[at(i, j + 1)] >= lcs[at(i + 1, j)]) {
            added.push(b[j]);
            j += 1;
        } else {
            removed.push(a[i]);
            i += 1;
        }
    }
    flush(&mut removed, &mut added, &mut fixes);
    fixes
}

/// Times one pair was fixed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Observation {
    /// The misspelling as it was pasted (lowercased).
    pub from: String,
    /// The user's spelling, as typed.
    pub to: String,
    /// Unix ms of each fix, oldest first.
    pub seen: Vec<i64>,
}

/// A pair ready to offer: "add `to` (misspelled as `from`)".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Suggestion {
    pub from: String,
    pub to: String,
    pub count: usize,
    pub last_seen: i64,
}

impl Suggestion {
    /// The dictionary entry accepting this suggestion adds.
    pub fn entry(&self) -> Entry {
        Entry::correction(self.from.clone(), self.to.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Dismissed {
    pub from: String,
    pub to: String,
}

/// Everything remembered about suggestions; safe to persist as plain JSON.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SuggestionState {
    pub version: u32,
    pub observations: Vec<Observation>,
    /// Pairs the user said no to; never suggested again.
    pub dismissed: Vec<Dismissed>,
}

fn same(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

/// The dictionary already covers this pair: the word is there (as vocabulary
/// or a correction's target) or the misspelling is already corrected.
pub fn in_dictionary(from: &str, to: &str, dictionary: &[Entry]) -> bool {
    dictionary
        .iter()
        .any(|e| same(e.word.trim(), to) || e.misspelling.as_deref().is_some_and(|m| same(m.trim(), from)))
}

impl SuggestionState {
    pub const VERSION: u32 = 1;

    pub fn new() -> Self {
        Self {
            version: Self::VERSION,
            ..Default::default()
        }
    }

    fn is_dismissed(&self, from: &str, to: &str) -> bool {
        self.dismissed.iter().any(|d| same(&d.from, from) && same(&d.to, to))
    }

    /// Forget fixes older than 30 days.
    pub fn prune(&mut self, now: i64) {
        let cutoff = now - OBSERVATION_TTL_MS;
        for o in &mut self.observations {
            o.seen.retain(|&t| t > cutoff);
        }
        self.observations.retain(|o| !o.seen.is_empty());
    }

    /// Record fixes seen at `now`. Returns the suggestions that became ready
    /// because of these fixes (so the desktop can notify once).
    pub fn observe(&mut self, fixes: &[Fix], now: i64, dictionary: &[Entry]) -> Vec<Suggestion> {
        self.prune(now);
        let mut ready = Vec::new();
        for fix in fixes {
            if !looks_like_fix(&fix.from, &fix.to)
                || self.is_dismissed(&fix.from, &fix.to)
                || in_dictionary(&fix.from, &fix.to, dictionary)
            {
                continue;
            }
            let from = fix.from.to_lowercase();
            let idx = match self
                .observations
                .iter()
                .position(|o| o.from == from && same(&o.to, &fix.to))
            {
                Some(i) => i,
                None => {
                    self.observations.push(Observation {
                        from: from.clone(),
                        to: fix.to.clone(),
                        seen: Vec::new(),
                    });
                    self.observations.len() - 1
                }
            };
            let o = &mut self.observations[idx];
            // Prefer the spelling typed mid-sentence: a capital after a sentence start says nothing.
            if fix.from.chars().next().is_some_and(char::is_lowercase) {
                o.to = fix.to.clone();
            }
            let before = o.seen.len();
            o.seen.push(now);
            if o.seen.len() > MAX_TIMES {
                o.seen.remove(0);
            }
            if before + 1 == FIXES_TO_SUGGEST {
                ready.push(Suggestion {
                    from: o.from.clone(),
                    to: o.to.clone(),
                    count: o.seen.len(),
                    last_seen: now,
                });
            }
        }
        if self.observations.len() > MAX_PAIRS {
            self.observations
                .sort_by_key(|o| std::cmp::Reverse(*o.seen.last().unwrap_or(&0)));
            self.observations.truncate(MAX_PAIRS);
        }
        ready
    }

    /// Suggestions to show now, most recent first.
    pub fn pending(&self, now: i64, dictionary: &[Entry]) -> Vec<Suggestion> {
        let cutoff = now - OBSERVATION_TTL_MS;
        let mut out: Vec<Suggestion> = self
            .observations
            .iter()
            .filter_map(|o| {
                let seen: Vec<i64> = o.seen.iter().copied().filter(|&t| t > cutoff).collect();
                (seen.len() >= FIXES_TO_SUGGEST
                    && !self.is_dismissed(&o.from, &o.to)
                    && !in_dictionary(&o.from, &o.to, dictionary))
                .then(|| Suggestion {
                    from: o.from.clone(),
                    to: o.to.clone(),
                    count: seen.len(),
                    last_seen: *seen.last().unwrap(),
                })
            })
            .collect();
        out.sort_by_key(|s| std::cmp::Reverse(s.last_seen));
        out
    }

    /// Never suggest this pair again, and forget its observations.
    pub fn dismiss(&mut self, from: &str, to: &str) {
        self.observations.retain(|o| !(same(&o.from, from) && same(&o.to, to)));
        if !self.is_dismissed(from, to) {
            self.dismissed.push(Dismissed {
                from: from.to_lowercase(),
                to: to.to_owned(),
            });
        }
    }

    /// The user added the pair: return the entry to save (validated against
    /// `dictionary`) and forget the observations.
    pub fn accept(&mut self, from: &str, to: &str, dictionary: &[Entry]) -> Result<Entry, dictionary::EntryError> {
        let pending = self
            .observations
            .iter()
            .find(|o| same(&o.from, from) && same(&o.to, to))
            .map(|o| Entry::correction(o.from.clone(), o.to.clone()))
            .unwrap_or_else(|| Entry::correction(from.to_lowercase(), to.to_owned()));
        dictionary::validate(&pending, dictionary, None)?;
        self.observations.retain(|o| !(same(&o.from, from) && same(&o.to, to)));
        Ok(pending)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    const DAY: i64 = 24 * 60 * 60 * 1000;

    fn fix(f: &str, t: &str) -> Fix {
        Fix {
            from: f.into(),
            to: t.into(),
        }
    }

    #[test]
    fn detects_word_fixes() {
        type Case<'a> = (&'a str, &'a str, &'a [(&'a str, &'a str)]);
        let table: &[Case] = &[
            (
                "We deploy to kubernets today.",
                "We deploy to Kubernetes today.",
                &[("kubernets", "Kubernetes")],
            ),
            (
                "ask jon about postgress",
                "ask Jon about Postgres",
                &[("postgress", "Postgres")],
            ),
            ("use github now", "use GitHub now", &[("github", "GitHub")]),
            ("same text", "same text", &[]),
            // Rewrites and unrelated substitutions are not fixes.
            ("the cat sat on the mat", "a dog lay under a rug", &[]),
            ("send the report today", "send the summary today", &[]),
            // Insertions and deletions are not fixes.
            ("send it now", "send it right now", &[]),
            // Short words are ignored.
            ("teh cat", "the cat", &[]),
            // Extra text around the paste is fine.
            (
                "call recieve",
                "Dear Sam, call receive. Thanks",
                &[("recieve", "receive")],
            ),
            // Numbers are not dictionary words.
            ("room 1234 ok", "room 1243 ok", &[]),
            ("", "anything", &[]),
        ];
        for (pasted, edited, want) in table {
            let want: Vec<Fix> = want.iter().map(|(f, t)| fix(f, t)).collect();
            assert_eq!(detect_fixes(pasted, edited), want, "{pasted:?} → {edited:?}");
        }
    }

    #[test]
    fn reports_each_pair_once_per_call() {
        assert_eq!(
            detect_fixes("recieve and recieve again", "receive and receive again"),
            vec![fix("recieve", "receive")]
        );
    }

    #[test]
    fn suggests_after_two_fixes() {
        let mut s = SuggestionState::new();
        assert!(s.observe(&[fix("kubernets", "Kubernetes")], 0, &[]).is_empty());
        assert!(s.pending(0, &[]).is_empty());
        let ready = s.observe(&[fix("Kubernets", "Kubernetes")], DAY, &[]);
        assert_eq!(
            ready,
            vec![Suggestion {
                from: "kubernets".into(),
                to: "Kubernetes".into(),
                count: 2,
                last_seen: DAY
            }]
        );
        assert_eq!(s.pending(DAY, &[]).len(), 1);
        // A third fix doesn't notify again.
        assert!(s.observe(&[fix("kubernets", "Kubernetes")], 2 * DAY, &[]).is_empty());
        assert_eq!(s.pending(2 * DAY, &[])[0].count, 3);
    }

    #[test]
    fn observations_expire_after_30_days() {
        let mut s = SuggestionState::new();
        s.observe(&[fix("recieve", "receive")], 0, &[]);
        assert!(s.observe(&[fix("recieve", "receive")], 31 * DAY, &[]).is_empty());
        assert!(s.pending(31 * DAY, &[]).is_empty());
        s.observe(&[fix("recieve", "receive")], 32 * DAY, &[]);
        assert_eq!(s.pending(32 * DAY, &[]).len(), 1);
        // And a ready suggestion lapses when its fixes age out.
        assert!(s.pending(62 * DAY, &[]).is_empty());
        s.prune(62 * DAY);
        assert!(s.observations.is_empty());
    }

    #[test]
    fn never_suggests_dictionary_words() {
        let dict = [Entry::vocab("Kubernetes")];
        let mut s = SuggestionState::new();
        for t in 0..3 {
            assert!(s.observe(&[fix("kubernets", "Kubernetes")], t, &dict).is_empty());
        }
        assert!(s.pending(3, &dict).is_empty());
        // Added to the dictionary after being observed: hidden too.
        let mut s = SuggestionState::new();
        s.observe(&[fix("recieve", "receive")], 0, &[]);
        s.observe(&[fix("recieve", "receive")], 1, &[]);
        assert!(s.pending(1, &[Entry::correction("recieve", "receive")]).is_empty());
    }

    #[test]
    fn dismissing_suppresses() {
        let mut s = SuggestionState::new();
        s.observe(&[fix("recieve", "receive")], 0, &[]);
        s.observe(&[fix("recieve", "receive")], 1, &[]);
        s.dismiss("recieve", "receive");
        assert!(s.pending(1, &[]).is_empty());
        for t in 2..6 {
            assert!(s.observe(&[fix("recieve", "receive")], t, &[]).is_empty());
        }
        assert!(s.pending(6, &[]).is_empty());
    }

    #[test]
    fn accept_returns_entry_and_forgets() {
        let mut s = SuggestionState::new();
        s.observe(&[fix("postgress", "PostgreSQL")], 0, &[]);
        s.observe(&[fix("postgress", "PostgreSQL")], 1, &[]);
        let e = s.accept("postgress", "PostgreSQL", &[]).unwrap();
        assert_eq!(e, Entry::correction("postgress", "PostgreSQL"));
        assert!(s.observations.is_empty());
        assert_eq!(
            s.accept("postgress", "PostgreSQL", &[e]),
            Err(dictionary::EntryError::Duplicate)
        );
    }

    #[test]
    fn rejects_short_or_long_words() {
        let mut s = SuggestionState::new();
        for t in 0..3 {
            s.observe(&[fix("teh", "the"), fix(&"a".repeat(51), &"b".repeat(51))], t, &[]);
        }
        assert!(s.observations.is_empty());
    }

    #[test]
    fn state_keeps_only_word_pairs() {
        let mut s = SuggestionState::new();
        let fixes = detect_fixes(
            "my secret plan is to visit kubernets tomorrow",
            "my secret plan is to visit Kubernetes tomorrow",
        );
        s.observe(&fixes, 5, &[]);
        let json = serde_json::to_string(&s).unwrap();
        assert!(!json.contains("secret") && !json.contains("tomorrow"), "{json}");
        let back: SuggestionState = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
        // Older or partial files load with defaults.
        let old: SuggestionState = serde_json::from_str("{}").unwrap();
        assert!(old.observations.is_empty());
    }

    proptest! {
        #[test]
        fn identical_text_has_no_fixes(s in "\\PC{0,200}") {
            prop_assert!(detect_fixes(&s, &s).is_empty());
        }

        #[test]
        fn fixes_come_from_both_texts(a in "[a-zA-Z ]{0,80}", b in "[a-zA-Z ]{0,80}") {
            let aw = words(&a);
            let bw = words(&b);
            for f in detect_fixes(&a, &b) {
                prop_assert!(aw.contains(&f.from.as_str()));
                prop_assert!(bw.contains(&f.to.as_str()));
                prop_assert!(char_len(&f.to) >= MIN_WORD_CHARS);
                prop_assert_ne!(&f.from, &f.to);
            }
        }

        #[test]
        fn never_panics(a in "\\PC{0,200}", b in "\\PC{0,200}") {
            let _ = detect_fixes(&a, &b);
        }
    }
}
