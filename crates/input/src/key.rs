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
            _ => {
                if let Some(n) = s
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
}
