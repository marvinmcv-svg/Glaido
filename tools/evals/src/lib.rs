//! Formatting eval suite (master prompt §5.4 "Evals", P2 exit check).
//!
//! Cases live in `packages/prompts/evals/formatting/cases.jsonl`, one JSON
//! object per line. Each case builds a rule set (defaults, optional presets
//! and custom rules), runs the shared pipeline from `vocalis-core`, and checks:
//!
//! * the **deterministic** result: the plan (final text, command, empty…),
//!   the assembled cleanup request, and the fallback text used when cleanup
//!   is unavailable. These run in `cargo test` on every CI build.
//! * for `llm` cases, optionally the **live** cleanup output from the real
//!   provider (`vocalis-evals --live`, needs `ANTHROPIC_API_KEY`). Model output
//!   varies, so live checks are properties (contains, never answers, ends
//!   with "?") rather than exact strings.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use vocalis_core::dictionary::Entry;
use vocalis_core::formatting::{self, AppContext, Input, LlmRequest, Plan, Rule, RuleKind, RuleSet, Style};
use vocalis_core::presets;
use vocalis_core::snippets::Snippet;
use vocalis_core::tags::TagContext;

/// Pass rate the deterministic set must reach (P2 exit: ≥ 95%).
pub const PASS_THRESHOLD: f64 = 0.95;

/// Default location of the cases, relative to the repository root.
pub fn default_cases_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/prompts/evals/formatting/cases.jsonl")
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct AllAppsPatch {
    pub style: Option<Style>,
    pub raw: Option<bool>,
    pub prompt: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct CustomRule {
    pub name: String,
    pub enabled: bool,
    pub style: Style,
    pub raw: bool,
    pub prompt: String,
    pub apps: Vec<String>,
    pub sites: Vec<String>,
}

impl Default for CustomRule {
    fn default() -> Self {
        Self {
            name: "Custom".into(),
            enabled: true,
            style: Style::Standard,
            raw: false,
            prompt: String::new(),
            apps: vec![],
            sites: vec![],
        }
    }
}

/// Property checks on a model's cleaned output (after the post-pass).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct OutputChecks {
    /// Each must appear (case-sensitive substring).
    pub contains: Vec<String>,
    /// At least one must appear (case-insensitive substring).
    pub contains_any: Vec<String>,
    /// None may appear as a whole word (case-insensitive).
    pub excludes_words: Vec<String>,
    pub ends_with: Option<String>,
    pub no_trailing_period: bool,
    pub lowercase: bool,
    pub min_lines: Option<usize>,
    pub max_chars: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "plan", rename_all = "snake_case")]
pub enum Expect {
    /// The pipeline finishes without the model (raw text, snippet).
    Final {
        text: String,
    },
    Command {
        request: String,
    },
    OpenCommandWindow,
    Empty,
    /// Goes to the cleanup model.
    Llm {
        #[serde(default)]
        system_contains: Vec<String>,
        #[serde(default)]
        system_excludes: Vec<String>,
        /// Exact text when the model is unavailable.
        #[serde(default)]
        fallback: Option<String>,
        /// A canned model answer, and the exact text after the post-pass.
        #[serde(default)]
        model_output: Option<String>,
        #[serde(default)]
        finished: Option<String>,
        /// Live checks; cases without them have no live part.
        #[serde(default)]
        output: Option<OutputChecks>,
    },
}

#[derive(Debug, Clone, Deserialize)]
pub struct Case {
    pub id: String,
    pub category: String,
    pub transcript: String,
    #[serde(default)]
    pub app: AppContext,
    #[serde(default)]
    pub all_apps: AllAppsPatch,
    #[serde(default)]
    pub email_enabled: Option<bool>,
    /// Preset ids added in order, for every OS, as one click each would.
    #[serde(default)]
    pub presets: Vec<String>,
    #[serde(default)]
    pub rules: Vec<CustomRule>,
    #[serde(default)]
    pub dictionary: Vec<Entry>,
    #[serde(default)]
    pub snippets: Vec<Snippet>,
    #[serde(default)]
    pub tags: TagContext,
    pub expect: Expect,
}

impl Case {
    pub fn is_live(&self) -> bool {
        matches!(&self.expect, Expect::Llm { output: Some(_), .. })
    }

    /// The rule set this case dictates into.
    pub fn rules(&self) -> Result<RuleSet, String> {
        let mut rs = RuleSet::default();
        let all = &mut rs.rules[0];
        if let Some(s) = self.all_apps.style {
            all.style = s;
        }
        if let Some(r) = self.all_apps.raw {
            all.raw = r;
        }
        if let Some(p) = &self.all_apps.prompt {
            all.prompt = p.clone();
        }
        if let Some(e) = self.email_enabled {
            rs.rules[1].enabled = e;
        }
        for (i, id) in self.presets.iter().enumerate() {
            let p = presets::find(id).ok_or_else(|| format!("unknown preset {id}"))?;
            let applied = presets::apply(p, &rs, None, &format!("preset-{i}"));
            if p.builtin_email {
                let at = rs
                    .rules
                    .iter()
                    .position(|r| r.kind == RuleKind::Email)
                    .expect("email rule");
                rs.rules[at] = applied.rule;
            } else if !applied.is_empty() {
                rs.rules.push(applied.rule);
            }
        }
        for (i, c) in self.rules.iter().enumerate() {
            let mut r = Rule::new(&format!("custom-{i}"), &c.name, RuleKind::Custom);
            r.enabled = c.enabled;
            r.style = c.style;
            r.raw = c.raw;
            r.prompt = c.prompt.clone();
            r.apps = c.apps.clone();
            r.sites = c.sites.clone();
            rs.rules.push(r);
        }
        rs.validate().map_err(|e| format!("invalid rules: {e}"))?;
        Ok(rs)
    }
}

pub fn load(path: &Path) -> Result<Vec<Case>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    text.lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty() && !l.trim_start().starts_with("//"))
        .map(|(n, l)| serde_json::from_str(l).map_err(|e| format!("line {}: {e}", n + 1)))
        .collect()
}

fn input<'a>(case: &'a Case, rules: &'a RuleSet) -> Input<'a> {
    Input {
        transcript: &case.transcript,
        app: &case.app,
        rules,
        dictionary: &case.dictionary,
        snippets: &case.snippets,
        wake_word: vocalis_core::product::WAKE_WORD,
        tags: &case.tags,
    }
}

fn describe(p: &Plan) -> String {
    match p {
        Plan::Empty => "empty".into(),
        Plan::Command(r) => format!("command {r:?}"),
        Plan::OpenCommandWindow => "open_command_window".into(),
        Plan::Final(t) => format!("final {t:?}"),
        Plan::Llm(_) => "llm".into(),
    }
}

/// Run the deterministic part of a case.
pub fn run_deterministic(case: &Case) -> Result<(), String> {
    let rules = case.rules()?;
    let inp = input(case, &rules);
    let plan = formatting::plan(inp);
    let mismatch = |want: &str| Err(format!("expected {want}, got {}", describe(&plan)));
    match (&case.expect, &plan) {
        (Expect::Final { text }, Plan::Final(got)) if got == text => Ok(()),
        (Expect::Final { text }, _) => mismatch(&format!("final {text:?}")),
        (Expect::Command { request }, Plan::Command(got)) if got == request => Ok(()),
        (Expect::Command { request }, _) => mismatch(&format!("command {request:?}")),
        (Expect::OpenCommandWindow, Plan::OpenCommandWindow) | (Expect::Empty, Plan::Empty) => Ok(()),
        (Expect::OpenCommandWindow, _) => mismatch("open_command_window"),
        (Expect::Empty, _) => mismatch("empty"),
        (
            Expect::Llm {
                system_contains,
                system_excludes,
                fallback,
                model_output,
                finished,
                ..
            },
            Plan::Llm(req),
        ) => {
            for s in system_contains {
                if !req.system.contains(s.as_str()) {
                    return Err(format!("system prompt lacks {s:?}"));
                }
            }
            for s in system_excludes {
                if req.system.contains(s.as_str()) {
                    return Err(format!("system prompt should not contain {s:?}"));
                }
            }
            if !req.user.contains(case.transcript.trim()) {
                return Err("the dictation is missing from the user message".into());
            }
            if let Some(want) = fallback {
                let got = formatting::fallback(inp);
                if &got != want {
                    return Err(format!("fallback: expected {want:?}, got {got:?}"));
                }
            }
            if let (Some(model), Some(want)) = (model_output, finished) {
                let got = formatting::finish(model, inp);
                if &got != want {
                    return Err(format!("post-pass: expected {want:?}, got {got:?}"));
                }
            }
            Ok(())
        }
        (Expect::Llm { .. }, _) => mismatch("llm"),
    }
}

/// The cleanup request for a live case.
pub fn llm_request(case: &Case) -> Result<LlmRequest, String> {
    let rules = case.rules()?;
    match formatting::plan(input(case, &rules)) {
        Plan::Llm(r) => Ok(r),
        other => Err(format!("expected llm, got {}", describe(&other))),
    }
}

/// Post-pass the model's answer exactly as the gateway does.
pub fn finish(case: &Case, model_output: &str) -> Result<String, String> {
    let rules = case.rules()?;
    Ok(formatting::finish(model_output, input(case, &rules)))
}

fn has_word(text: &str, word: &str) -> bool {
    let w = word.to_lowercase();
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '\'')
        .any(|t| t == w)
}

/// Check a live output against a case's properties.
pub fn check_output(checks: &OutputChecks, out: &str) -> Result<(), String> {
    let out = out.trim();
    for s in &checks.contains {
        if !out.contains(s.as_str()) {
            return Err(format!("missing {s:?}"));
        }
    }
    let lower = out.to_lowercase();
    if !checks.contains_any.is_empty() && !checks.contains_any.iter().any(|s| lower.contains(&s.to_lowercase())) {
        return Err(format!("none of {:?}", checks.contains_any));
    }
    for w in &checks.excludes_words {
        if has_word(out, w) {
            return Err(format!("should not contain the word {w:?}"));
        }
    }
    if let Some(e) = &checks.ends_with {
        if !out.ends_with(e.as_str()) {
            return Err(format!("should end with {e:?}"));
        }
    }
    if checks.no_trailing_period && out.ends_with('.') && !out.ends_with("...") {
        return Err("should not end with a full stop".into());
    }
    if checks.lowercase && out.chars().any(char::is_uppercase) {
        return Err("should be lowercase".into());
    }
    if let Some(n) = checks.min_lines {
        let lines = out.lines().filter(|l| !l.trim().is_empty()).count();
        if lines < n {
            return Err(format!("expected at least {n} lines, got {lines}"));
        }
    }
    if let Some(n) = checks.max_chars {
        if out.chars().count() > n {
            return Err(format!("longer than {n} characters"));
        }
    }
    Ok(())
}

/// Outcome of a set of cases.
#[derive(Debug, Default)]
pub struct Report {
    pub passed: usize,
    pub failures: Vec<(String, String)>,
}

impl Report {
    pub fn total(&self) -> usize {
        self.passed + self.failures.len()
    }

    pub fn rate(&self) -> f64 {
        if self.total() == 0 {
            1.0
        } else {
            self.passed as f64 / self.total() as f64
        }
    }

    pub fn record(&mut self, id: &str, r: Result<(), String>) {
        match r {
            Ok(()) => self.passed += 1,
            Err(e) => self.failures.push((id.to_owned(), e)),
        }
    }

    pub fn summary(&self, label: &str) -> String {
        let mut s = format!(
            "{label}: {}/{} passed ({:.1}%)",
            self.passed,
            self.total(),
            self.rate() * 100.0
        );
        for (id, e) in &self.failures {
            s.push_str(&format!("\n  FAIL {id}: {e}"));
        }
        s
    }
}

pub fn run_all_deterministic(cases: &[Case]) -> Report {
    let mut r = Report::default();
    for c in cases {
        r.record(&c.id, run_deterministic(c));
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_checks() {
        let c = OutputChecks {
            contains: vec!["Thursday".into()],
            excludes_words: vec!["um".into()],
            ..Default::default()
        };
        assert!(check_output(&c, "Move it to Thursday.").is_ok());
        assert!(check_output(&c, "Um, move it to Thursday.").is_err());
        assert!(
            check_output(&c, "Summer plans for Thursday").is_ok(),
            "whole words only"
        );
        let q = OutputChecks {
            ends_with: Some("?".into()),
            no_trailing_period: true,
            ..Default::default()
        };
        assert!(check_output(&q, "What is it?").is_ok());
        assert!(check_output(&q, "It is.").is_err());
    }
}
