//! Vocalis desktop app: main window, dictation bar, tray, deep links, and the
//! dictation engine (hotkeys, microphone, gateway, paste, history).

mod adapters;
mod engine;
mod gateway_ws;
mod secrets;

use serde::Serialize;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, Runtime};

/// Logical size of the bar window; keep in sync with `tauri.conf.json`.
const BAR_SIZE: (f64, f64) = (380.0, 72.0);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppInfo {
    name: &'static str,
    version: &'static str,
    core_version: &'static str,
}

#[tauri::command]
fn app_info() -> AppInfo {
    AppInfo {
        name: vocalis_core::product::PRODUCT_NAME,
        version: env!("CARGO_PKG_VERSION"),
        core_version: env!("CARGO_PKG_VERSION"),
    }
}

/// Show the bar bottom-centre on the current monitor without taking focus, so
/// the user's app keeps the cursor. Must run on the main thread.
pub(crate) fn show_bar<R: Runtime>(app: &AppHandle<R>) {
    let Some(bar) = app.get_webview_window("bar") else {
        return;
    };
    if bar.is_visible().unwrap_or(false) {
        return;
    }
    // Position from the configured logical size: on Linux the window reports a
    // zero size until GTK realizes it, which happens asynchronously after `show`.
    if let Ok(Some(monitor)) = bar.current_monitor() {
        let scale = monitor.scale_factor();
        let (w, h) = ((BAR_SIZE.0 * scale) as i32, (BAR_SIZE.1 * scale) as i32);
        let area = monitor.size();
        let origin = monitor.position();
        let x = origin.x + (area.width as i32 - w) / 2;
        let y = origin.y + area.height as i32 - h - (area.height as i32 / 12);
        let _ = bar.set_position(PhysicalPosition::new(x, y));
    }
    let _ = bar.show();
    // Must follow `show`: making an unrealized GTK window click-through panics in tao.
    let _ = bar.set_ignore_cursor_events(true);
}

pub(crate) fn hide_bar<R: Runtime>(app: &AppHandle<R>) {
    if let Some(bar) = app.get_webview_window("bar") {
        let _ = bar.hide();
    }
}

/// Settings "preview the bar" button.
#[tauri::command]
fn toggle_bar<R: Runtime>(app: AppHandle<R>) -> Result<bool, String> {
    let bar = app.get_webview_window("bar").ok_or("bar window missing")?;
    if bar.is_visible().map_err(|e| e.to_string())? {
        hide_bar(&app);
        Ok(false)
    } else {
        show_bar(&app);
        Ok(true)
    }
}

/// Default hotkeys per OS (docs/MASTER_PROMPT.md Appendix A).
fn default_hotkeys() -> (&'static str, &'static str) {
    if cfg!(target_os = "macos") {
        ("fn", "fn+space")
    } else if cfg!(target_os = "windows") {
        ("ctrl+win", "ctrl+win+shift")
    } else {
        ("left_ctrl+space", "left_ctrl+shift+space")
    }
}

fn env(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|v| !v.trim().is_empty())
}

/// Dictation languages: `VOCALIS_LANGUAGES` (comma-separated), else the system
/// locale's language, else auto-detect.
fn languages() -> Vec<String> {
    if let Some(v) = env("VOCALIS_LANGUAGES") {
        return v
            .split(',')
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect();
    }
    env("LANG")
        .and_then(|l| l.split(['_', '.', '-']).next().map(str::to_lowercase))
        .filter(|l| l.len() == 2 && l != "c")
        .into_iter()
        .collect()
}

fn start_engine<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let (primary, hands_free) = default_hotkeys();
    // Until sign-in lands (P1 webapp deep link), a development token can be supplied.
    let dev_token = env("VOCALIS_DEV_TOKEN");
    let cfg = engine::EngineConfig {
        data_dir,
        gateway_url: env("VOCALIS_GATEWAY_URL").unwrap_or_else(|| "https://api.vocalis.app".into()),
        token: std::sync::Arc::new(move || dev_token.clone()),
        primary: vocalis_input::Chord::parse(primary).map_err(|e| e.to_string())?,
        hands_free: vocalis_input::Chord::parse(hands_free).map_err(|e| e.to_string())?,
        gesture: vocalis_core::gesture::Config::default(),
        languages: languages(),
        keep_on_clipboard: false,
        dev_wav: env("VOCALIS_DEV_WAV").map(std::path::PathBuf::from),
    };
    engine::start(app.clone(), cfg)
}

fn show_main<R: Runtime>(app: &AppHandle<R>) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

/// Deep links (`vocalis://…`) are forwarded to the main window, which handles
/// auth callbacks and record links (P1).
fn forward_deep_links<R: Runtime>(app: &AppHandle<R>, urls: Vec<String>) {
    let urls: Vec<String> = urls.into_iter().filter(|u| u.starts_with("vocalis://")).collect();
    if !urls.is_empty() {
        show_main(app);
        let _ = app.emit_to("main", "deep-link", urls);
    }
}

pub fn run() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .try_init();
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            forward_deep_links(app, argv);
            show_main(app);
        }))
        .plugin(tauri_plugin_deep_link::init())
        .invoke_handler(tauri::generate_handler![app_info, toggle_bar])
        .setup(|app| {
            use tauri_plugin_deep_link::DeepLinkExt;
            #[cfg(any(target_os = "linux", all(debug_assertions, windows)))]
            let _ = app.deep_link().register_all();
            let handle = app.handle().clone();
            app.deep_link().on_open_url(move |event| {
                forward_deep_links(&handle, event.urls().iter().map(|u| u.to_string()).collect());
            });

            let open = MenuItem::with_id(app, "open", "Open Vocalis", true, None::<&str>)?;
            let bar = MenuItem::with_id(app, "bar", "Preview dictation bar", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit Vocalis", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &bar, &quit])?;
            let mut tray = TrayIconBuilder::with_id("main")
                .tooltip("Vocalis")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => show_main(app),
                    "bar" => {
                        let _ = toggle_bar(app.clone());
                    }
                    "quit" => app.exit(0),
                    _ => {}
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;
            if let Err(e) = start_engine(app.handle()) {
                tracing::error!(target: "engine", error = %e, "dictation engine did not start");
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Vocalis");
}
