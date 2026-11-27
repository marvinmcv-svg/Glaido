//! Platform-neutral key identifiers.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Key {
    Fn,
    LeftCtrl,
    RightCtrl,
    LeftAlt,
    RightAlt,
    LeftShift,
    RightShift,
    /// ⌘ on macOS, Win on Windows, Super on Linux.
    LeftMeta,
    RightMeta,
    Space,
    Enter,
    Escape,
    Tab,
    Backspace,
    /// Letters, digits and other printable keys, by lowercase character.
    Char(char),
    /// F1–F24.
    F(u8),
    /// Extra mouse buttons (back/forward and beyond), numbered from 4.
    Mouse(u8),
    /// Anything else, by native code, so it can still be bound.
    Other(u32),
}

impl Key {
    pub fn is_modifier(self) -> bool {
        matches!(
            self,
            Key::Fn
                | Key::LeftCtrl
                | Key::RightCtrl
                | Key::LeftAlt
                | Key::RightAlt
                | Key::LeftShift
                | Key::RightShift
                | Key::LeftMeta
                | Key::RightMeta
        )
    }

    /// The settings name, the inverse of [`Key::parse`].
    pub fn name(self) -> String {
        match self {
            Key::Fn => "fn".into(),
            Key::LeftCtrl => "left_ctrl".into(),
            Key::RightCtrl => "right_ctrl".into(),
            Key::LeftAlt => "left_alt".into(),
            Key::RightAlt => "right_alt".into(),
            Key::LeftShift => "shift".into(),
            Key::RightShift => "right_shift".into(),
            Key::LeftMeta => "left_meta".into(),
            Key::RightMeta => "right_meta".into(),
            Key::Space => "space".into(),
            Key::Enter => "enter".into(),
            Key::Escape => "esc".into(),
            Key::Tab => "tab".into(),
            Key::Backspace => "backspace".into(),
            Key::Char('+') => "plus".into(),
            Key::Char(c) => c.to_string(),
            Key::F(n) => format!("f{n}"),
            Key::Mouse(n) => format!("mouse{n}"),
            Key::Other(code) => format!("code{code}"),
        }
    }

    /// Parse names used in settings: `fn`, `left_ctrl`, `right_option`, `cmd`, `space`, `k`, `f5`, `mouse4`.
    /// Side-less modifier names mean the left key.
    pub fn parse(s: &str) -> Option<Key> {
        let s = s.trim().to_lowercase().replace(['-', ' '], "_");
        Some(match s.as_str() {
            "fn" | "globe" => Key::Fn,
            "ctrl" | "control" | "left_ctrl" | "left_control" => Key::LeftCtrl,
            "right_ctrl" | "right_control" => Key::RightCtrl,
            "alt" | "option" | "left_alt" | "left_option" => Key::LeftAlt,
            "right_alt" | "right_option" | "altgr" => Key::RightAlt,
            "shift" | "left_shift" => Key::LeftShift,
            "right_shift" => Key::RightShift,
            "cmd" | "command" | "meta" | "win" | "super" | "left_cmd" | "left_meta" | "left_win" | "left_super" => {
                Key::LeftMeta
            }
            "right_cmd" | "right_meta" | "right_win" | "right_super" => Key::RightMeta,
            "space" => Key::Space,
            "enter" | "return" => Key::Enter,
            "esc" | "escape" => Key::Escape,
            "tab" => Key::Tab,
            "backspace" => Key::Backspace,
            "plus" => Key::Char('+'),
            _ => {
                if let Some(code) = s.strip_prefix("code").and_then(|n| n.parse().ok()) {
                    Key::Other(code)
                } else if let Some(n) = s
                    .strip_prefix("mouse")
                    .and_then(|n| n.parse().ok())
                    .filter(|n: &u8| *n >= 4)
                {
                    Key::Mouse(n)
                } else if let Some(n) = s
                    .strip_prefix('f')
                    .and_then(|n| n.parse().ok())
                    .filter(|n: &u8| (1..=24).contains(n))
                {
                    Key::F(n)
                } else {
                    let mut chars = s.chars();
                    match (chars.next(), chars.next()) {
                        (Some(c), None) if !c.is_whitespace() => Key::Char(c),
                        _ => return None,
                    }
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_settings_names() {
        assert_eq!(Key::parse("fn"), Some(Key::Fn));
        assert_eq!(Key::parse("Right Option"), Some(Key::RightAlt));
        assert_eq!(Key::parse("ctrl"), Some(Key::LeftCtrl));
        assert_eq!(Key::parse("win"), Some(Key::LeftMeta));
        assert_eq!(Key::parse("K"), Some(Key::Char('k')));
        assert_eq!(Key::parse("f12"), Some(Key::F(12)));
        assert_eq!(Key::parse("f25"), None);
        assert_eq!(Key::parse("mouse4"), Some(Key::Mouse(4)));
        assert_eq!(Key::parse("mouse1"), None, "primary buttons can't be hotkeys");
        assert_eq!(Key::parse("banana"), None);
    }

    #[test]
    fn names_round_trip() {
        let keys = [
            Key::Fn,
            Key::LeftCtrl,
            Key::RightCtrl,
            Key::LeftAlt,
            Key::RightAlt,
            Key::LeftShift,
            Key::RightShift,
            Key::LeftMeta,
            Key::RightMeta,
            Key::Space,
            Key::Enter,
            Key::Escape,
            Key::Tab,
            Key::Backspace,
            Key::Char('k'),
            Key::Char('7'),
            Key::Char('+'),
            Key::Char(';'),
            Key::F(5),
            Key::F(24),
            Key::Mouse(4),
            Key::Other(314),
        ];
        for k in keys {
            assert_eq!(Key::parse(&k.name()), Some(k), "{k:?}");
        }
    }
}
