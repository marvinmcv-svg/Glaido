//! OS backends: X11 on Linux, AppKit on macOS, Win32 on Windows.

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
                Box::new(crate::NoDesktop)
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
