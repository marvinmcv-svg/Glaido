//! App context for dictation (master prompt §5.2 `crates/context`):
//!
//! * which app and window were in front when a dictation started,
//! * what kind of field that probably is ([`field_kind`]), which picks the
//!   paste keystroke (terminals on Linux need Ctrl+Shift+V) and, from P2, the
//!   formatting style,
//! * where the result should go when it is ready ([`plan_paste`]): back into
//!   the starting app if the user switched away (spec 4.1), or nowhere if no
//!   text field has focus (macOS).
//!
//! Browser URLs and selected text (AX / UIA / AT-SPI) come in P3.

mod kind;
#[cfg(feature = "os")]
pub mod os;

use serde::Serialize;
use vocalis_core::formatting::FieldKind;

pub use kind::{field_kind, field_kind_for_url};

/// The app and window in front at a moment in time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Frontmost {
    /// Human-readable app name ("Slack", "Code").
    pub name: String,
    /// Stable identifier: macOS bundle id, Windows exe name, X11 WM_CLASS class.
    pub app_id: String,
    pub pid: Option<u32>,
    /// Native window handle (X11 window id, HWND). Never sent off the device.
    #[serde(skip)]
    pub window: Option<u64>,
}

impl Frontmost {
    pub fn field_kind(&self) -> FieldKind {
        field_kind(&self.app_id, &self.name)
    }

    /// Same window if both have handles, else the same process and app.
    pub fn same_target(&self, other: &Frontmost) -> bool {
        match (self.window, other.window) {
            (Some(a), Some(b)) => a == b,
            _ => self.pid == other.pid && self.app_id == other.app_id,
        }
    }
}

/// Whether the focused element accepts text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusedField {
    Text,
    NotText,
    /// The platform can't tell (no API, or no permission). Always paste.
    Unknown,
}

/// Platform access to the desktop. Implementations must be cheap to call.
pub trait Desktop: Send + Sync {
    fn frontmost(&self) -> Option<Frontmost>;
    /// Bring `target` back to the front. `true` if the platform accepted it.
    fn activate(&self, target: &Frontmost) -> bool;
    fn focused_field(&self) -> FocusedField {
        FocusedField::Unknown
    }
}

/// A desktop that knows nothing (tests, unsupported sessions).
pub struct NoDesktop;

impl Desktop for NoDesktop {
    fn frontmost(&self) -> Option<Frontmost> {
        None
    }
    fn activate(&self, _: &Frontmost) -> bool {
        false
    }
}

/// Where a finished dictation goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PastePlan {
    /// Paste where the cursor is now.
    Here,
    /// The user switched apps while speaking: bring the starting app back first.
    Refocus(Frontmost),
}

/// Decide where to paste. `refocus` is the "Paste into the starting app"
/// setting. Unknown starts or ends paste where the cursor is.
pub fn plan_paste(start: Option<&Frontmost>, now: Option<&Frontmost>, refocus: bool) -> PastePlan {
    match (start, now) {
        (Some(s), Some(n)) if refocus && !s.same_target(n) => PastePlan::Refocus(s.clone()),
        _ => PastePlan::Here,
    }
}

/// The context the gateway sees: name, id and field kind only.
pub fn app_context(f: Option<&Frontmost>) -> vocalis_core::formatting::AppContext {
    match f {
        Some(f) => vocalis_core::formatting::AppContext {
            name: f.name.clone(),
            app_id: f.app_id.clone(),
            url: None,
            field_kind: f.field_kind(),
        },
        None => Default::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(id: &str, pid: u32, window: Option<u64>) -> Frontmost {
        Frontmost {
            name: id.into(),
            app_id: id.into(),
            pid: Some(pid),
            window,
        }
    }

    #[test]
    fn pastes_here_when_nothing_changed() {
        let a = app("slack", 1, Some(10));
        assert_eq!(plan_paste(Some(&a), Some(&a.clone()), true), PastePlan::Here);
    }

    #[test]
    fn refocuses_the_starting_app_after_a_switch() {
        let a = app("slack", 1, Some(10));
        let b = app("code", 2, Some(20));
        assert_eq!(plan_paste(Some(&a), Some(&b), true), PastePlan::Refocus(a.clone()));
        assert_eq!(plan_paste(Some(&a), Some(&b), false), PastePlan::Here, "setting off");
    }

    #[test]
    fn another_window_of_the_same_app_counts_as_a_switch() {
        let a = app("code", 1, Some(10));
        let b = app("code", 1, Some(11));
        assert!(matches!(plan_paste(Some(&a), Some(&b), true), PastePlan::Refocus(_)));
    }

    #[test]
    fn without_handles_compares_process_and_app() {
        assert!(app("x", 1, None).same_target(&app("x", 1, None)));
        assert!(!app("x", 1, None).same_target(&app("x", 2, None)));
    }

    #[test]
    fn unknown_ends_paste_here() {
        let a = app("slack", 1, Some(10));
        assert_eq!(plan_paste(None, Some(&a), true), PastePlan::Here);
        assert_eq!(plan_paste(Some(&a), None, true), PastePlan::Here);
        assert_eq!(plan_paste(None, None, true), PastePlan::Here);
    }

    #[test]
    fn gateway_context_has_no_handles() {
        let ctx = app_context(Some(&app("com.googlecode.iterm2", 3, Some(5))));
        assert_eq!(ctx.field_kind, FieldKind::Terminal);
        assert_eq!(ctx.app_id, "com.googlecode.iterm2");
        let json = serde_json::to_string(&app("a", 1, Some(99))).unwrap();
        assert!(!json.contains("99"), "window handles stay on the device: {json}");
    }
}
