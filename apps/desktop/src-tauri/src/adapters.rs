//! Desktop implementations of the session ports: the UI (bar, sounds, notices,
//! paste) and History (the encrypted store).

use std::sync::{Arc, Mutex};

use serde_json::json;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use vocalis_context::{plan_paste, Desktop, FocusedField, Frontmost, PastePlan};
use vocalis_core::formatting::FieldKind;
use vocalis_inject::os::{OsClipboard, OsKeystrokes};
use vocalis_inject::{combo_for, Os, PasteOptions};
use vocalis_session::{BarState, History, NewEntry, Notice, PasteOutcome, Sound, Ui};
use vocalis_store::{NewRecord, RecordStatus, RecordType, Store};

use crate::engine::SharedSettings;

/// The desktop and the app that was in front when the current dictation began.
pub struct AppTracker {
    pub desktop: Box<dyn Desktop>,
    pub start: Mutex<Option<Frontmost>>,
}

impl AppTracker {
    pub fn new(desktop: Box<dyn Desktop>) -> Self {
        Self {
            desktop,
            start: Mutex::new(None),
        }
    }

    /// Remember the frontmost app at the start of a dictation.
    pub fn capture(&self) -> Option<Frontmost> {
        let f = self.desktop.frontmost();
        *self.start.lock().expect("context lock") = f.clone();
        f
    }
}

pub struct TauriUi<R: Runtime> {
    pub app: AppHandle<R>,
    pub keep_on_clipboard: bool,
    pub context: Arc<AppTracker>,
    pub settings: SharedSettings,
    /// One clipboard handle for the app's lifetime. On X11 the clipboard is
    /// owned by a process: dropping the handle after restoring the user's
    /// clipboard would make the restored contents vanish.
    pub clipboard: Mutex<Option<OsClipboard>>,
}

impl<R: Runtime> TauriUi<R> {
    pub fn new(app: AppHandle<R>, keep_on_clipboard: bool, context: Arc<AppTracker>, settings: SharedSettings) -> Self {
        Self {
            app,
            keep_on_clipboard,
            context,
            settings,
            clipboard: Mutex::new(None),
        }
    }
}

impl<R: Runtime> TauriUi<R> {
    /// Put text on the clipboard (History "Copy", notification "Copy"). Uses the
    /// shared handle for the same X11 ownership reason as `paste`.
    pub fn copy(&self, text: &str) -> Result<(), String> {
        let mut guard = self.clipboard.lock().expect("clipboard lock");
        if guard.is_none() {
            *guard = Some(OsClipboard::new().map_err(|e| e.to_string())?);
        }
        let clip = guard.as_mut().expect("clipboard initialized");
        vocalis_inject::copy(text, clip).map_err(|e| e.to_string())
    }
}

impl<R: Runtime> Ui for TauriUi<R> {
    fn bar(&self, state: BarState) {
        let _ = self.app.emit_to("bar", "bar-state", state);
        let app = self.app.clone();
        let _ = self.app.run_on_main_thread(move || {
            if state == BarState::Hidden {
                crate::hide_bar(&app);
            } else {
                crate::show_bar(&app);
            }
        });
    }

    fn level(&self, level: f32) {
        let _ = self.app.emit_to("bar", "bar-level", level);
    }

    fn sound(&self, sound: Sound) {
        let name = match sound {
            Sound::Start => "start",
            Sound::Stop => "stop",
            Sound::Error => "error",
        };
        let _ = self.app.emit_to("bar", "sound", name);
    }

    fn notify(&self, notice: Notice) {
        let payload = match &notice {
            Notice::TranscriptionFailed { record_id, code } => {
                json!({ "kind": "transcription_failed", "recordId": record_id, "code": code })
            }
            Notice::NowhereToPaste { record_id } => json!({ "kind": "nowhere_to_paste", "recordId": record_id }),
            Notice::PasteFailed { record_id } => json!({ "kind": "paste_failed", "recordId": record_id }),
            Notice::RecordingLimitNear => json!({ "kind": "recording_limit_near" }),
            Notice::MicSilent => json!({ "kind": "mic_silent" }),
            Notice::RetrySucceeded { record_id } => json!({ "kind": "retry_succeeded", "recordId": record_id }),
        };
        let _ = self.app.emit_to("main", "notice", payload);
    }

    fn paste(&self, text: &str) -> PasteOutcome {
        let start = self.context.start.lock().expect("context lock").clone();
        let refocus = self.settings.read().expect("settings lock").paste_into_starting_app;
        let desktop = &self.context.desktop;
        if let PastePlan::Refocus(target) = plan_paste(start.as_ref(), desktop.frontmost().as_ref(), refocus) {
            if desktop.activate(&target) {
                // Give the window manager a moment to move focus.
                tokio::task::block_in_place(|| std::thread::sleep(std::time::Duration::from_millis(150)));
            } else {
                tracing::info!(target: "paste", "could not return to the starting app; pasting here");
            }
        }
        if desktop.focused_field() == FocusedField::NotText {
            return PasteOutcome::NowhereToPaste;
        }
        let field = start.as_ref().map(Frontmost::field_kind).unwrap_or(FieldKind::Generic);
        let opts = PasteOptions {
            keep_on_clipboard: self.keep_on_clipboard,
            ..PasteOptions::new(combo_for(Os::current(), field))
        };
        let run = || -> Result<(), vocalis_inject::InjectError> {
            let mut guard = self.clipboard.lock().expect("clipboard lock");
            if guard.is_none() {
                *guard = Some(OsClipboard::new()?);
            }
            let clip = guard.as_mut().expect("clipboard initialized");
            let mut keys = OsKeystrokes::new()?;
            vocalis_inject::paste(text, clip, &mut keys, opts, &mut std::thread::sleep).map(|_| ())
        };
        // The paste waits ~250 ms for the target app; don't stall the async workers.
        match tokio::task::block_in_place(run) {
            Ok(()) => PasteOutcome::Pasted,
            Err(e) => {
                tracing::warn!(target: "paste", error = %e, "paste failed");
                PasteOutcome::Failed
            }
        }
    }

    fn command(&self, request: Option<&str>, record_id: &str) {
        let _ = self
            .app
            .emit_to("main", "command", json!({ "request": request, "recordId": record_id }));
        if let Some(w) = self.app.get_webview_window("main") {
            let _ = w.show();
        }
    }
}

pub struct StoreHistory {
    pub store: Mutex<Store>,
    /// Called after every change so the History list can refresh.
    pub on_change: Box<dyn Fn() + Send + Sync>,
}

impl StoreHistory {
    pub fn new(store: Store, on_change: impl Fn() + Send + Sync + 'static) -> Self {
        Self {
            store: Mutex::new(store),
            on_change: Box::new(on_change),
        }
    }

    pub fn lock(&self) -> std::sync::MutexGuard<'_, Store> {
        self.store.lock().expect("store lock")
    }

    pub fn changed(&self) {
        (self.on_change)();
    }
}

impl History for StoreHistory {
    fn insert(&self, e: NewEntry) {
        let failed = e.error_code.is_some();
        let record = NewRecord {
            kind: Some(if e.is_command {
                RecordType::Command
            } else {
                RecordType::Dictation
            }),
            status: Some(if failed { RecordStatus::Failed } else { RecordStatus::Ok }),
            text: e.text,
            app_name: e.app_name,
            app_id: e.app_id,
            url: e.url,
            field_kind: None,
            words: e.words,
            duration_ms: e.duration_ms,
            wpm: e.wpm,
            audio_path: e.audio_path,
            error_code: e.error_code,
            created_at: e.created_at,
        };
        let store = self.store.lock().expect("store lock");
        match store.insert_with_id(&e.id, record) {
            Ok(_) => {
                if let Ok(expired) = store.enforce_audio_retention() {
                    for p in expired {
                        let _ = std::fs::remove_file(p);
                    }
                }
            }
            Err(err) => tracing::error!(target: "history", error = %err, "could not save record"),
        }
        drop(store);
        self.changed();
    }

    fn complete(&self, id: &str, text: &str, words: u32, wpm: u32) {
        if let Err(err) = self.lock().complete(id, text, words, wpm) {
            tracing::error!(target: "history", error = %err, "could not update record");
        }
        self.changed();
    }

    fn fail(&self, id: &str, code: &str) {
        if let Err(err) = self.lock().fail(id, code) {
            tracing::error!(target: "history", error = %err, "could not update record");
        }
        self.changed();
    }
}
