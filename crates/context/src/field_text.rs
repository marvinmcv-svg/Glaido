//! Reading back the text of the focused field, for dictionary auto-suggestions
//! (spec 4.5.1): some seconds after a paste, the desktop reads what the user
//! left in the field and compares it with what was pasted
//! (`vocalis_core::suggest::detect_fixes`).
//!
//! **Status per platform** (see `docs/troubleshooting.md`, "Dictionary
//! suggestions"):
//!
//! * **Linux (X11 and Wayland):** not available yet. X11 has no API for the
//!   contents of another app's text field; it needs the accessibility bus
//!   (AT-SPI), which arrives with selected-text capture in P3. Wayland gives
//!   no access by design outside AT-SPI.
//! * **macOS:** planned through the Accessibility API (`AXValue` of the
//!   focused element), which needs the Accessibility permission we already ask
//!   for. Not wired yet.
//! * **Windows:** planned through UI Automation (`ValuePattern`/`TextPattern`).
//!   Not wired yet.
//!
//! Until a reader exists, suggestions still work from edits the user makes in
//! History (the desktop passes the original and the edited text), on every OS.
//!
//! Readers must never return text from password fields, and callers must keep
//! the text in memory only long enough to compare it.

/// Why no text came back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unreadable {
    /// The platform has no reader yet (or never will, e.g. plain Wayland).
    Unsupported,
    /// The user hasn't granted the permission the reader needs.
    NoPermission,
    /// Nothing focused, or the field doesn't expose its text.
    NoField,
    /// A password or other secure field: never read.
    Secure,
}

/// Platform access to the focused field's text.
pub trait FieldTextReader: Send + Sync {
    /// The whole text of the focused field, or why it can't be read.
    fn read_focused_text(&self) -> Result<String, Unreadable>;
}

/// The reader for platforms without one yet: always `Unsupported`.
pub struct NoFieldText;

impl FieldTextReader for NoFieldText {
    fn read_focused_text(&self) -> Result<String, Unreadable> {
        Err(Unreadable::Unsupported)
    }
}

/// The best reader for this build. Today every platform gets [`NoFieldText`];
/// the macOS (AX), Windows (UIA) and Linux (AT-SPI) readers plug in here.
pub fn reader() -> Box<dyn FieldTextReader> {
    Box::new(NoFieldText)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_reports_unsupported() {
        assert_eq!(reader().read_focused_text(), Err(Unreadable::Unsupported));
    }
}
