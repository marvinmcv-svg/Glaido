//! Screen-lock state from systemd-logind (`LockedHint` on our session), which
//! GNOME, KDE and most lockers keep up to date, on X11 and Wayland alike.

use std::sync::Mutex;
use std::time::Duration;

use dbus::blocking::stdintf::org_freedesktop_dbus::Properties;
use dbus::blocking::Connection;

const DEST: &str = "org.freedesktop.login1";
const TIMEOUT: Duration = Duration::from_millis(300);

pub struct Logind {
    inner: Mutex<Option<(Connection, dbus::Path<'static>)>>,
}

impl Logind {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(None),
        }
    }

    fn connect() -> Option<(Connection, dbus::Path<'static>)> {
        let conn = Connection::new_system().ok()?;
        let path: dbus::Path<'static> = {
            let mgr = conn.with_proxy(DEST, "/org/freedesktop/login1", TIMEOUT);
            let (p,): (dbus::Path<'static>,) = mgr
                .method_call(
                    "org.freedesktop.login1.Manager",
                    "GetSessionByPID",
                    (std::process::id(),),
                )
                .ok()?;
            p
        };
        Some((conn, path))
    }

    pub fn locked(&self) -> Option<bool> {
        let mut guard = self.inner.lock().ok()?;
        if guard.is_none() {
            *guard = Self::connect();
        }
        let (conn, path) = guard.as_ref()?;
        match conn
            .with_proxy(DEST, path.clone(), TIMEOUT)
            .get::<bool>("org.freedesktop.login1.Session", "LockedHint")
        {
            Ok(v) => Some(v),
            Err(_) => {
                *guard = None; // reconnect next time
                None
            }
        }
    }
}
