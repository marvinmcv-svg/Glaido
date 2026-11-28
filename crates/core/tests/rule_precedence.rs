//! P2 exit check: formatting rule precedence (spec 4.5.3, master prompt §5.4).
//!
//! * The All apps prompt comes first, then the matching rule's prompt.
//! * A website rule beats an app rule when both match (the site is the more
//!   specific target), and the most specific domain wins among sites.
//! * A switched-off rule falls back to the next match, then to All apps.
//! * Raw text skips the prompt stack entirely.

use vocalis_core::formatting::{plan, AppContext, FieldKind, Input, Plan, Rule, RuleKind, RuleSet, Style};
use vocalis_core::tags::TagContext;

fn rules() -> RuleSet {
    let mut rs = RuleSet::default();
    rs.rules[0].prompt = "Use British spelling.".into();
    rs.rules[1].prompt = "Short paragraphs.".into();
    let mut code = Rule::new("code", "Writing code", RuleKind::Custom);
    code.apps = vec!["com.microsoft.VSCode".into()];
    code.prompt = "Identifiers in code form.".into();
    let mut browser_rule = Rule::new("chrome", "Chrome app", RuleKind::Custom);
    browser_rule.apps = vec!["com.google.Chrome".into()];
    browser_rule.style = Style::Lowercase;
    browser_rule.prompt = "Browser prompt.".into();
    let mut gh = Rule::new("gh", "GitHub", RuleKind::Custom);
    gh.sites = vec!["github.com".into()];
    gh.prompt = "Markdown is fine.".into();
    let mut docs = Rule::new("ghdocs", "GitHub docs", RuleKind::Custom);
    docs.sites = vec!["docs.github.com".into()];
    docs.style = Style::Casual;
    let mut term = Rule::new("term", "Terminal", RuleKind::Custom);
    term.apps = vec!["com.apple.Terminal".into()];
    term.raw = true;
    term.prompt = "Never seen.".into();
    rs.rules.extend([code, browser_rule, gh, docs, term]);
    rs.validate().unwrap();
    rs
}

fn ctx(app_id: &str, url: Option<&str>) -> AppContext {
    AppContext {
        name: String::new(),
        app_id: app_id.into(),
        url: url.map(Into::into),
        field_kind: FieldKind::Generic,
    }
}

#[test]
fn all_apps_prompt_comes_first_then_the_app_prompt() {
    let e = rules().resolve(&ctx("com.microsoft.vscode", None));
    assert_eq!(e.rule_id, "code");
    assert_eq!(e.prompts, ["Use British spelling.", "Identifiers in code form."]);
}

#[test]
fn an_app_without_a_rule_uses_all_apps_only() {
    let e = rules().resolve(&ctx("com.apple.Notes", None));
    assert_eq!(e.rule_id, rules().all_apps().id);
    assert_eq!(e.prompts, ["Use British spelling."]);
    assert_eq!(e.style, Style::Standard);
}

#[test]
fn an_empty_all_apps_prompt_leaves_just_the_rule_prompt() {
    let mut rs = rules();
    rs.rules[0].prompt = "   ".into();
    assert_eq!(
        rs.resolve(&ctx("com.microsoft.VSCode", None)).prompts,
        ["Identifiers in code form."]
    );
}

#[test]
fn a_site_rule_beats_an_app_rule() {
    let e = rules().resolve(&ctx("com.google.Chrome", Some("https://github.com/org/repo")));
    assert_eq!(e.rule_id, "gh");
    assert_eq!(e.prompts, ["Use British spelling.", "Markdown is fine."]);
    // Prompts combine All apps + the winning rule only; the app rule adds nothing.
    assert!(!e.prompts.iter().any(|p| p == "Browser prompt."));
}

#[test]
fn the_most_specific_site_wins() {
    let e = rules().resolve(&ctx("x", Some("https://docs.github.com/en")));
    assert_eq!((e.rule_id.as_str(), e.style), ("ghdocs", Style::Casual));
    let e = rules().resolve(&ctx("x", Some("https://api.github.com")));
    assert_eq!(e.rule_id, "gh", "subdomains match their parent site");
}

#[test]
fn an_unmatched_url_falls_through_to_the_app_rule() {
    let e = rules().resolve(&ctx("com.google.Chrome", Some("https://example.org")));
    assert_eq!((e.rule_id.as_str(), e.style), ("chrome", Style::Lowercase));
}

#[test]
fn a_disabled_rule_falls_back() {
    let mut rs = rules();
    let off = |rs: &mut RuleSet, id: &str| rs.rules.iter_mut().find(|r| r.id == id).unwrap().enabled = false;
    off(&mut rs, "code");
    let e = rs.resolve(&ctx("com.microsoft.VSCode", None));
    assert_eq!(e.rule_id, rs.all_apps().id);
    assert_eq!(e.prompts, ["Use British spelling."]);

    // A disabled site rule falls back to the app rule, then to All apps.
    off(&mut rs, "gh");
    let e = rs.resolve(&ctx("com.google.Chrome", Some("https://github.com")));
    assert_eq!(e.rule_id, "chrome");
    off(&mut rs, "chrome");
    let e = rs.resolve(&ctx("com.google.Chrome", Some("https://github.com")));
    assert_eq!(e.rule_id, rs.all_apps().id);
}

#[test]
fn the_builtin_email_rule_can_be_switched_off() {
    let mut rs = rules();
    let gmail = ctx("com.google.Chrome", Some("https://mail.google.com/mail/u/0"));
    assert_eq!(
        rs.resolve(&gmail).prompts,
        ["Use British spelling.", "Short paragraphs."]
    );
    rs.rules[1].enabled = false;
    rs.validate().unwrap();
    assert_eq!(rs.resolve(&gmail).rule_id, "chrome");
}

#[test]
fn the_system_prompt_lists_prompts_in_order() {
    let rs = rules();
    let app = ctx("com.microsoft.VSCode", None);
    let tags = TagContext::default();
    let input = Input {
        transcript: "rename the user id function",
        app: &app,
        rules: &rs,
        dictionary: &[],
        snippets: &[],
        wake_word: "Vocalis",
        tags: &tags,
    };
    let Plan::Llm(req) = plan(input) else {
        panic!("expected cleanup")
    };
    let first = req.system.find("1. Use British spelling.").unwrap();
    let second = req.system.find("2. Identifiers in code form.").unwrap();
    assert!(first < second);
}

#[test]
fn raw_text_skips_every_prompt() {
    let rs = rules();
    let app = ctx("com.apple.Terminal", None);
    let tags = TagContext::default();
    let input = Input {
        transcript: "git checkout -b feature/login",
        app: &app,
        rules: &rs,
        dictionary: &[],
        snippets: &[],
        wake_word: "Vocalis",
        tags: &tags,
    };
    assert_eq!(plan(input), Plan::Final("Git checkout -b feature/login".into()));
}
