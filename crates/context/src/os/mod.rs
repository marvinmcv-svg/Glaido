//! OS backends: X11 on Linux, AppKit on macOS, Win32 on Windows.

#[cfg(target_os = "linux")]
mod logind;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;
#[cfg(target_os = "linux")]
mod x11;

use crate::Desktop;

/// The desktop backend for this session. On Linux without an X server (pure
/// Wayland), context is unavailable and pastes go where the cursor is.
pub fn desktop() -> Box<dyn Desktop> {
    #[cfg(target_os = "linux")]
    {
        match x11::X11Desktop::connect() {
            Some(d) => Box::new(d),
            None => {
                tracing::info!(target: "context", "no X11 display; app context unavailable");
                Box::new(LockOnly(logind::Logind::new()))
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        Box::new(macos::MacDesktop)
    }
    #[cfg(windows)]
    {
        Box::new(windows::WinDesktop)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    {
        Box::new(crate::NoDesktop)
    }
}

/// Wayland without XWayland: no app context, but lock state still comes from logind.
#[cfg(target_os = "linux")]
struct LockOnly(logind::Logind);

#[cfg(target_os = "linux")]
impl Desktop for LockOnly {
    fn frontmost(&self) -> Option<crate::Frontmost> {
        None
    }
    fn activate(&self, _: &crate::Frontmost) -> bool {
        false
    }
    fn screen_locked(&self) -> Option<bool> {
        self.0.locked()
    }
}
