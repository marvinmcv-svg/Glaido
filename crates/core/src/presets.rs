//! The preset rule gallery (spec 4.5.3 ★): ready-made formatting rules added
//! with one click. Wording is ours.
//!
//! Adding a preset respects "one app or website belongs to exactly one rule":
//! apps and sites that another rule already owns are skipped and reported, so
//! the caller can tell the user. The Email preset fills in the built-in Email
//! rule instead of adding a second one.

use serde::Serialize;

use crate::formatting::{normalize_host, Rule, RuleKind, RuleSet, Style};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Os {
    Macos,
    Windows,
    Linux,
}

impl Os {
    /// The OS this binary runs on, if it is one we ship for.
    pub fn current() -> Option<Self> {
        match std::env::consts::OS {
            "macos" => Some(Self::Macos),
            "windows" => Some(Self::Windows),
            "linux" => Some(Self::Linux),
            _ => None,
        }
    }
}

/// An app a preset targets. `id` is the macOS bundle id, Windows exe name or
/// X11 WM_CLASS class, as `crates/context` reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetApp {
    pub id: &'static str,
    pub name: &'static str,
    pub os: Os,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    pub id: &'static str,
    pub name: &'static str,
    /// One line for the gallery card.
    pub summary: &'static str,
    pub style: Style,
    pub raw: bool,
    pub prompt: &'static str,
    pub apps: &'static [PresetApp],
    pub sites: &'static [&'static str],
    /// Fills in the built-in Email rule rather than adding a new rule.
    pub builtin_email: bool,
}

const fn app(id: &'static str, name: &'static str, os: Os) -> PresetApp {
    PresetApp { id, name, os }
}

use Os::{Linux, Macos, Windows};

const CODE_APPS: &[PresetApp] = &[
    app("com.microsoft.VSCode", "Visual Studio Code", Macos),
    app("Code.exe", "Visual Studio Code", Windows),
    app("Code", "Visual Studio Code", Linux),
    app("com.todesktop.230313mzl4w4u92", "Cursor", Macos),
    app("Cursor.exe", "Cursor", Windows),
    app("Cursor", "Cursor", Linux),
];

const AI_CHAT_APPS: &[PresetApp] = &[
    app("com.openai.chat", "ChatGPT", Macos),
    app("ChatGPT.exe", "ChatGPT", Windows),
];

const AI_CHAT_SITES: &[&str] = &["chatgpt.com", "claude.ai", "gemini.google.com"];

const TEAM_CHAT_APPS: &[PresetApp] = &[
    app("com.tinyspeck.slackmacgap", "Slack", Macos),
    app("slack.exe", "Slack", Windows),
    app("Slack", "Slack", Linux),
];

const EMAIL_APPS: &[PresetApp] = &[
    app("com.apple.mail", "Mail", Macos),
    app("com.microsoft.Outlook", "Microsoft Outlook", Macos),
    app("OUTLOOK.EXE", "Microsoft Outlook", Windows),
    app("olk.exe", "Outlook (new)", Windows),
    app("thunderbird", "Thunderbird", Linux),
];

const TERMINAL_APPS: &[PresetApp] = &[
    app("com.apple.Terminal", "Terminal", Macos),
    app("com.googlecode.iterm2", "iTerm2", Macos),
    app("com.anthropic.claudefordesktop", "Claude Code desktop", Macos),
    app("WindowsTerminal.exe", "Windows Terminal", Windows),
    app("claude.exe", "Claude Code desktop", Windows),
    app("Gnome-terminal", "GNOME Terminal", Linux),
    app("org.gnome.Ptyxis", "Ptyxis", Linux),
    app("konsole", "Konsole", Linux),
    app("kitty", "kitty", Linux),
    app("Alacritty", "Alacritty", Linux),
];

/// The gallery, in display order.
pub const PRESETS: &[Preset] = &[
    Preset {
        id: "writing-code",
        name: "Writing code",
        summary: "Identifiers, files and flags in code form; spoken steps become a numbered list.",
        style: Style::Standard,
        raw: false,
        prompt: "Write file names, function and variable names, commands and flags in their code form, for example getUserById, auth.ts or --force. Keep my wording and the order I said things in. When I describe steps one after another, turn them into a numbered list.",
        apps: CODE_APPS,
        sites: &[],
        builtin_email: false,
    },
    Preset {
        id: "ai-chat",
        name: "AI chat",
        summary: "Prompts for assistants: questions stay questions, lists become numbered points.",
        style: Style::Standard,
        raw: false,
        prompt: "This text is a prompt I am writing for an AI assistant. Keep my wording. A question stays a question; never answer it. When I list several things, make them numbered points. Put anything I quote on its own lines.",
        apps: AI_CHAT_APPS,
        sites: AI_CHAT_SITES,
        builtin_email: false,
    },
    Preset {
        id: "team-chat",
        name: "Team chat",
        summary: "Short messages in your own tone, one point per line, no added greetings.",
        style: Style::Casual,
        raw: false,
        prompt: "Keep it short and in my own tone. Do not add a greeting or a sign-off unless I said one. When I give an update with several points, put each point on its own line.",
        apps: TEAM_CHAT_APPS,
        sites: &["app.slack.com"],
        builtin_email: false,
    },
    Preset {
        id: "email",
        name: "Email",
        summary: "Greeting on its own line, short paragraphs, a sign-off only when you say one.",
        style: Style::Standard,
        raw: false,
        prompt: "Put the greeting on its own line, then write short paragraphs. Add a sign-off only if I said one.",
        apps: EMAIL_APPS,
        sites: &[],
        builtin_email: true,
    },
    Preset {
        id: "terminal",
        name: "Terminal",
        summary: "Raw text, so paths, flags and branch names come out exactly as spoken.",
        style: Style::Standard,
        raw: true,
        prompt: "",
        apps: TERMINAL_APPS,
        sites: &[],
        builtin_email: false,
    },
    Preset {
        id: "friendly-filter",
        name: "Friendly filter",
        summary: "Takes the heat out of what you send to AI chats while keeping the request.",
        style: Style::Standard,
        raw: false,
        prompt: "Leave out insults, swearing and angry or abusive wording. Keep what I am asking for and every detail of the request, phrased calmly and plainly.",
        apps: AI_CHAT_APPS,
        sites: AI_CHAT_SITES,
        builtin_email: false,
    },
];

pub fn find(id: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|p| p.id == id)
}

/// An app or site the preset wanted but another rule already owns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Skipped {
    /// The app's name or the site's domain.
    pub item: String,
    /// The owning rule's name.
    pub owner: String,
}

/// The rule to save after applying a preset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Applied {
    /// A new custom rule, or the updated built-in Email rule.
    pub rule: Rule,
    /// `(id, name)` for the apps the preset added.
    pub added_apps: Vec<(String, String)>,
    pub skipped: Vec<Skipped>,
    /// The Email rule already had a prompt of its own, which was kept.
    pub kept_prompt: bool,
}

impl Applied {
    /// The preset could not claim a single app or site.
    pub fn is_empty(&self) -> bool {
        self.rule.apps.is_empty() && self.rule.sites.is_empty()
    }
}

/// Apply `preset` to `rules` for apps on `os` (`None`: every OS). `new_id` is
/// the id for a new custom rule.
pub fn apply(preset: &Preset, rules: &RuleSet, os: Option<Os>, new_id: &str) -> Applied {
    let email = rules.rules.iter().find(|r| r.kind == RuleKind::Email);
    let (mut rule, kept_prompt) = match (preset.builtin_email, email) {
        (true, Some(e)) => {
            let mut r = e.clone();
            r.enabled = true;
            r.style = preset.style;
            r.raw = preset.raw;
            let kept = !r.prompt.trim().is_empty() && r.prompt.trim() != preset.prompt;
            if !kept {
                r.prompt = preset.prompt.to_owned();
            }
            (r, kept)
        }
        _ => {
            let mut r = Rule::new(new_id, preset.name, RuleKind::Custom);
            r.style = preset.style;
            r.raw = preset.raw;
            r.prompt = preset.prompt.to_owned();
            (r, false)
        }
    };
    let others: Vec<&Rule> = rules.rules.iter().filter(|r| r.id != rule.id).collect();
    let app_owner = |id: &str| {
        let id = id.to_lowercase();
        others
            .iter()
            .find(|r| r.apps.iter().any(|a| a.trim().to_lowercase() == id))
            .map(|r| r.name.clone())
    };
    let site_owner = |site: &str| {
        others
            .iter()
            .find(|r| r.sites.iter().any(|s| normalize_host(s) == site))
            .map(|r| r.name.clone())
    };
    let mut skipped = Vec::new();
    let mut added_apps = Vec::new();
    for a in preset.apps.iter().filter(|a| os.is_none_or(|os| a.os == os)) {
        if rule.apps.iter().any(|x| x.eq_ignore_ascii_case(a.id)) {
            continue;
        }
        match app_owner(a.id) {
            Some(owner) => skipped.push(Skipped {
                item: a.name.to_owned(),
                owner,
            }),
            None => {
                rule.apps.push(a.id.to_owned());
                added_apps.push((a.id.to_owned(), a.name.to_owned()));
            }
        }
    }
    for s in preset.sites {
        let s = normalize_host(s);
        if rule.sites.iter().any(|x| normalize_host(x) == s) {
            continue;
        }
        match site_owner(&s) {
            Some(owner) => skipped.push(Skipped { item: s, owner }),
            None => rule.sites.push(s),
        }
    }
    Applied {
        rule,
        added_apps,
        skipped,
        kept_prompt,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formatting::MAX_PROMPT_CHARS;
    use crate::text::char_len;

    #[test]
    fn gallery_is_complete_and_within_limits() {
        let ids: Vec<_> = PRESETS.iter().map(|p| p.id).collect();
        assert_eq!(
            ids,
            [
                "writing-code",
                "ai-chat",
                "team-chat",
                "email",
                "terminal",
                "friendly-filter"
            ]
        );
        for p in PRESETS {
            assert!(char_len(p.prompt) <= MAX_PROMPT_CHARS, "{}", p.id);
            assert!(!p.apps.is_empty() || !p.sites.is_empty(), "{}", p.id);
            assert!(
                p.apps.iter().all(|a| !crate::apps::is_excluded(a.id, a.name)),
                "{}",
                p.id
            );
        }
        let t = find("terminal").unwrap();
        assert!(t.raw && t.style == Style::Standard);
        assert_eq!(find("team-chat").unwrap().style, Style::Casual);
        assert!(find("nope").is_none());
    }

    #[test]
    fn every_preset_adds_cleanly_to_the_default_rules() {
        for p in PRESETS {
            let mut rs = RuleSet::default();
            let a = apply(p, &rs, Some(Os::Macos), "new");
            assert!(a.skipped.is_empty(), "{}", p.id);
            if p.builtin_email {
                let i = rs.rules.iter().position(|r| r.kind == RuleKind::Email).unwrap();
                rs.rules[i] = a.rule;
            } else {
                rs.rules.push(a.rule);
            }
            rs.validate().unwrap();
        }
    }

    #[test]
    fn only_apps_for_this_os() {
        let a = apply(find("writing-code").unwrap(), &RuleSet::default(), Some(Os::Linux), "n");
        assert_eq!(a.rule.apps, vec!["Code", "Cursor"]);
        let a = apply(find("writing-code").unwrap(), &RuleSet::default(), None, "n");
        assert_eq!(a.rule.apps.len(), 6);
    }

    #[test]
    fn owned_apps_and_sites_are_skipped_and_named() {
        let mut rs = RuleSet::default();
        let ai = apply(find("ai-chat").unwrap(), &rs, Some(Os::Macos), "ai");
        rs.rules.push(ai.rule);
        let f = apply(find("friendly-filter").unwrap(), &rs, Some(Os::Macos), "ff");
        assert!(f.is_empty());
        assert_eq!(
            f.skipped,
            vec![
                Skipped {
                    item: "ChatGPT".into(),
                    owner: "AI chat".into()
                },
                Skipped {
                    item: "chatgpt.com".into(),
                    owner: "AI chat".into()
                },
                Skipped {
                    item: "claude.ai".into(),
                    owner: "AI chat".into()
                },
                Skipped {
                    item: "gemini.google.com".into(),
                    owner: "AI chat".into()
                },
            ]
        );
        // A partly owned preset keeps the rest.
        let mut slack = Rule::new("s", "My Slack", RuleKind::Custom);
        slack.apps = vec!["COM.TINYSPECK.SLACKMACGAP".into()];
        rs.rules.push(slack);
        let t = apply(find("team-chat").unwrap(), &rs, Some(Os::Macos), "tc");
        assert_eq!(t.rule.apps, Vec::<String>::new());
        assert_eq!(t.rule.sites, vec!["app.slack.com"]);
        assert_eq!(t.skipped[0].owner, "My Slack");
    }

    #[test]
    fn email_preset_fills_the_builtin_rule_and_keeps_a_custom_prompt() {
        let mut rs = RuleSet::default();
        rs.rules[1].enabled = false;
        let a = apply(find("email").unwrap(), &rs, Some(Os::Macos), "unused");
        assert_eq!(a.rule.id, "email");
        assert!(a.rule.enabled);
        assert_eq!(a.rule.sites, vec!["mail.google.com", "outlook.live.com"]);
        assert_eq!(a.rule.apps, vec!["com.apple.mail", "com.microsoft.Outlook"]);
        assert!(!a.kept_prompt);
        assert!(a.rule.prompt.starts_with("Put the greeting"));

        rs.rules[1].prompt = "Sign every email with my initials.".into();
        let a = apply(find("email").unwrap(), &rs, Some(Os::Macos), "unused");
        assert!(a.kept_prompt);
        assert_eq!(a.rule.prompt, "Sign every email with my initials.");
    }
}
