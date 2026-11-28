//! macOS: NSWorkspace for the frontmost app, NSRunningApplication to bring it
//! back, and the Accessibility API for "is a text field focused?".

use std::ffi::c_void;

use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication, NSWorkspace};

use crate::{Desktop, FocusedField, Frontmost};

pub struct MacDesktop;

impl Desktop for MacDesktop {
    fn frontmost(&self) -> Option<Frontmost> {
        let app = NSWorkspace::sharedWorkspace().frontmostApplication()?;
        let name = app.localizedName().map(|s| s.to_string()).unwrap_or_default();
        let app_id = app
            .bundleIdentifier()
            .map(|s| s.to_string())
            .unwrap_or_else(|| name.clone());
        let pid = u32::try_from(app.processIdentifier()).ok();
        Some(Frontmost {
            name,
            app_id,
            pid,
            window: None,
        })
    }

    fn activate(&self, target: &Frontmost) -> bool {
        let Some(pid) = target.pid.and_then(|p| i32::try_from(p).ok()) else {
            return false;
        };
        let Some(app) = NSRunningApplication::runningApplicationWithProcessIdentifier(pid) else {
            return false;
        };
        #[allow(deprecated)] // `activate()` needs macOS 14; this works on 12+.
        let ok = app.activateWithOptions(NSApplicationActivationOptions::ActivateAllWindows);
        if !ok {
            tracing::debug!(target: "context", "macOS declined to activate the starting app");
        }
        ok
    }

    fn focused_field(&self) -> FocusedField {
        ax::focused_field()
    }

    fn screen_locked(&self) -> Option<bool> {
        ax::screen_locked()
    }
}

/// Raw Accessibility (ApplicationServices) and Core Foundation calls.
mod ax {
    use super::*;

    type CFTypeRef = *const c_void;
    type CFStringRef = *const c_void;
    type AXError = i32;
    const AX_SUCCESS: AXError = 0;
    const AX_NO_VALUE: AXError = -25212;
    const UTF8: u32 = 0x0800_0100;

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXIsProcessTrusted() -> bool;
        fn AXUIElementCreateSystemWide() -> CFTypeRef;
        fn AXUIElementCopyAttributeValue(element: CFTypeRef, attribute: CFStringRef, value: *mut CFTypeRef) -> AXError;
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGSessionCopyCurrentDictionary() -> CFTypeRef;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFDictionaryGetValue(dict: CFTypeRef, key: CFTypeRef) -> CFTypeRef;
        fn CFBooleanGetValue(b: CFTypeRef) -> u8;
        fn CFBooleanGetTypeID() -> usize;
        fn CFRelease(cf: CFTypeRef);
        fn CFStringCreateWithBytes(
            alloc: CFTypeRef,
            bytes: *const u8,
            len: isize,
            encoding: u32,
            external: u8,
        ) -> CFStringRef;
        fn CFStringGetCString(s: CFStringRef, buf: *mut u8, size: isize, encoding: u32) -> u8;
        fn CFGetTypeID(cf: CFTypeRef) -> usize;
        fn CFStringGetTypeID() -> usize;
    }

    /// An owned CF object, released on drop.
    struct Owned(CFTypeRef);
    impl Drop for Owned {
        fn drop(&mut self) {
            if !self.0.is_null() {
                // SAFETY: we own one retain count on a non-null CF object.
                unsafe { CFRelease(self.0) }
            }
        }
    }

    fn cfstr(s: &str) -> Owned {
        // SAFETY: valid UTF-8 bytes with their length; the result is owned.
        Owned(unsafe { CFStringCreateWithBytes(std::ptr::null(), s.as_ptr(), s.len() as isize, UTF8, 0) })
    }

    fn copy_attr(element: CFTypeRef, name: &str) -> Result<Owned, AXError> {
        let attr = cfstr(name);
        let mut out: CFTypeRef = std::ptr::null();
        // SAFETY: element and attr are live CF objects; out receives an owned ref.
        let err = unsafe { AXUIElementCopyAttributeValue(element, attr.0, &mut out) };
        if err == AX_SUCCESS && !out.is_null() {
            Ok(Owned(out))
        } else {
            Err(if err == AX_SUCCESS { AX_NO_VALUE } else { err })
        }
    }

    fn string(v: &Owned) -> Option<String> {
        // SAFETY: type check before treating the object as a CFString.
        if unsafe { CFGetTypeID(v.0) != CFStringGetTypeID() } {
            return None;
        }
        let mut buf = [0u8; 128];
        // SAFETY: buf is writable for its full length.
        let ok = unsafe { CFStringGetCString(v.0, buf.as_mut_ptr(), buf.len() as isize, UTF8) };
        (ok != 0).then(|| {
            let end = buf.iter().position(|b| *b == 0).unwrap_or(buf.len());
            String::from_utf8_lossy(&buf[..end]).into_owned()
        })
    }

    /// Roles that never take typed text. Anything else (including web areas
    /// and custom Electron views, which often report generic roles) counts
    /// as a place to paste, so a wrong guess can't swallow a dictation.
    const NOT_TEXT: &[&str] = &[
        "AXApplication",
        "AXWindow",
        "AXSheet",
        "AXDrawer",
        "AXButton",
        "AXRadioButton",
        "AXCheckBox",
        "AXPopUpButton",
        "AXMenuButton",
        "AXMenu",
        "AXMenuBar",
        "AXMenuItem",
        "AXImage",
        "AXList",
        "AXTable",
        "AXOutline",
        "AXBrowser",
        "AXRow",
        "AXCell",
        "AXSlider",
        "AXToolbar",
        "AXScrollBar",
        "AXDockItem",
    ];

    /// `CGSSessionScreenIsLocked` in the session dictionary; absent means unlocked.
    pub fn screen_locked() -> Option<bool> {
        // SAFETY: an owned dictionary (or null) we release on drop.
        let dict = Owned(unsafe { CGSessionCopyCurrentDictionary() });
        if dict.0.is_null() {
            return None;
        }
        let key = cfstr("CGSSessionScreenIsLocked");
        // SAFETY: borrowed value from a live dictionary; type-checked before use.
        unsafe {
            let v = CFDictionaryGetValue(dict.0, key.0);
            if v.is_null() {
                return Some(false);
            }
            if CFGetTypeID(v) != CFBooleanGetTypeID() {
                return None;
            }
            Some(CFBooleanGetValue(v) != 0)
        }
    }

    pub fn focused_field() -> FocusedField {
        // SAFETY: argument-free queries.
        if !unsafe { AXIsProcessTrusted() } {
            return FocusedField::Unknown;
        }
        // SAFETY: returns an owned system-wide element.
        let system = Owned(unsafe { AXUIElementCreateSystemWide() });
        match copy_attr(system.0, "AXFocusedUIElement") {
            Err(AX_NO_VALUE) => FocusedField::NotText,
            Err(_) => FocusedField::Unknown,
            Ok(el) => match copy_attr(el.0, "AXRole").ok().as_ref().and_then(string) {
                Some(role) if NOT_TEXT.contains(&role.as_str()) => FocusedField::NotText,
                Some(_) => FocusedField::Text,
                None => FocusedField::Unknown,
            },
        }
    }
}
