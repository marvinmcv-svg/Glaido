//! Real clipboard (arboard) and keystrokes (enigo), feature `os`.
//!
//! arboard holds text and images; other formats (files, rich text) on the
//! user's clipboard are not restored yet. Native NSPasteboard / Win32
//! multi-format save/restore is tracked for P5 (docs/spec/4-10).

use std::borrow::Cow;

use enigo::{Direction, Enigo, Key, Keyboard, Settings};

use crate::{ClipContent, Clipboard, InjectError, Keystrokes, PasteCombo};

pub struct OsClipboard(arboard::Clipboard);

impl OsClipboard {
    pub fn new() -> Result<Self, InjectError> {
        arboard::Clipboard::new()
            .map(Self)
            .map_err(|e| InjectError::Clipboard(e.to_string()))
    }
}

impl Clipboard for OsClipboard {
    fn get(&mut self) -> Result<Option<ClipContent>, InjectError> {
        if let Ok(t) = self.0.get_text() {
            return Ok(Some(ClipContent::Text(t)));
        }
        if let Ok(img) = self.0.get_image() {
            return Ok(Some(ClipContent::Image {
                width: img.width,
                height: img.height,
                rgba: img.bytes.into_owned(),
            }));
        }
        Ok(None)
    }

    fn set(&mut self, content: &ClipContent) -> Result<(), InjectError> {
        let r = match content {
            ClipContent::Text(t) => self.0.set_text(t.clone()),
            ClipContent::Image { width, height, rgba } => self.0.set_image(arboard::ImageData {
                width: *width,
                height: *height,
                bytes: Cow::Borrowed(rgba),
            }),
        };
        r.map_err(|e| InjectError::Clipboard(e.to_string()))
    }

    fn clear(&mut self) -> Result<(), InjectError> {
        self.0.clear().map_err(|e| InjectError::Clipboard(e.to_string()))
    }
}

pub struct OsKeystrokes(Enigo);

impl OsKeystrokes {
    pub fn new() -> Result<Self, InjectError> {
        Enigo::new(&Settings::default())
            .map(Self)
            .map_err(|e| InjectError::Keystroke(e.to_string()))
    }
}

impl Keystrokes for OsKeystrokes {
    fn paste(&mut self, combo: PasteCombo) -> Result<(), InjectError> {
        let mods: &[Key] = match combo {
            PasteCombo::CmdV => &[Key::Meta],
            PasteCombo::CtrlV => &[Key::Control],
            PasteCombo::CtrlShiftV => &[Key::Control, Key::Shift],
        };
        let err = |e: enigo::InputError| InjectError::Keystroke(e.to_string());
        for m in mods {
            self.0.key(*m, Direction::Press).map_err(err)?;
        }
        let result = self.0.key(Key::Unicode('v'), Direction::Click).map_err(err);
        // Always release modifiers, even if the click failed.
        for m in mods.iter().rev() {
            let _ = self.0.key(*m, Direction::Release);
        }
        result
    }
}
