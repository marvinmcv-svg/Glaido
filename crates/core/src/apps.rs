//! App identity helpers for the Formatting page (spec 4.5.3): which apps can be
//! targets of an app rule. Browsers are excluded (websites are matched by URL
//! instead), and so is our own app.
//!
//! Matching is by whole tokens of the app id and name, as in the field-kind
//! heuristic, so "Edge" matches `msedge.exe` without matching "Knowledgebase".

use crate::product::PRODUCT_NAME;

const BROWSERS: &[&str] = &[
    "chrome",
    "chromium",
    "firefox",
    "safari",
    "msedge",
    "edgemac",
    "edge",
    "brave",
    "opera",
    "vivaldi",
    "arc",
    "thebrowser",
    "librewolf",
    "waterfox",
    "floorp",
    "orion",
    "zen",
    "epiphany",
    "falkon",
    "qutebrowser",
    "iexplore",
    "yandex",
    "duckduckgo",
    "sidekick",
];

fn tokens(s: &str) -> impl Iterator<Item = String> + '_ {
    s.split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|t| !t.is_empty())
        .map(str::to_lowercase)
}

/// True for web browsers on any OS (bundle id, exe name or WM_CLASS, plus the name).
pub fn is_browser(app_id: &str, name: &str) -> bool {
    tokens(app_id)
        .chain(tokens(name))
        .any(|t| BROWSERS.contains(&t.as_str()) || t.ends_with("browser"))
}

/// True for this app itself (`app.vocalis.desktop`, `Vocalis.exe`, `vocalis`).
pub fn is_own_app(app_id: &str, name: &str) -> bool {
    let own = PRODUCT_NAME.to_lowercase();
    tokens(app_id).chain(tokens(name)).any(|t| t == own)
}

/// Apps that may not be the target of an app rule.
pub fn is_excluded(app_id: &str, name: &str) -> bool {
    app_id.trim().is_empty() || is_browser(app_id, name) || is_own_app(app_id, name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browsers_on_every_os() {
        for (id, name) in [
            ("com.google.Chrome", "Google Chrome"),
            ("chrome.exe", "Google Chrome"),
            ("Google-chrome", "Google-chrome"),
            ("org.mozilla.firefox", "Firefox"),
            ("firefox", "firefox"),
            ("com.apple.Safari", "Safari"),
            ("msedge.exe", "Microsoft Edge"),
            ("com.microsoft.edgemac", "Microsoft Edge"),
            ("company.thebrowser.Browser", "Arc"),
            ("com.brave.Browser", "Brave Browser"),
            ("Navigator", "Firefox"),
        ] {
            assert!(is_browser(id, name), "{id} / {name}");
        }
        for (id, name) in [
            ("com.microsoft.VSCode", "Code"),
            ("com.tinyspeck.slackmacgap", "Slack"),
            ("com.apple.Terminal", "Terminal"),
            ("org.example.knowledgebase", "Knowledgebase"),
        ] {
            assert!(!is_browser(id, name), "{id} / {name}");
        }
    }

    #[test]
    fn own_app_and_empty_ids_are_excluded() {
        assert!(is_excluded("app.vocalis.desktop", "Vocalis"));
        assert!(is_excluded("Vocalis.exe", "Vocalis"));
        assert!(is_excluded("  ", "Notes"));
        assert!(!is_excluded("com.apple.Notes", "Notes"));
    }
}
