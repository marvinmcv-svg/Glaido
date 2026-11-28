//! Vocalis desktop app: main window, dictation bar, tray, deep links, and the
//! dictation engine (hotkeys, microphone, gateway, paste, history).

mod adapters;
mod auth;
mod engine;
mod history_cmds;
mod secrets;
mod settings;
mod setup_cmds;

use std::sync::Arc;

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

fn env(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|v| !v.trim().is_empty())
}

fn start_engine<R: Runtime>(
    app: &AppHandle<R>,
    auth: Arc<auth::Auth>,
    settings: engine::SharedSettings,
) -> Result<(), String> {
    let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let cfg = engine::EngineConfig {
        data_dir,
        gateway_url: env("VOCALIS_GATEWAY_URL").unwrap_or_else(|| "https://api.vocalis.app".into()),
        token: Arc::new(move || auth.access_token()),
        settings,
        gesture: vocalis_core::gesture::Config::default(),
        keep_on_clipboard: false,
        dev_wav: env("VOCALIS_DEV_WAV").map(std::path::PathBuf::from),
        dev_lock_file: env("VOCALIS_DEV_LOCK_FILE").map(std::path::PathBuf::from),
    };
    let handle = engine::start(app.clone(), cfg)?;
    app.manage(handle);
    Ok(())
}

fn show_main<R: Runtime>(app: &AppHandle<R>) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

/// Deep links (`vocalis://…`): sign-in callbacks are finished here; the rest
/// (record links, extension setup) go to the main window.
fn forward_deep_links<R: Runtime>(app: &AppHandle<R>, urls: Vec<String>) {
    let (sign_in, rest): (Vec<String>, Vec<String>) = urls
        .into_iter()
        .filter(|u| u.starts_with("vocalis://"))
        .partition(|u| auth::is_callback(u));
    for link in sign_in {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            let auth = app.state::<Arc<auth::Auth>>().inner().clone();
            let payload = match auth.complete(&link).await {
                Ok(account) => serde_json::json!({ "account": account }),
                Err(e) => serde_json::json!({ "account": auth.account(), "error": e.to_string() }),
            };
            show_main(&app);
            let _ = app.emit_to("main", "auth-changed", payload);
        });
    }
    if !rest.is_empty() {
        show_main(app);
        let _ = app.emit_to("main", "deep-link", rest);
    }
}

/// Why the dictation engine didn't start, for the main window to show.
struct EngineFailure(String);

#[tauri::command]
fn engine_error<R: Runtime>(app: AppHandle<R>) -> Option<String> {
    app.try_state::<EngineFailure>().map(|f| f.0.clone())
}

#[tauri::command]
fn auth_status(auth: tauri::State<'_, Arc<auth::Auth>>) -> Option<auth::Account> {
    auth.account()
}

/// Open the web app to sign in; the browser comes back through a deep link.
#[tauri::command]
fn auth_sign_in<R: Runtime>(app: AppHandle<R>, auth: tauri::State<'_, Arc<auth::Auth>>) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let url = auth.begin();
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|_| "Couldn't open your browser.".to_owned())
}

#[tauri::command]
async fn auth_sign_out<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    let auth = app.state::<Arc<auth::Auth>>().inner().clone();
    auth.sign_out().await;
    let _ = app.emit_to("main", "auth-changed", serde_json::json!({ "account": null }));
    Ok(())
}

/// Keep the access token fresh while signed in.
fn spawn_refresher<R: Runtime>(app: AppHandle<R>, auth: Arc<auth::Auth>) {
    tauri::async_runtime::spawn(async move {
        loop {
            match auth.refresh_if_due().await {
                Err(e @ auth::AuthError::SessionEnded) => {
                    let _ = app.emit_to(
                        "main",
                        "auth-changed",
                        serde_json::json!({ "account": null, "error": e.to_string() }),
                    );
                }
                Err(e) => tracing::info!(target: "auth", error = %e, "token refresh will retry"),
                Ok(()) => {}
            }
            tokio::time::sleep(std::time::Duration::from_secs(20)).await;
        }
    });
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
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            app_info,
            toggle_bar,
            engine_error,
            auth_status,
            auth_sign_in,
            auth_sign_out,
            history_cmds::history_list,
            history_cmds::history_search,
            history_cmds::history_retry,
            history_cmds::history_delete,
            history_cmds::history_audio,
            history_cmds::history_copy,
            setup_cmds::settings_get,
            setup_cmds::settings_set_languages,
            setup_cmds::settings_set_microphone,
            setup_cmds::settings_set_hotkeys,
            setup_cmds::settings_set_setup_completed,
            setup_cmds::settings_set_paste_into_starting_app,
            setup_cmds::hotkey_record,
            setup_cmds::mic_list,
            setup_cmds::mic_test_start,
            setup_cmds::mic_test_stop,
            setup_cmds::permissions_status,
            setup_cmds::open_permission_settings,
        ])
        .setup(|app| {
            use tauri_plugin_deep_link::DeepLinkExt;
            let key_dir = app.path().app_data_dir()?.join("keys");
            let web_url = env("VOCALIS_WEB_URL").unwrap_or_else(|| "https://app.vocalis.app".into());
            let auth = Arc::new(auth::Auth::new(&web_url, key_dir, env("VOCALIS_DEV_TOKEN")));
            app.manage(auth.clone());
            spawn_refresher(app.handle().clone(), auth.clone());
            let settings = setup_cmds::AppSettings::load(&app.path().app_data_dir()?);
            if let Some(langs) = env("VOCALIS_LANGUAGES") {
                // Development override; not saved.
                let langs = langs.split(',').map(str::to_owned).collect();
                let _ = settings.shared.write().expect("settings lock").set_languages(langs);
            }
            let shared = settings.shared.clone();
            app.manage(settings);
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
            if let Err(e) = start_engine(app.handle(), auth, shared) {
                tracing::error!(target: "engine", error = %e, "dictation engine did not start");
                app.manage(EngineFailure(e));
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Vocalis");
}
