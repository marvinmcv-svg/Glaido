//! Pure parts of the latency harness: the utterance manifest, synthetic audio,
//! percentiles, word error rate and the §5.9 budget check.

use serde::Serialize;

/// Sample rate the gateway expects.
pub const RATE: u32 = 16_000;
/// Speaking pace used to size synthetic audio from a word count.
pub const WORDS_PER_SECOND: f64 = 2.5;
/// "1-sentence (≤ 15 words)" in §5.9.
pub const SHORT_MAX_WORDS: usize = 15;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Short,
    Long,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Utterance {
    pub id: String,
    pub kind: Kind,
    pub text: String,
}

impl Utterance {
    pub fn words(&self) -> usize {
        self.text.split_whitespace().count()
    }
}

/// Parse `id<TAB>kind<TAB>text` lines; blank lines and `#` comments are skipped.
pub fn parse_manifest(src: &str) -> Result<Vec<Utterance>, String> {
    let mut out: Vec<Utterance> = Vec::new();
    for (n, line) in src.lines().enumerate() {
        let line = line.trim_end_matches('\r');
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        let mut parts = line.splitn(3, '\t');
        let (Some(id), Some(kind), Some(text)) = (parts.next(), parts.next(), parts.next()) else {
            return Err(format!("line {}: expected id<TAB>kind<TAB>text", n + 1));
        };
        let id = id.trim();
        if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            return Err(format!("line {}: id must be letters, digits, '-' or '_'", n + 1));
        }
        let kind = match kind.trim() {
            "short" => Kind::Short,
            "long" => Kind::Long,
            other => return Err(format!("line {}: kind must be short or long, not {other:?}", n + 1)),
        };
        let text = text.trim();
        if text.is_empty() {
            return Err(format!("line {}: empty text", n + 1));
        }
        if out.iter().any(|u| u.id == id) {
            return Err(format!("line {}: duplicate id {id}", n + 1));
        }
        let u = Utterance {
            id: id.to_owned(),
            kind,
            text: text.to_owned(),
        };
        if kind == Kind::Short && u.words() > SHORT_MAX_WORDS {
            return Err(format!(
                "line {}: a short utterance has at most {SHORT_MAX_WORDS} words",
                n + 1
            ));
        }
        out.push(u);
    }
    if out.is_empty() {
        return Err("the manifest has no utterances".into());
    }
    Ok(out)
}

/// Speech-like synthetic audio for `words` words: a voiced tone with a
/// syllable-rate envelope (passes the client VAD; only meaningful with a mock
/// or stub speech provider).
pub fn synthetic_audio(words: usize) -> Vec<i16> {
    let secs = (words.max(1) as f64 / WORDS_PER_SECOND).max(0.8);
    let n = (secs * RATE as f64) as usize;
    (0..n)
        .map(|i| {
            let t = i as f64 / RATE as f64;
            let env = 0.5 + 0.5 * (2.0 * std::f64::consts::PI * 3.0 * t).sin();
            (12_000.0 * env * (2.0 * std::f64::consts::PI * 220.0 * t).sin()) as i16
        })
        .collect()
}

/// Nearest-rank percentile (`q` in 0..=1) of unsorted values; `None` if empty.
pub fn percentile(values: &[f64], q: f64) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut v = values.to_vec();
    v.sort_by(|a, b| a.total_cmp(b));
    let rank = ((q.clamp(0.0, 1.0) * v.len() as f64).ceil() as usize).max(1);
    Some(v[rank - 1])
}

/// Lowercased words with punctuation removed (apostrophes inside words kept).
pub fn normalize_words(s: &str) -> Vec<String> {
    s.split_whitespace()
        .map(|w| {
            w.chars()
                .filter(|c| c.is_alphanumeric() || *c == '\'')
                .collect::<String>()
                .trim_matches('\'')
                .to_lowercase()
        })
        .filter(|w| !w.is_empty())
        .collect()
}

/// Word error rate: word-level edit distance over the reference length.
pub fn wer(reference: &str, hypothesis: &str) -> f64 {
    let r = normalize_words(reference);
    let h = normalize_words(hypothesis);
    if r.is_empty() {
        return if h.is_empty() { 0.0 } else { 1.0 };
    }
    let mut prev: Vec<usize> = (0..=h.len()).collect();
    for (i, rw) in r.iter().enumerate() {
        let mut cur = vec![i + 1; h.len() + 1];
        for (j, hw) in h.iter().enumerate() {
            let sub = prev[j] + usize::from(rw != hw);
            cur[j + 1] = sub.min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        prev = cur;
    }
    prev[h.len()] as f64 / r.len() as f64
}

/// One measured dictation.
#[derive(Debug, Clone, Serialize)]
pub struct Sample {
    pub id: String,
    pub kind: Kind,
    pub words: usize,
    pub audio_ms: u64,
    /// Connect + start → ready.
    pub open_ms: Option<f64>,
    /// Stop sent → final received.
    pub stop_to_final_ms: Option<f64>,
    pub wer: Option<f64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Check {
    pub name: &'static str,
    pub limit_ms: f64,
    pub measured_ms: Option<f64>,
    /// `None` when there were no samples for this check.
    pub pass: Option<bool>,
}

/// The §5.9 dictation budgets for stop → result.
pub fn evaluate(samples: &[Sample]) -> Vec<Check> {
    let times = |k: Kind| -> Vec<f64> {
        samples
            .iter()
            .filter(|s| s.kind == k)
            .filter_map(|s| s.stop_to_final_ms)
            .collect()
    };
    let (short, long) = (times(Kind::Short), times(Kind::Long));
    let check = |name, limit_ms, measured: Option<f64>| Check {
        name,
        limit_ms,
        measured_ms: measured,
        pass: measured.map(|m| m < limit_ms),
    };
    vec![
        check("short p50", 1_000.0, percentile(&short, 0.5)),
        check("short p95", 2_000.0, percentile(&short, 0.95)),
        check("long (60 s) p50", 3_000.0, percentile(&long, 0.5)),
    ]
}

/// The gate passes when nothing failed outright and every measured budget holds.
pub fn gate_passes(samples: &[Sample], checks: &[Check]) -> bool {
    samples.iter().all(|s| s.error.is_none()) && checks.iter().all(|c| c.pass != Some(false))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_parses_and_validates() {
        let src = "# comment\n\na1\tshort\tHello there.\nl1\tlong\tA much longer passage.\r\n";
        let m = parse_manifest(src).unwrap();
        assert_eq!(m.len(), 2);
        assert_eq!(m[0].id, "a1");
        assert_eq!(m[1].kind, Kind::Long);
        assert_eq!(m[1].text, "A much longer passage.");

        for bad in [
            "x\tshort",
            "x\tmedium\ttext",
            "x y\tshort\ttext",
            "x\tshort\t ",
            "a\tshort\tone\na\tshort\ttwo",
            "# only comments",
        ] {
            assert!(parse_manifest(bad).is_err(), "{bad:?}");
        }
        let long_short = format!("x\tshort\t{}", "word ".repeat(16));
        assert!(parse_manifest(&long_short).unwrap_err().contains("at most 15"));
    }

    #[test]
    fn percentiles_nearest_rank() {
        let v = [5.0, 1.0, 3.0, 2.0, 4.0];
        assert_eq!(percentile(&v, 0.5), Some(3.0));
        assert_eq!(percentile(&v, 0.95), Some(5.0));
        assert_eq!(percentile(&v, 0.0), Some(1.0));
        assert_eq!(percentile(&[], 0.5), None);
        let hundred: Vec<f64> = (1..=100).map(f64::from).collect();
        assert_eq!(percentile(&hundred, 0.95), Some(95.0));
    }

    #[test]
    fn word_error_rate() {
        assert_eq!(wer("Hello, world!", "hello world"), 0.0);
        assert_eq!(wer("the cat sat", "the cat sat down"), 1.0 / 3.0);
        assert_eq!(wer("the cat sat", "the bat sat"), 1.0 / 3.0);
        assert_eq!(wer("the cat sat", "cat sat"), 1.0 / 3.0);
        assert_eq!(wer("It's done.", "its done"), 0.5, "apostrophes matter");
        assert_eq!(wer("", ""), 0.0);
        assert_eq!(wer("", "noise"), 1.0);
        assert_eq!(wer("a b", ""), 1.0);
    }

    #[test]
    fn synthetic_audio_matches_word_count() {
        assert_eq!(synthetic_audio(5).len(), 2 * RATE as usize);
        assert_eq!(synthetic_audio(150).len(), 60 * RATE as usize);
        assert_eq!(synthetic_audio(0).len(), (0.8 * RATE as f64) as usize);
        assert!(
            synthetic_audio(5).iter().any(|s| s.abs() > 8_000),
            "loud enough for the VAD"
        );
    }

    fn sample(kind: Kind, ms: Option<f64>, error: Option<&str>) -> Sample {
        Sample {
            id: "x".into(),
            kind,
            words: 5,
            audio_ms: 2_000,
            open_ms: Some(50.0),
            stop_to_final_ms: ms,
            wer: None,
            error: error.map(str::to_owned),
        }
    }

    #[test]
    fn budgets() {
        let ok: Vec<Sample> = (0..20)
            .map(|i| sample(Kind::Short, Some(400.0 + i as f64), None))
            .collect();
        let checks = evaluate(&ok);
        assert_eq!(checks[0].pass, Some(true));
        assert_eq!(checks[1].pass, Some(true));
        assert_eq!(checks[2].pass, None, "no long samples: not measured");
        assert!(gate_passes(&ok, &checks));

        let mut slow_tail = ok.clone();
        slow_tail.push(sample(Kind::Short, Some(2_500.0), None));
        slow_tail.push(sample(Kind::Short, Some(2_600.0), None));
        let checks = evaluate(&slow_tail);
        assert_eq!(checks[0].pass, Some(true));
        assert_eq!(checks[1].pass, Some(false), "p95 over 2 s");
        assert!(!gate_passes(&slow_tail, &checks));

        let long = vec![sample(Kind::Long, Some(3_200.0), None)];
        assert_eq!(evaluate(&long)[2].pass, Some(false));

        let failed = vec![sample(Kind::Short, None, Some("NETWORK"))];
        assert!(
            !gate_passes(&failed, &evaluate(&failed)),
            "a failed dictation fails the gate"
        );
    }
}
