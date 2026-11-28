//! Windows: the foreground window, its process image name, and bringing it
//! back to the front.

use windows_sys::Win32::Foundation::{CloseHandle, HWND};
use windows_sys::Win32::System::Threading::{
    AttachThreadInput, GetCurrentThreadId, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId, IsIconic, IsWindow, SetForegroundWindow,
    ShowWindow, SW_RESTORE,
};

use crate::{Desktop, Frontmost};

pub struct WinDesktop;

fn image_name(pid: u32) -> Option<String> {
    // SAFETY: plain Win32 calls; the handle is closed before returning.
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return None;
        }
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len);
        CloseHandle(h);
        (ok != 0).then(|| String::from_utf16_lossy(&buf[..len as usize]))
    }
}

impl Desktop for WinDesktop {
    fn frontmost(&self) -> Option<Frontmost> {
        // SAFETY: plain Win32 queries.
        let (hwnd, pid) = unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.is_null() {
                return None;
            }
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, &mut pid);
            (hwnd, pid)
        };
        let path = image_name(pid)?;
        let exe = path.rsplit(['\\', '/']).next().unwrap_or(&path).to_owned();
        let name = exe
            .rsplit_once('.')
            .map(|(stem, _)| stem.to_owned())
            .unwrap_or_else(|| exe.clone());
        Some(Frontmost {
            name,
            app_id: exe.to_lowercase(),
            pid: Some(pid),
            window: Some(hwnd as usize as u64),
        })
    }

    fn activate(&self, target: &Frontmost) -> bool {
        let Some(raw) = target.window else { return false };
        let hwnd = raw as usize as HWND;
        // SAFETY: the handle is validated with IsWindow before use. Windows only
        // lets the foreground thread move focus, so we briefly share its input
        // state (the documented AttachThreadInput approach).
        unsafe {
            if IsWindow(hwnd) == 0 {
                return false;
            }
            if IsIconic(hwnd) != 0 {
                ShowWindow(hwnd, SW_RESTORE);
            }
            let fg = GetForegroundWindow();
            let fg_thread = GetWindowThreadProcessId(fg, std::ptr::null_mut());
            let me = GetCurrentThreadId();
            let attached = fg_thread != 0 && fg_thread != me && AttachThreadInput(me, fg_thread, 1) != 0;
            BringWindowToTop(hwnd);
            let ok = SetForegroundWindow(hwnd) != 0;
            if attached {
                AttachThreadInput(me, fg_thread, 0);
            }
            if !ok {
                tracing::debug!(target: "context", "Windows declined to bring the starting window forward");
            }
            ok
        }
    }
}
