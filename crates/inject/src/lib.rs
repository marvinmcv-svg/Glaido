//! Paste injection (master prompt §5.2 "Injection", spec 4.1 "Paste semantics").
//!
//! save the clipboard → put the text on it → send the platform's paste
//! keystroke → wait → put the user's clipboard back, unless they copied
//! something new in the meantime (or "Copy to clipboard" is on).
//!
//! The algorithm is written against the [`Clipboard`] and [`Keystrokes`]
//! traits so it is tested without a desktop; the `os` feature provides
//! arboard/enigo implementations.

#[cfg(feature = "os")]
pub mod os;

use std::time::Duration;

use thiserror::Error;
use vocalis_core::formatting::FieldKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipContent {
    Text(String),
    /// RGBA8 pixels.
    Image {
        width: usize,
        height: usize,
        rgba: Vec<u8>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum InjectError {
    #[error("the clipboard is unavailable: {0}")]
    Clipboard(String),
    #[error("the paste keystroke could not be sent: {0}")]
    Keystroke(String),
}

pub trait Clipboard {
    /// Current contents, or `None` when empty or in a format we can't hold.
    fn get(&mut self) -> Result<Option<ClipContent>, InjectError>;
    fn set(&mut self, content: &ClipContent) -> Result<(), InjectError>;
    fn clear(&mut self) -> Result<(), InjectError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteCombo {
    /// ⌘V (macOS)
    CmdV,
    /// Ctrl+V (Windows, Linux)
    CtrlV,
    /// Ctrl+Shift+V (Linux terminals)
    CtrlShiftV,
}

pub trait Keystrokes {
    fn paste(&mut self, combo: PasteCombo) -> Result<(), InjectError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    MacOs,
    Windows,
    Linux,
}

impl Os {
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            Os::MacOs
        } else if cfg!(target_os = "windows") {
            Os::Windows
        } else {
            Os::Linux
        }
    }
}

/// The paste keystroke for a platform and destination.
pub fn combo_for(os: Os, field: FieldKind) -> PasteCombo {
    match (os, field) {
        (Os::MacOs, _) => PasteCombo::CmdV,
        (Os::Linux, FieldKind::Terminal) => PasteCombo::CtrlShiftV,
        _ => PasteCombo::CtrlV,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PasteOptions {
    pub combo: PasteCombo,
    /// How long the target app gets to read the clipboard before it is restored.
    pub restore_delay: Duration,
    /// "Copy to clipboard" setting: leave the result on the clipboard.
    pub keep_on_clipboard: bool,
}

impl PasteOptions {
    pub fn new(combo: PasteCombo) -> Self {
        Self {
            combo,
            restore_delay: Duration::from_millis(250),
            keep_on_clipboard: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Restore {
    /// The user's previous clipboard is back.
    Restored,
    /// Left as is: "Copy to clipboard" is on.
    KeptByOption,
    /// Left as is: the user copied something new after we set the clipboard.
    UserCopiedSince,
}

/// Paste `text` at the cursor of the focused app.
///
/// On a keystroke failure the clipboard is restored and the error returned, so
/// the caller can offer "Copy" (spec: "Could not paste").
pub fn paste(
    text: &str,
    clip: &mut dyn Clipboard,
    keys: &mut dyn Keystrokes,
    opts: PasteOptions,
    sleep: &mut dyn FnMut(Duration),
) -> Result<Restore, InjectError> {
    let saved = clip.get()?;
    let ours = ClipContent::Text(text.to_owned());
    clip.set(&ours)?;
    if let Err(e) = keys.paste(opts.combo) {
        put_back(clip, saved.as_ref())?;
        return Err(e);
    }
    if opts.keep_on_clipboard {
        return Ok(Restore::KeptByOption);
    }
    sleep(opts.restore_delay);
    if clip.get()?.as_ref() != Some(&ours) {
        return Ok(Restore::UserCopiedSince);
    }
    put_back(clip, saved.as_ref())?;
    Ok(Restore::Restored)
}

fn put_back(clip: &mut dyn Clipboard, saved: Option<&ClipContent>) -> Result<(), InjectError> {
    match saved {
        Some(c) => clip.set(c),
        None => clip.clear(),
    }
}

/// Put text on the clipboard for the "Copy" action of a notification.
pub fn copy(text: &str, clip: &mut dyn Clipboard) -> Result<(), InjectError> {
    clip.set(&ClipContent::Text(text.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct FakeClip {
        content: Option<ClipContent>,
        log: Vec<String>,
    }

    impl Clipboard for FakeClip {
        fn get(&mut self) -> Result<Option<ClipContent>, InjectError> {
            Ok(self.content.clone())
        }
        fn set(&mut self, c: &ClipContent) -> Result<(), InjectError> {
            self.log.push(format!("set {c:?}"));
            self.content = Some(c.clone());
            Ok(())
        }
        fn clear(&mut self) -> Result<(), InjectError> {
            self.log.push("clear".into());
            self.content = None;
            Ok(())
        }
    }

    struct FakeKeys {
        fail: bool,
        sent: Vec<PasteCombo>,
    }

    impl Keystrokes for FakeKeys {
        fn paste(&mut self, combo: PasteCombo) -> Result<(), InjectError> {
            self.sent.push(combo);
            if self.fail {
                Err(InjectError::Keystroke("denied".into()))
            } else {
                Ok(())
            }
        }
    }

    fn keys(fail: bool) -> FakeKeys {
        FakeKeys { fail, sent: vec![] }
    }

    fn text(s: &str) -> ClipContent {
        ClipContent::Text(s.into())
    }

    #[test]
    fn restores_previous_clipboard() {
        let mut clip = FakeClip {
            content: Some(text("user's")),
            ..Default::default()
        };
        let mut k = keys(false);
        let mut slept = Duration::ZERO;
        let r = paste(
            "dictated",
            &mut clip,
            &mut k,
            PasteOptions::new(PasteCombo::CmdV),
            &mut |d| slept = d,
        )
        .unwrap();
        assert_eq!(r, Restore::Restored);
        assert_eq!(clip.content, Some(text("user's")));
        assert_eq!(k.sent, vec![PasteCombo::CmdV]);
        assert_eq!(slept, Duration::from_millis(250));
    }

    #[test]
    fn restores_images_and_empty_clipboards() {
        let img = ClipContent::Image {
            width: 1,
            height: 1,
            rgba: vec![1, 2, 3, 4],
        };
        let mut clip = FakeClip {
            content: Some(img.clone()),
            ..Default::default()
        };
        paste(
            "x",
            &mut clip,
            &mut keys(false),
            PasteOptions::new(PasteCombo::CtrlV),
            &mut |_| {},
        )
        .unwrap();
        assert_eq!(clip.content, Some(img));

        let mut clip = FakeClip::default();
        paste(
            "x",
            &mut clip,
            &mut keys(false),
            PasteOptions::new(PasteCombo::CtrlV),
            &mut |_| {},
        )
        .unwrap();
        assert_eq!(clip.content, None);
        assert_eq!(clip.log.last().map(String::as_str), Some("clear"));
    }

    /// A clipboard handle that a sleep callback can also modify, like a user copying.
    #[derive(Clone, Default)]
    struct SharedClip(std::rc::Rc<std::cell::RefCell<Option<ClipContent>>>);

    impl Clipboard for SharedClip {
        fn get(&mut self) -> Result<Option<ClipContent>, InjectError> {
            Ok(self.0.borrow().clone())
        }
        fn set(&mut self, c: &ClipContent) -> Result<(), InjectError> {
            *self.0.borrow_mut() = Some(c.clone());
            Ok(())
        }
        fn clear(&mut self) -> Result<(), InjectError> {
            *self.0.borrow_mut() = None;
            Ok(())
        }
    }

    #[test]
    fn leaves_a_newer_user_copy_alone() {
        let mut clip = SharedClip::default();
        clip.set(&text("old")).unwrap();
        let user = clip.clone();
        let r = paste(
            "dictated",
            &mut clip,
            &mut keys(false),
            PasteOptions::new(PasteCombo::CtrlV),
            &mut |_| {
                *user.0.borrow_mut() = Some(text("new copy")); // the user copies while we wait
            },
        )
        .unwrap();
        assert_eq!(r, Restore::UserCopiedSince);
        assert_eq!(clip.get().unwrap(), Some(text("new copy")));
    }

    #[test]
    fn keep_on_clipboard_option() {
        let mut clip = FakeClip {
            content: Some(text("old")),
            ..Default::default()
        };
        let opts = PasteOptions {
            keep_on_clipboard: true,
            ..PasteOptions::new(PasteCombo::CtrlV)
        };
        let mut slept = false;
        assert_eq!(
            paste("dictated", &mut clip, &mut keys(false), opts, &mut |_| slept = true).unwrap(),
            Restore::KeptByOption
        );
        assert_eq!(clip.content, Some(text("dictated")));
        assert!(!slept, "no need to wait");
    }

    #[test]
    fn keystroke_failure_restores_and_reports() {
        let mut clip = FakeClip {
            content: Some(text("old")),
            ..Default::default()
        };
        let err = paste(
            "dictated",
            &mut clip,
            &mut keys(true),
            PasteOptions::new(PasteCombo::CtrlV),
            &mut |_| {},
        )
        .unwrap_err();
        assert!(matches!(err, InjectError::Keystroke(_)));
        assert_eq!(clip.content, Some(text("old")));
    }

    #[test]
    fn combos_per_platform() {
        assert_eq!(combo_for(Os::MacOs, FieldKind::Terminal), PasteCombo::CmdV);
        assert_eq!(combo_for(Os::Windows, FieldKind::Terminal), PasteCombo::CtrlV);
        assert_eq!(combo_for(Os::Linux, FieldKind::Terminal), PasteCombo::CtrlShiftV);
        assert_eq!(combo_for(Os::Linux, FieldKind::Chat), PasteCombo::CtrlV);
    }
}
