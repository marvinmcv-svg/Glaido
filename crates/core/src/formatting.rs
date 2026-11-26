//! Formatting pipeline (master prompt §5.4, spec 4.5.3).
//!
//! The order is deterministic:
//! 0. Empty transcript → nothing to do.
//! 1. Wake word in the first two words → route to Commands.
//! 2. Snippet: the whole transcript equals a trigger → snippet content.
//!    (ADR-0010: snippets also fire when Raw text is on.)
//! 3. Raw text on → deterministic style transforms only.
//!    Otherwise → LLM cleanup with an assembled system prompt.
//! 4. Post-pass: dictionary corrections and tag expansion.
//!
//! [`plan`] covers steps 0–3 and returns either a final text or an LLM request.
//! After the LLM answers, call [`finish`] to run the post-pass.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::dictionary::{self, Entry};
use crate::snippets::{self, Snippet};
use crate::tags::TagContext;
use crate::text::char_len;
use crate::wake::{self, WakeMatch};

/// Maximum characters in a rule's custom prompt.
pub const MAX_PROMPT_CHARS: usize = 500;
/// Version tag of the cleanup prompt, recorded alongside eval results.
pub const CLEANUP_PROMPT_VERSION: &str = "cleanup/v1";
const CLEANUP_PROMPT: &str = include_str!("../../../packages/prompts/cleanup/v1.txt");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Style {
    /// Full capitalization and punctuation.
    #[default]
    Standard,
    /// Capitals stay, fewer full stops.
    Casual,
    /// No capitals, minimal punctuation.
    Lowercase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldKind {
    Email,
    Chat,
    Code,
    Terminal,
    Document,
    #[default]
    Generic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleKind {
    AllApps,
    Email,
    Custom,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rule {
    pub id: String,
    pub name: String,
    pub kind: RuleKind,
    pub enabled: bool,
    pub style: Style,
    pub raw: bool,
    pub prompt: String,
    /// App identifiers (macOS bundle ID, Windows exe name, Linux desktop ID). Case-insensitive.
    pub apps: Vec<String>,
    /// Domains such as `github.com`. Subdomains match too.
    pub sites: Vec<String>,
}

impl Rule {
    pub fn new(id: &str, name: &str, kind: RuleKind) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            kind,
            enabled: true,
            style: Style::Standard,
            raw: false,
            prompt: String::new(),
            apps: vec![],
            sites: vec![],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleSet {
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RuleError {
    #[error("there must be exactly one All apps rule")]
    AllAppsCount,
    #[error("the All apps rule cannot be switched off")]
    AllAppsDisabled,
    #[error("the prompt for \"{0}\" is longer than {MAX_PROMPT_CHARS} characters")]
    PromptTooLong(String),
    #[error("{item} is already used by \"{owner}\"")]
    AlreadyUsed { item: String, owner: String },
}

impl Default for RuleSet {
    /// All apps (Standard) plus the built-in Email rule seeded with two webmail sites.
    fn default() -> Self {
        let all = Rule::new("all", "All apps", RuleKind::AllApps);
        let mut email = Rule::new("email", "Email", RuleKind::Email);
        email.sites = vec!["mail.google.com".into(), "outlook.live.com".into()];
        Self {
            rules: vec![all, email],
        }
    }
}

/// What the client knows about where the text is going.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppContext {
    pub name: String,
    pub app_id: String,
    pub url: Option<String>,
    pub field_kind: FieldKind,
}

/// The settings that apply to one dictation after rule resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Effective {
    pub rule_id: String,
    pub style: Style,
    pub raw: bool,
    /// Custom prompts in application order: All apps first, then the matched rule.
    pub prompts: Vec<String>,
}

/// Normalize a site entry or URL to a bare lowercase host: no scheme, `www.`, port or path.
pub fn normalize_host(s: &str) -> String {
    let s = s.trim().to_lowercase();
    let s = s.split_once("://").map_or(s.as_str(), |(_, r)| r);
    let s = s.split(['/', '?', '#']).next().unwrap_or("");
    let s = s.rsplit_once('@').map_or(s, |(_, h)| h);
    let s = s.split(':').next().unwrap_or("");
    s.strip_prefix("www.").unwrap_or(s).trim_end_matches('.').to_owned()
}

fn host_matches(host: &str, site: &str) -> bool {
    !site.is_empty() && (host == site || host.ends_with(&format!(".{site}")))
}

impl RuleSet {
    pub fn all_apps(&self) -> &Rule {
        self.rules
            .iter()
            .find(|r| r.kind == RuleKind::AllApps)
            .expect("validated rule set has All apps")
    }

    /// Enforce the invariants from spec 4.5.3.
    pub fn validate(&self) -> Result<(), RuleError> {
        let all: Vec<_> = self.rules.iter().filter(|r| r.kind == RuleKind::AllApps).collect();
        if all.len() != 1 {
            return Err(RuleError::AllAppsCount);
        }
        if !all[0].enabled {
            return Err(RuleError::AllAppsDisabled);
        }
        let mut owners: std::collections::HashMap<String, &str> = Default::default();
        for r in &self.rules {
            if char_len(&r.prompt) > MAX_PROMPT_CHARS {
                return Err(RuleError::PromptTooLong(r.name.clone()));
            }
            let items = r
                .apps
                .iter()
                .map(|a| format!("app:{}", a.trim().to_lowercase()))
                .chain(r.sites.iter().map(|s| format!("site:{}", normalize_host(s))));
            for item in items {
                if let Some(owner) = owners.insert(item.clone(), &r.name) {
                    let item = item.split_once(':').map(|(_, v)| v.to_owned()).unwrap_or(item);
                    return Err(RuleError::AlreadyUsed {
                        item,
                        owner: owner.to_owned(),
                    });
                }
            }
        }
        Ok(())
    }

    /// Which enabled rule (if any) owns this app or site. Sites are checked first
    /// (browsers are never app-rule targets), and the most specific domain wins.
    pub fn matching_rule(&self, app: &AppContext) -> Option<&Rule> {
        let enabled = || self.rules.iter().filter(|r| r.enabled && r.kind != RuleKind::AllApps);
        if let Some(url) = app.url.as_deref() {
            let host = normalize_host(url);
            let best = enabled()
                .flat_map(|r| r.sites.iter().map(move |s| (r, normalize_host(s))))
                .filter(|(_, s)| host_matches(&host, s))
                .max_by_key(|(_, s)| s.len());
            if let Some((r, _)) = best {
                return Some(r);
            }
        }
        let id = app.app_id.trim().to_lowercase();
        if id.is_empty() {
            return None;
        }
        enabled().find(|r| r.apps.iter().any(|a| a.trim().to_lowercase() == id))
    }

    /// Resolve the effective style, raw flag and prompt stack for a dictation.
    pub fn resolve(&self, app: &AppContext) -> Effective {
        let all = self.all_apps();
        let mut prompts = Vec::new();
        if !all.prompt.trim().is_empty() {
            prompts.push(all.prompt.trim().to_owned());
        }
        match self.matching_rule(app) {
            Some(r) => {
                if !r.prompt.trim().is_empty() {
                    prompts.push(r.prompt.trim().to_owned());
                }
                Effective {
                    rule_id: r.id.clone(),
                    style: r.style,
                    raw: r.raw,
                    prompts,
                }
            }
            None => Effective {
                rule_id: all.id.clone(),
                style: all.style,
                raw: all.raw,
                prompts,
            },
        }
    }
}

/// Deterministic style transforms used on the Raw text path, and as a final
/// casing pass for Lowercase.
pub fn apply_style(text: &str, style: Style) -> String {
    let text = text.trim();
    match style {
        Style::Standard => capitalize_sentences(text),
        Style::Casual => strip_final_period(&capitalize_sentences(text)),
        Style::Lowercase => strip_final_period(&text.to_lowercase()),
    }
}

fn capitalize_sentences(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut cap_next = true;
    let mut prev_terminal = false;
    for c in text.chars() {
        if cap_next && c.is_alphabetic() {
            out.extend(c.to_uppercase());
            cap_next = false;
        } else {
            out.push(c);
        }
        if c.is_alphanumeric() {
            cap_next = false;
        }
        if c.is_whitespace() && prev_terminal {
            cap_next = true;
        }
        if c == '\n' {
            cap_next = true;
        }
        if !c.is_whitespace() {
            prev_terminal = matches!(c, '.' | '!' | '?');
        }
    }
    out
}

fn strip_final_period(text: &str) -> String {
    match text.strip_suffix('.') {
        Some(rest) if !rest.ends_with('.') => rest.to_owned(),
        _ => text.to_owned(),
    }
}

/// A request for the cleanup model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmRequest {
    pub system: String,
    pub user: String,
    pub prompt_version: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plan {
    /// Nothing was said.
    Empty,
    /// Wake word with a request: run it as a Command.
    Command(String),
    /// Wake word alone: open the command window and wait.
    OpenCommandWindow,
    /// Finished text, ready to paste.
    Final(String),
    /// Send to the cleanup model, then call [`finish`].
    Llm(LlmRequest),
}

/// Everything the pipeline needs for one dictation.
#[derive(Debug, Clone, Copy)]
pub struct Input<'a> {
    pub transcript: &'a str,
    pub app: &'a AppContext,
    pub rules: &'a RuleSet,
    pub dictionary: &'a [Entry],
    pub snippets: &'a [Snippet],
    pub wake_word: &'a str,
    pub tags: &'a TagContext,
}

/// Steps 0–3 of the pipeline.
pub fn plan(input: Input<'_>) -> Plan {
    let transcript = input.transcript.trim();
    if !transcript.chars().any(char::is_alphanumeric) {
        return Plan::Empty;
    }
    match wake::detect(transcript, input.wake_word) {
        Some(WakeMatch::Command(req)) => return Plan::Command(req),
        Some(WakeMatch::NameOnly) => return Plan::OpenCommandWindow,
        None => {}
    }
    if let Some(s) = snippets::find_match(transcript, input.snippets) {
        return Plan::Final(snippets::render(s, input.tags));
    }
    let eff = input.rules.resolve(input.app);
    if eff.raw {
        return Plan::Final(post_pass(&apply_style(transcript, eff.style), input));
    }
    Plan::Llm(LlmRequest {
        system: system_prompt(&eff, input),
        user: format!("<dictation>\n{transcript}\n</dictation>"),
        prompt_version: CLEANUP_PROMPT_VERSION,
    })
}

/// Step 4, after the cleanup model has answered.
pub fn finish(llm_output: &str, input: Input<'_>) -> String {
    let eff = input.rules.resolve(input.app);
    let cleaned = strip_wrappers(llm_output);
    let styled = if eff.style == Style::Lowercase {
        apply_style(&cleaned, Style::Lowercase)
    } else {
        cleaned
    };
    post_pass(&styled, input)
}

/// Deterministic result used when the cleanup model is unavailable, refuses or
/// times out: the style transforms plus the dictionary post-pass, as on the Raw
/// text path. Dictation never fails just because cleanup did.
pub fn fallback(input: Input<'_>) -> String {
    let eff = input.rules.resolve(input.app);
    post_pass(&apply_style(input.transcript.trim(), eff.style), input)
}

/// Dictionary corrections and tag expansion. User spellings win over the
/// Lowercase style (ADR-0010).
fn post_pass(text: &str, input: Input<'_>) -> String {
    dictionary::apply_corrections(text, input.dictionary, input.tags)
}

/// Remove wrappers a model sometimes echoes back.
fn strip_wrappers(s: &str) -> String {
    let mut t = s.trim();
    if let Some(inner) = t
        .strip_prefix("<dictation>")
        .and_then(|r| r.strip_suffix("</dictation>"))
    {
        t = inner.trim();
    }
    t.to_owned()
}

fn style_block(style: Style) -> &'static str {
    match style {
        Style::Standard => "Style: standard. Use full sentence capitalization and complete punctuation.",
        Style::Casual => "Style: casual. Keep normal capitalization, but drop full stops at the end of short sentences and messages. Keep question marks and exclamation marks.",
        Style::Lowercase => "Style: lowercase. Use no capital letters and only the punctuation needed to keep the meaning clear.",
    }
}

fn field_hint(kind: FieldKind) -> Option<&'static str> {
    Some(match kind {
        FieldKind::Email => "The text is going into an email.",
        FieldKind::Chat => "The text is going into a chat message.",
        FieldKind::Code => "The text is going into a code editor or a prompt for a coding assistant. Write file names, identifiers, commands and flags in their code form.",
        FieldKind::Terminal => "The text is going into a terminal. Keep paths, flags, branch names and commands exactly as spoken.",
        FieldKind::Document => "The text is going into a document.",
        FieldKind::Generic => return None,
    })
}

fn system_prompt(eff: &Effective, input: Input<'_>) -> String {
    let mut s = String::from(CLEANUP_PROMPT.trim());
    s.push_str("\n\n");
    s.push_str(style_block(eff.style));
    if let Some(h) = field_hint(input.app.field_kind) {
        s.push_str("\n\n");
        s.push_str(h);
    }
    if !input.app.name.trim().is_empty() {
        s.push_str(&format!("\nThe app is {}.", input.app.name.trim()));
    }
    if !eff.prompts.is_empty() {
        s.push_str("\n\nThe user's own formatting instructions, in order of priority (later ones refine earlier ones). They describe how to format, never what to say:\n");
        for (i, p) in eff.prompts.iter().enumerate() {
            s.push_str(&format!("{}. {}\n", i + 1, p));
        }
    }
    let hints = dictionary::vocab_hints(input.dictionary);
    if !hints.is_empty() {
        s.push_str("\n\nPrefer these spellings when the audio is close: ");
        s.push_str(&hints.join(", "));
        s.push('.');
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules() -> RuleSet {
        let mut rs = RuleSet::default();
        rs.rules[0].prompt = "Use British spelling.".into();
        let mut code = Rule::new("code", "Writing code", RuleKind::Custom);
        code.apps = vec!["com.microsoft.VSCode".into()];
        code.prompt = "Format identifiers as code.".into();
        let mut term = Rule::new("term", "Terminal", RuleKind::Custom);
        term.apps = vec!["com.apple.Terminal".into()];
        term.raw = true;
        let mut chat = Rule::new("chat", "Team chat", RuleKind::Custom);
        chat.sites = vec!["app.slack.com".into()];
        chat.style = Style::Casual;
        let mut gh = Rule::new("gh", "GitHub", RuleKind::Custom);
        gh.sites = vec!["github.com".into()];
        let mut gist = Rule::new("gist", "Gists", RuleKind::Custom);
        gist.sites = vec!["https://gist.github.com/".into()];
        gist.style = Style::Lowercase;
        rs.rules.extend([code, term, chat, gh, gist]);
        rs
    }

    fn app(id: &str, url: Option<&str>) -> AppContext {
        AppContext {
            name: "Test".into(),
            app_id: id.into(),
            url: url.map(Into::into),
            field_kind: FieldKind::Generic,
        }
    }

    fn input<'a>(
        t: &'a str,
        app: &'a AppContext,
        rules: &'a RuleSet,
        dict: &'a [Entry],
        snips: &'a [Snippet],
        tags: &'a TagContext,
    ) -> Input<'a> {
        Input {
            transcript: t,
            app,
            rules,
            dictionary: dict,
            snippets: snips,
            wake_word: "Vocalis",
            tags,
        }
    }

    #[test]
    fn default_ruleset_is_valid() {
        RuleSet::default().validate().unwrap();
        rules().validate().unwrap();
    }

    #[test]
    fn validation_rejects_reuse_and_long_prompts() {
        let mut rs = rules();
        let mut dup = Rule::new("dup", "Dup", RuleKind::Custom);
        dup.sites = vec!["www.GitHub.com/path".into()];
        rs.rules.push(dup);
        assert_eq!(
            rs.validate(),
            Err(RuleError::AlreadyUsed {
                item: "github.com".into(),
                owner: "GitHub".into()
            })
        );

        let mut rs = rules();
        rs.rules[2].prompt = "x".repeat(501);
        assert!(matches!(rs.validate(), Err(RuleError::PromptTooLong(_))));
        rs.rules[2].prompt = "é".repeat(500);
        rs.validate().unwrap();

        let mut rs = rules();
        rs.rules[0].enabled = false;
        assert_eq!(rs.validate(), Err(RuleError::AllAppsDisabled));
        rs.rules.remove(0);
        assert_eq!(rs.validate(), Err(RuleError::AllAppsCount));
    }

    #[test]
    fn prompt_inheritance_all_apps_first() {
        let e = rules().resolve(&app("COM.MICROSOFT.VSCODE", None));
        assert_eq!(e.rule_id, "code");
        assert_eq!(e.prompts, vec!["Use British spelling.", "Format identifiers as code."]);
        let e = rules().resolve(&app("com.unknown", None));
        assert_eq!(e.rule_id, "all");
        assert_eq!(e.prompts, vec!["Use British spelling."]);
    }

    #[test]
    fn sites_match_subdomains_and_most_specific_wins() {
        let rs = rules();
        assert_eq!(
            rs.resolve(&app("com.google.Chrome", Some("https://mail.google.com/mail/u/0")))
                .rule_id,
            "email"
        );
        assert_eq!(rs.resolve(&app("x", Some("https://github.com/org/repo"))).rule_id, "gh");
        assert_eq!(
            rs.resolve(&app("x", Some("https://gist.github.com/abc"))).rule_id,
            "gist"
        );
        assert_eq!(rs.resolve(&app("x", Some("https://notgithub.com"))).rule_id, "all");
    }

    #[test]
    fn disabled_rule_falls_back_to_all_apps() {
        let mut rs = rules();
        rs.rules[1].enabled = false; // Email
        assert_eq!(rs.resolve(&app("x", Some("https://mail.google.com"))).rule_id, "all");
    }

    #[test]
    fn app_context_fields_are_optional_on_the_wire() {
        let a: AppContext = serde_json::from_str(r#"{"name":"Notes"}"#).unwrap();
        assert_eq!(a.field_kind, FieldKind::Generic);
        assert_eq!(a.url, None);
        let t: TagContext = serde_json::from_str("{}").unwrap();
        assert_eq!(t, TagContext::default());
    }

    #[test]
    fn host_normalization() {
        assert_eq!(normalize_host("HTTPS://user@www.Example.com:8443/a?b#c"), "example.com");
        assert_eq!(normalize_host("github.com"), "github.com");
    }

    #[test]
    fn styles() {
        assert_eq!(
            apply_style("hello there. how are you? fine.", Style::Standard),
            "Hello there. How are you? Fine."
        );
        assert_eq!(apply_style("sounds good.", Style::Casual), "Sounds good");
        assert_eq!(apply_style("wait...", Style::Casual), "Wait...");
        assert_eq!(apply_style("Ship It Today.", Style::Lowercase), "ship it today");
        assert_eq!(apply_style("version 2.5 is out", Style::Standard), "Version 2.5 is out");
    }

    #[test]
    fn pipeline_routes_wake_word_before_snippets() {
        let rs = rules();
        let a = app("x", None);
        let tags = TagContext::default();
        let snips = [Snippet {
            trigger: "vocalis".into(),
            content: "never".into(),
        }];
        assert_eq!(
            plan(input("Hey Vocalis, make this formal", &a, &rs, &[], &snips, &tags)),
            Plan::Command("make this formal".into())
        );
        assert_eq!(
            plan(input("Vocalis", &a, &rs, &[], &snips, &tags)),
            Plan::OpenCommandWindow
        );
        assert_eq!(plan(input("  ...  ", &a, &rs, &[], &snips, &tags)), Plan::Empty);
    }

    #[test]
    fn snippet_fires_even_in_raw_apps() {
        let rs = rules();
        let a = app("com.apple.Terminal", None);
        let tags = TagContext::default();
        let snips = [Snippet {
            trigger: "deploy command".into(),
            content: "make deploy ENV=prod".into(),
        }];
        assert_eq!(
            plan(input("Deploy command.", &a, &rs, &[], &snips, &tags)),
            Plan::Final("make deploy ENV=prod".into())
        );
    }

    #[test]
    fn raw_path_applies_style_and_dictionary() {
        let rs = rules();
        let a = app("com.apple.Terminal", None);
        let tags = TagContext::default();
        let dict = [Entry::correction("get hub", "GitHub")];
        assert_eq!(
            plan(input("push to get hub main", &a, &rs, &dict, &[], &tags)),
            Plan::Final("Push to GitHub main".into())
        );
    }

    #[test]
    fn llm_request_is_assembled_and_post_pass_runs() {
        let rs = rules();
        let mut a = app("com.microsoft.VSCode", None);
        a.field_kind = FieldKind::Code;
        let tags = TagContext {
            clipboard: Some("src/auth.ts".into()),
            ..Default::default()
        };
        let dict = [Entry::vocab("Supabase"), Entry::correction("that file", "{clipboard}")];
        let inp = input("um so open that file and uh fix the bug", &a, &rs, &dict, &[], &tags);
        let Plan::Llm(req) = plan(inp) else {
            panic!("expected LLM plan")
        };
        assert!(req.system.starts_with("You turn raw speech-to-text output"));
        assert!(req
            .system
            .contains("1. Use British spelling.\n2. Format identifiers as code."));
        assert!(req.system.contains("code editor"));
        assert!(req
            .system
            .contains("Prefer these spellings when the audio is close: Supabase."));
        assert!(!req.system.contains("{clipboard}"));
        assert!(req.user.contains("<dictation>\num so open"));
        assert_eq!(
            finish("<dictation>Open that file and fix the bug.</dictation>", inp),
            "Open src/auth.ts and fix the bug."
        );
    }

    #[test]
    fn fallback_is_deterministic_cleanup() {
        let rs = rules();
        let a = app("x", None);
        let tags = TagContext::default();
        let dict = [Entry::correction("postgres", "PostgreSQL")];
        let inp = input("  we moved to postgres. it works  ", &a, &rs, &dict, &[], &tags);
        assert_eq!(fallback(inp), "We moved to PostgreSQL. It works");
    }

    #[test]
    fn lowercase_rule_lowercases_llm_output_but_keeps_dictionary_spellings() {
        let rs = rules();
        let a = app("x", Some("https://gist.github.com/1"));
        let tags = TagContext::default();
        let dict = [Entry::correction("postgres", "PostgreSQL")];
        let inp = input("We Moved To Postgres.", &a, &rs, &dict, &[], &tags);
        assert_eq!(finish("We Moved To Postgres.", inp), "we moved to PostgreSQL");
    }
}
