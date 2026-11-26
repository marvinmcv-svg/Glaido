//! Global key listener (feature `listen`), built on rdev:
//! CGEventTap on macOS (needs Accessibility / Input Monitoring permission),
//! low-level hooks on Windows, X11 record on Linux.
//!
//! Known limits, tracked for P5 (docs/spec/4-10): Linux Wayland sessions need the
//! evdev reader or the GlobalShortcuts portal instead; macOS Fn/Globe detection
//! depends on the keyboard and is verified per device in the setup wizard.

use std::sync::mpsc::{channel, Receiver};
use std::time::Instant;

use crate::chord::KeyEvent;
use crate::key::Key;

/// Translate an rdev key into ours.
pub fn map_key(k: rdev::Key) -> Key {
    use rdev::Key as R;
    match k {
        R::Function => Key::Fn,
        R::ControlLeft => Key::LeftCtrl,
        R::ControlRight => Key::RightCtrl,
        R::Alt => Key::LeftAlt,
        R::AltGr => Key::RightAlt,
        R::ShiftLeft => Key::LeftShift,
        R::ShiftRight => Key::RightShift,
        R::MetaLeft => Key::LeftMeta,
        R::MetaRight => Key::RightMeta,
        R::Space => Key::Space,
        R::Return | R::KpReturn => Key::Enter,
        R::Escape => Key::Escape,
        R::Tab => Key::Tab,
        R::Backspace => Key::Backspace,
        R::F1 => Key::F(1),
        R::F2 => Key::F(2),
        R::F3 => Key::F(3),
        R::F4 => Key::F(4),
        R::F5 => Key::F(5),
        R::F6 => Key::F(6),
        R::F7 => Key::F(7),
        R::F8 => Key::F(8),
        R::F9 => Key::F(9),
        R::F10 => Key::F(10),
        R::F11 => Key::F(11),
        R::F12 => Key::F(12),
        R::Unknown(code) => Key::Other(code),
        other => match key_char(other) {
            Some(c) => Key::Char(c),
            None => Key::Other(u32::MAX),
        },
    }
}

fn key_char(k: rdev::Key) -> Option<char> {
    use rdev::Key as R;
    let letters = [
        (R::KeyA, 'a'),
        (R::KeyB, 'b'),
        (R::KeyC, 'c'),
        (R::KeyD, 'd'),
        (R::KeyE, 'e'),
        (R::KeyF, 'f'),
        (R::KeyG, 'g'),
        (R::KeyH, 'h'),
        (R::KeyI, 'i'),
        (R::KeyJ, 'j'),
        (R::KeyK, 'k'),
        (R::KeyL, 'l'),
        (R::KeyM, 'm'),
        (R::KeyN, 'n'),
        (R::KeyO, 'o'),
        (R::KeyP, 'p'),
        (R::KeyQ, 'q'),
        (R::KeyR, 'r'),
        (R::KeyS, 's'),
        (R::KeyT, 't'),
        (R::KeyU, 'u'),
        (R::KeyV, 'v'),
        (R::KeyW, 'w'),
        (R::KeyX, 'x'),
        (R::KeyY, 'y'),
        (R::KeyZ, 'z'),
        (R::Num0, '0'),
        (R::Num1, '1'),
        (R::Num2, '2'),
        (R::Num3, '3'),
        (R::Num4, '4'),
        (R::Num5, '5'),
        (R::Num6, '6'),
        (R::Num7, '7'),
        (R::Num8, '8'),
        (R::Num9, '9'),
        (R::Comma, ','),
        (R::Dot, '.'),
        (R::Slash, '/'),
        (R::SemiColon, ';'),
        (R::Quote, '\''),
        (R::LeftBracket, '['),
        (R::RightBracket, ']'),
        (R::BackSlash, '\\'),
        (R::Minus, '-'),
        (R::Equal, '='),
        (R::BackQuote, '`'),
    ];
    letters.iter().find(|(r, _)| *r == k).map(|(_, c)| *c)
}

fn map_button(b: rdev::Button) -> Option<Key> {
    match b {
        rdev::Button::Unknown(n) if n >= 4 => Some(Key::Mouse(n)),
        _ => None,
    }
}

/// Start listening on a dedicated thread. Events carry milliseconds since
/// `epoch`. If the OS refuses (e.g. missing permission) the receiver closes.
pub fn start(epoch: Instant) -> Receiver<KeyEvent> {
    let (tx, rx) = channel();
    std::thread::Builder::new()
        .name("vocalis-hotkeys".into())
        .spawn(move || {
            let result = rdev::listen(move |ev| {
                let t = epoch.elapsed().as_millis() as u64;
                let out = match ev.event_type {
                    rdev::EventType::KeyPress(k) => Some(KeyEvent::Down(map_key(k), t)),
                    rdev::EventType::KeyRelease(k) => Some(KeyEvent::Up(map_key(k), t)),
                    rdev::EventType::ButtonPress(b) => map_button(b).map(|k| KeyEvent::Down(k, t)),
                    rdev::EventType::ButtonRelease(b) => map_button(b).map(|k| KeyEvent::Up(k, t)),
                    _ => None,
                };
                if let Some(e) = out {
                    let _ = tx.send(e);
                }
            });
            if let Err(e) = result {
                eprintln!("vocalis: hotkey listener stopped: {e:?}");
            }
        })
        .expect("spawn hotkey thread");
    rx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_keys() {
        assert_eq!(map_key(rdev::Key::Function), Key::Fn);
        assert_eq!(map_key(rdev::Key::AltGr), Key::RightAlt);
        assert_eq!(map_key(rdev::Key::KeyK), Key::Char('k'));
        assert_eq!(map_key(rdev::Key::Unknown(179)), Key::Other(179));
        assert_eq!(map_button(rdev::Button::Unknown(4)), Some(Key::Mouse(4)));
        assert_eq!(map_button(rdev::Button::Left), None);
    }
}
