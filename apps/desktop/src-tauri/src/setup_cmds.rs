//! Commands behind the setup wizard and Settings (spec 4.8): settings,
//! the hotkey picker, the microphone test and permission checks.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime, State};

use crate::engine::{EngineHandle, SharedSettings};
use crate::settings::{platform, Hotkeys, Settings, SettingsFile};

/// Settings plus where they live (Tauri managed state).
pub struct AppSettings {
    file: SettingsFile,
    pub shared: SharedSettings,
    mic_test: Mutex<Option<Arc<AtomicBool>>>,
}

impl AppSettings {
    pub fn load(data_dir: &Path) -> Self {
        let file = SettingsFile::new(data_dir);
        let settings = file.load();
        Self {
            file,
            shared: Arc::new(RwLock::new(settings)),
            mic_test: Mutex::new(None),
        }
    }

    fn update(&self, f: impl FnOnce(&mut Settings) -> Result<(), String>) -> Result<SettingsView, String> {
        let mut s = self.shared.write().expect("settings lock");
        let mut next = s.clone();
        f(&mut next)?;
        self.file
            .save(&next)
            .map_err(|_| "Couldn't save your settings.".to_owned())?;
        *s = next;
        Ok(SettingsView::from(&*s))
    }
}

/// What the UI sees: this platform's hotkeys only.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    languages: Vec<String>,
    microphone: Option<String>,
    hotkeys: Hotkeys,
    setup_completed: bool,
    paste_into_starting_app: bool,
    platform: &'static str,
}

impl From<&Settings> for SettingsView {
    fn from(s: &Settings) -> Self {
        Self {
            languages: s.languages.clone(),
            microphone: s.microphone.clone(),
            hotkeys: s.hotkeys(),
            setup_completed: s.setup_completed,
            paste_into_starting_app: s.paste_into_starting_app,
            platform: platform(),
        }
    }
}

#[tauri::command]
pub fn settings_get(settings: State<'_, AppSettings>) -> SettingsView {
    SettingsView::from(&*settings.shared.read().expect("settings lock"))
}

#[tauri::command]
pub fn settings_set_languages(
    settings: State<'_, AppSettings>,
    languages: Vec<String>,
) -> Result<SettingsView, String> {
    settings.update(|s| s.set_languages(languages))
}

#[tauri::command]
pub fn settings_set_microphone(
    settings: State<'_, AppSettings>,
    microphone: Option<String>,
) -> Result<SettingsView, String> {
    settings.update(|s| {
        s.microphone = microphone.filter(|m| !m.trim().is_empty());
        Ok(())
    })
}

#[tauri::command]
pub fn settings_set_hotkeys<R: Runtime>(
    app: AppHandle<R>,
    settings: State<'_, AppSettings>,
    hotkeys: Hotkeys,
) -> Result<SettingsView, String> {
    let view = settings.update(|s| s.set_hotkeys(hotkeys))?;
    if let Some(engine) = app.try_state::<EngineHandle>() {
        engine.hotkeys_changed()?;
    }
    Ok(view)
}

#[tauri::command]
pub fn settings_set_setup_completed(settings: State<'_, AppSettings>, completed: bool) -> Result<SettingsView, String> {
    settings.update(|s| {
        s.setup_completed = completed;
        Ok(())
    })
}

#[tauri::command]
pub fn settings_set_paste_into_starting_app(
    settings: State<'_, AppSettings>,
    enabled: bool,
) -> Result<SettingsView, String> {
    settings.update(|s| {
        s.paste_into_starting_app = enabled;
        Ok(())
    })
}

/// Hotkey picker: wait for a combination (global, so Fn and modifier-only
/// shortcuts work). Returns the settings value, or `None` if cancelled with Esc.
#[tauri::command]
pub async fn hotkey_record<R: Runtime>(app: AppHandle<R>) -> Result<Option<String>, String> {
    let engine = app
        .try_state::<EngineHandle>()
        .ok_or("Shortcuts can't be changed because the dictation engine did not start.")?;
    let chord = tokio::time::timeout(Duration::from_secs(20), engine.record_chord())
        .await
        .map_err(|_| "No keys were pressed. Try again.".to_owned())??;
    Ok(chord.map(|c| c.to_setting()))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Mic {
    name: String,
    is_default: bool,
}

#[tauri::command]
pub fn mic_list() -> Vec<Mic> {
    vocalis_audio::capture::list_inputs()
        .into_iter()
        .map(|d| Mic {
            name: d.name,
            is_default: d.is_default,
        })
        .collect()
}

/// Start the live input meter: `mic-level` events (0–1, ~30 per second) until
/// `mic_test_stop`, or `mic-test-error` if the microphone can't be opened.
/// Nothing is recorded or sent anywhere.
#[tauri::command]
pub fn mic_test_start<R: Runtime>(
    app: AppHandle<R>,
    settings: State<'_, AppSettings>,
    device: Option<String>,
) -> Result<(), String> {
    let stop = Arc::new(AtomicBool::new(false));
    if let Some(old) = settings.mic_test.lock().expect("mic lock").replace(stop.clone()) {
        old.store(true, Ordering::SeqCst);
    }
    std::thread::Builder::new()
        .name("vocalis-mic-test".into())
        .spawn(move || {
            let cap = match vocalis_audio::capture::start(device.as_deref()) {
                Ok(c) => c,
                Err(e) => {
                    let _ = app.emit_to("main", "mic-test-error", e.to_string());
                    return;
                }
            };
            let mut meter = vocalis_audio::level::LevelMeter::default();
            let frame = (cap.sample_rate as usize / 50).max(1); // 20 ms
            let mut pending: Vec<f32> = Vec::new();
            let mut last_emit = Instant::now();
            while !stop.load(Ordering::SeqCst) {
                match cap.blocks.recv_timeout(Duration::from_millis(50)) {
                    Ok(block) => pending.extend_from_slice(&block),
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(_) => {
                        let _ = app.emit_to("main", "mic-test-error", "The microphone was disconnected.");
                        return;
                    }
                }
                for chunk in pending.chunks(frame) {
                    if chunk.len() == frame {
                        meter.push(chunk);
                    }
                }
                let keep = pending.len() % frame;
                pending.drain(..pending.len() - keep);
                if last_emit.elapsed() >= Duration::from_millis(33) {
                    last_emit = Instant::now();
                    let _ = app.emit_to("main", "mic-level", meter.value());
                }
            }
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn mic_test_stop(settings: State<'_, AppSettings>) {
    if let Some(flag) = settings.mic_test.lock().expect("mic lock").take() {
        flag.store(true, Ordering::SeqCst);
    }
}

/// `granted`, `missing`, `not_needed`, or `unknown` (only the OS prompt tells).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Permissions {
    platform: &'static str,
    accessibility: &'static str,
    microphone: &'static str,
    /// Linux Wayland sessions limit global shortcuts and synthetic paste.
    wayland: bool,
}

#[cfg(target_os = "macos")]
fn accessibility() -> &'static str {
    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXIsProcessTrusted() -> bool;
    }
    // SAFETY: a side-effect-free query with no arguments.
    if unsafe { AXIsProcessTrusted() } {
        "granted"
    } else {
        "missing"
    }
}

#[cfg(not(target_os = "macos"))]
fn accessibility() -> &'static str {
    "not_needed"
}

#[tauri::command]
pub fn permissions_status() -> Permissions {
    let microphone = if vocalis_audio::capture::list_inputs().is_empty() {
        "missing"
    } else {
        "unknown"
    };
    Permissions {
        platform: platform(),
        accessibility: accessibility(),
        microphone,
        wayland: cfg!(target_os = "linux") && std::env::var("WAYLAND_DISPLAY").is_ok_and(|v| !v.is_empty()),
    }
}

/// Open the OS privacy pane for `accessibility` or `microphone`.
#[tauri::command]
pub fn open_permission_settings<R: Runtime>(app: AppHandle<R>, kind: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let url = match (platform(), kind.as_str()) {
        ("macos", "accessibility") => "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
        ("macos", "microphone") => "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone",
        ("windows", "microphone") => "ms-settings:privacy-microphone",
        _ => return Err("Open your system's sound settings to check the microphone.".into()),
    };
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|_| "Couldn't open system settings.".to_owned())
}
