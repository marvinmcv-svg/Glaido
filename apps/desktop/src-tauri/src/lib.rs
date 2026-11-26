//! Vocalis desktop shell (P0): main window, dictation bar window, tray and deep links.
//! Hotkeys, audio and paste arrive in P1 (`crates/input`, `audio`, `inject`).

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

/// Show or hide the dictation bar, anchored bottom-centre on the current monitor.
/// It is shown without taking focus so the user's app keeps the cursor.
#[tauri::command]
fn toggle_bar<R: Runtime>(app: AppHandle<R>) -> Result<bool, String> {
    let bar = app.get_webview_window("bar").ok_or("bar window missing")?;
    if bar.is_visible().map_err(|e| e.to_string())? {
        bar.hide().map_err(|e| e.to_string())?;
        return Ok(false);
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
    bar.show().map_err(|e| e.to_string())?;
    // Must follow `show`: making an unrealized GTK window click-through panics in tao.
    let _ = bar.set_ignore_cursor_events(true);
    Ok(true)
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
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Vocalis");
}
