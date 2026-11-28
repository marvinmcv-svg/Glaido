//! X11 (and XWayland apps): EWMH `_NET_ACTIVE_WINDOW` when a window manager
//! provides it, else the input focus walked up to its top-level window.

use std::sync::Mutex;

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ClientMessageEvent, ConfigureWindowAux, ConnectionExt, EventMask, InputFocus, StackMode, Window,
};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;
use x11rb::CURRENT_TIME;

use crate::{Desktop, Frontmost};

struct Atoms {
    net_active_window: Atom,
    net_wm_pid: Atom,
}

struct Conn {
    conn: RustConnection,
    root: Window,
    atoms: Atoms,
}

pub struct X11Desktop {
    inner: Mutex<Option<Conn>>,
}

fn open() -> Option<Conn> {
    let (conn, screen) = x11rb::connect(None).ok()?;
    let root = conn.setup().roots.get(screen)?.root;
    let atom = |name: &[u8]| -> Option<Atom> { Some(conn.intern_atom(false, name).ok()?.reply().ok()?.atom) };
    let atoms = Atoms {
        net_active_window: atom(b"_NET_ACTIVE_WINDOW")?,
        net_wm_pid: atom(b"_NET_WM_PID")?,
    };
    Some(Conn { conn, root, atoms })
}

impl X11Desktop {
    pub fn connect() -> Option<Self> {
        Some(Self {
            inner: Mutex::new(Some(open()?)),
        })
    }

    /// Run `f` on the connection, reconnecting once if the server went away.
    fn with<T>(&self, f: impl Fn(&Conn) -> Option<T>) -> Option<T> {
        let mut guard = self.inner.lock().ok()?;
        if guard.is_none() {
            *guard = open();
        }
        let r = f(guard.as_ref()?);
        if r.is_none() && guard.as_ref().is_some_and(|c| c.conn.get_input_focus().is_err()) {
            *guard = open();
            return f(guard.as_ref()?);
        }
        r
    }
}

impl Conn {
    fn prop(&self, w: Window, prop: Atom, ty: Atom) -> Option<Vec<u8>> {
        let r = self.conn.get_property(false, w, prop, ty, 0, 1024).ok()?.reply().ok()?;
        (r.format != 0 && !r.value.is_empty()).then_some(r.value)
    }

    fn active_window(&self) -> Option<Window> {
        let ewmh = self
            .prop(self.root, self.atoms.net_active_window, AtomEnum::WINDOW.into())
            .and_then(|v| v.get(..4).map(|b| u32::from_ne_bytes(b.try_into().unwrap())))
            .filter(|w| *w != 0);
        if ewmh.is_some() {
            return ewmh;
        }
        let focus = self.conn.get_input_focus().ok()?.reply().ok()?.focus;
        (focus > 1).then(|| self.client_of(focus)).flatten()
    }

    /// Walk up from `w` to the nearest window that carries WM_CLASS.
    fn client_of(&self, mut w: Window) -> Option<Window> {
        for _ in 0..16 {
            if self
                .prop(w, AtomEnum::WM_CLASS.into(), AtomEnum::STRING.into())
                .is_some()
            {
                return Some(w);
            }
            let tree = self.conn.query_tree(w).ok()?.reply().ok()?;
            if tree.parent == 0 || tree.parent == self.root {
                return Some(w);
            }
            w = tree.parent;
        }
        None
    }

    fn describe(&self, w: Window) -> Option<Frontmost> {
        // WM_CLASS is "instance\0class\0"; the class names the app.
        let class = self.prop(w, AtomEnum::WM_CLASS.into(), AtomEnum::STRING.into())?;
        let mut parts = class.split(|b| *b == 0).filter(|p| !p.is_empty());
        let instance = parts.next().map(|p| String::from_utf8_lossy(p).into_owned());
        let class = parts.next().map(|p| String::from_utf8_lossy(p).into_owned());
        let app_id = class.clone().or(instance)?;
        let pid = self
            .prop(w, self.atoms.net_wm_pid, AtomEnum::CARDINAL.into())
            .and_then(|v| v.get(..4).map(|b| u32::from_ne_bytes(b.try_into().unwrap())));
        Some(Frontmost {
            name: class.unwrap_or_else(|| app_id.clone()),
            app_id,
            pid,
            window: Some(u64::from(w)),
        })
    }
}

impl Desktop for X11Desktop {
    fn frontmost(&self) -> Option<Frontmost> {
        self.with(|c| {
            let w = c.active_window()?;
            c.describe(w)
        })
    }

    fn activate(&self, target: &Frontmost) -> bool {
        let Some(w) = target.window.and_then(|w| u32::try_from(w).ok()) else {
            return false;
        };
        self.with(|c| {
            // Ask the window manager (source 2 = pager/user action, which WMs honour)…
            let ev = ClientMessageEvent::new(32, w, c.atoms.net_active_window, [2u32, CURRENT_TIME, 0, 0, 0]);
            let mask = EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY;
            c.conn.send_event(false, c.root, mask, ev).ok()?;
            // …and set focus directly for sessions without one.
            c.conn
                .configure_window(w, &ConfigureWindowAux::new().stack_mode(StackMode::ABOVE))
                .ok()?;
            c.conn.set_input_focus(InputFocus::PARENT, w, CURRENT_TIME).ok()?;
            c.conn.sync().ok()?;
            Some(())
        })
        .is_some()
    }
}
