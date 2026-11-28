//! Field-kind heuristic over the app identity and, for browsers, the URL.
//! Matching is by whole tokens of the bundle id / exe / WM_CLASS and name,
//! so "code" matches VS Code ("com.microsoft.VSCode" → `vscode`, "Code.exe" →
//! `code`) without matching "Xcode" by accident (that has its own entry).

use vocalis_core::formatting::FieldKind;

const TERMINAL: &[&str] = &[
    "terminal",
    "iterm2",
    "iterm",
    "alacritty",
    "kitty",
    "wezterm",
    "konsole",
    "xterm",
    "urxvt",
    "tilix",
    "terminator",
    "ghostty",
    "foot",
    "warp",
    "hyper",
    "windowsterminal",
    "conhost",
    "cmd",
    "powershell",
    "pwsh",
    "mintty",
    "terminology",
    "ptyxis",
    "kgx",
    "termius",
    "tabby",
];
const CODE: &[&str] = &[
    "vscode",
    "code",
    "vscodium",
    "codium",
    "cursor",
    "zed",
    "xcode",
    "sublime",
    "sublime_text",
    "idea",
    "pycharm",
    "webstorm",
    "goland",
    "clion",
    "rider",
    "rubymine",
    "phpstorm",
    "datagrip",
    "fleet",
    "androidstudio",
    "studio64",
    "nova",
    "bbedit",
    "emacs",
    "gvim",
    "neovide",
    "kate",
    "windsurf",
    "devenv",
];
const EMAIL: &[&str] = &[
    "mail",
    "outlook",
    "olk",
    "thunderbird",
    "spark",
    "superhuman",
    "airmail",
    "mimestream",
    "geary",
    "evolution",
    "mailspring",
    "betterbird",
    "postbox",
    "hey",
];
const CHAT: &[&str] = &[
    "slack",
    "discord",
    "teams",
    "msteams",
    "telegram",
    "whatsapp",
    "signal",
    "mobilesms",
    "messages",
    "element",
    "skype",
    "mattermost",
    "zulip",
    "wechat",
    "line",
    "messenger",
    "beeper",
    "chatgpt",
    "claude",
];
const DOCUMENT: &[&str] = &[
    "word",
    "winword",
    "pages",
    "notion",
    "obsidian",
    "notes",
    "bear",
    "craft",
    "ulysses",
    "scrivener",
    "writer",
    "soffice",
    "libreoffice",
    "onenote",
    "evernote",
    "typora",
    "ia",
    "logseq",
    "textedit",
    "wordpad",
    "notepad",
];

fn tokens(s: &str) -> impl Iterator<Item = String> + '_ {
    s.split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|t| !t.is_empty())
        .map(str::to_lowercase)
}

/// Field kind from the app alone. Browsers come back `Generic`; use
/// [`field_kind_for_url`] when the URL is known.
pub fn field_kind(app_id: &str, name: &str) -> FieldKind {
    let toks: Vec<String> = tokens(app_id).chain(tokens(name)).filter(|t| t != "exe").collect();
    let has = |list: &[&str]| toks.iter().any(|t| list.contains(&t.as_str()));
    // Order matters: "Microsoft Outlook" is email even though Microsoft also ships Word.
    if has(TERMINAL) {
        FieldKind::Terminal
    } else if has(CODE) {
        FieldKind::Code
    } else if has(EMAIL) {
        FieldKind::Email
    } else if has(CHAT) {
        FieldKind::Chat
    } else if has(DOCUMENT) {
        FieldKind::Document
    } else {
        FieldKind::Generic
    }
}

/// Field kind for a web page by host (e.g. Gmail in a browser is email).
pub fn field_kind_for_url(url: &str) -> FieldKind {
    let host = url
        .split("://")
        .nth(1)
        .unwrap_or(url)
        .split(['/', '?', '#', ':'])
        .next()
        .unwrap_or("")
        .to_lowercase();
    let is = |domain: &str| host == domain || host.ends_with(&format!(".{domain}"));
    if is("mail.google.com")
        || is("outlook.live.com")
        || is("outlook.office.com")
        || is("mail.yahoo.com")
        || is("app.fastmail.com")
        || is("mail.proton.me")
    {
        FieldKind::Email
    } else if is("slack.com")
        || is("discord.com")
        || is("web.whatsapp.com")
        || is("teams.microsoft.com")
        || is("messenger.com")
        || is("chatgpt.com")
        || is("claude.ai")
        || is("web.telegram.org")
    {
        FieldKind::Chat
    } else if is("docs.google.com") || is("notion.so") || is("notion.site") || is("dropbox.com") || is("quip.com") {
        FieldKind::Document
    } else if is("github.com")
        || is("gitlab.com")
        || is("bitbucket.org")
        || is("replit.com")
        || is("codesandbox.io")
        || is("vscode.dev")
    {
        FieldKind::Code
    } else {
        FieldKind::Generic
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use FieldKind::*;

    #[test]
    fn the_ten_qa_apps_and_friends() {
        let cases = [
            ("com.tinyspeck.slackmacgap", "Slack", Chat),
            ("slack.exe", "Slack", Chat),
            ("Slack", "Slack", Chat),
            ("com.apple.Notes", "Notes", Document),
            ("com.microsoft.VSCode", "Code", Code),
            ("Code.exe", "Code", Code),
            ("code", "Code", Code),
            ("com.todesktop.230313mzl4w4u92", "Cursor", Code),
            ("com.apple.Terminal", "Terminal", Terminal),
            ("gnome-terminal-server", "Gnome-terminal", Terminal),
            ("WindowsTerminal.exe", "WindowsTerminal", Terminal),
            ("com.googlecode.iterm2", "iTerm2", Terminal),
            ("notion.id", "Notion", Document),
            ("com.microsoft.Word", "Microsoft Word", Document),
            ("WINWORD.EXE", "WINWORD", Document),
            ("com.apple.MobileSMS", "Messages", Chat),
            ("com.apple.mail", "Mail", Email),
            ("com.microsoft.Outlook", "Microsoft Outlook", Email),
            ("com.apple.dt.Xcode", "Xcode", Code),
            ("com.google.Chrome", "Google Chrome", Generic),
            ("firefox", "Firefox", Generic),
            ("com.apple.finder", "Finder", Generic),
        ];
        for (id, name, kind) in cases {
            assert_eq!(field_kind(id, name), kind, "{id} / {name}");
        }
    }

    #[test]
    fn whole_tokens_only() {
        assert_eq!(
            field_kind("org.example.decoder", "Decoder"),
            Generic,
            "no substring 'code'"
        );
        assert_eq!(field_kind("com.example.mailchimp", "Mailchimp"), Generic);
    }

    #[test]
    fn urls() {
        assert_eq!(field_kind_for_url("https://mail.google.com/mail/u/0/#inbox"), Email);
        assert_eq!(field_kind_for_url("https://chatgpt.com/c/123"), Chat);
        assert_eq!(field_kind_for_url("https://acme.slack.com/client"), Chat);
        assert_eq!(field_kind_for_url("https://docs.google.com/document/d/x"), Document);
        assert_eq!(field_kind_for_url("https://github.com/a/b/pull/1"), Code);
        assert_eq!(
            field_kind_for_url("https://notslack.com"),
            Generic,
            "suffix needs a dot"
        );
        assert_eq!(field_kind_for_url("not a url"), Generic);
    }
}
