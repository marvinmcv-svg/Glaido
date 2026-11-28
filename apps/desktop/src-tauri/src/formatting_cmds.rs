//! Formatting page commands (spec 4.5.3): rules, the preset gallery and the
//! app picker's "Most used" and "All applications" lists.
//!
//! The rules live in the history database; the engine reads them at every
//! dictation start and retry, so a change applies to the next dictation.
//!
//! Installed apps: Linux reads `.desktop` files from the XDG data dirs (the
//! app id is `StartupWMClass`, else the file name, which is what X11 WM_CLASS
//! usually reports); macOS reads `CFBundleIdentifier` from the XML
//! `Info.plist` of each bundle in `/Applications` and `~/Applications`
//! (bundles with a binary plist are skipped). Windows has no list yet, so
//! "Most used" is not filtered there and apps can be added by exe name.

use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, Manager, Runtime, State};
use vocalis_core::formatting::Style;
use vocalis_core::presets::{self, Os, PresetApp};
use vocalis_store::{AppRef, FormattingRule, PresetOutcome, RuleDraft, UsedApp};

use crate::engine::EngineHandle;

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn engine<R: Runtime>(app: &AppHandle<R>) -> Result<State<'_, EngineHandle>, String> {
    app.try_state::<EngineHandle>()
        .ok_or_else(|| "Formatting rules are unavailable because the dictation engine did not start.".to_owned())
}

#[tauri::command]
pub fn rules_get<R: Runtime>(app: AppHandle<R>) -> Result<Vec<FormattingRule>, String> {
    engine(&app)?
        .history
        .lock()
        .formatting_rules(now_ms())
        .map_err(|e| e.to_string())
}

/// Add a custom rule (`id` absent) or update an existing one.
#[tauri::command]
pub fn rule_save<R: Runtime>(app: AppHandle<R>, id: Option<String>, rule: RuleDraft) -> Result<FormattingRule, String> {
    let engine = engine(&app)?;
    let store = engine.history.lock();
    match id {
        Some(id) => store.rule_update(&id, rule, now_ms()),
        None => store.rule_add(rule, now_ms()),
    }
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn rule_delete<R: Runtime>(app: AppHandle<R>, id: String) -> Result<(), String> {
    engine(&app)?
        .history
        .lock()
        .rule_delete(&id, now_ms())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn rule_toggle<R: Runtime>(app: AppHandle<R>, id: String, enabled: bool) -> Result<FormattingRule, String> {
    engine(&app)?
        .history
        .lock()
        .rule_set_enabled(&id, enabled, now_ms())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn rule_add_preset<R: Runtime>(app: AppHandle<R>, preset_id: String) -> Result<PresetOutcome, String> {
    engine(&app)?
        .history
        .lock()
        .rule_add_preset(&preset_id, Os::current(), now_ms())
        .map_err(|e| e.to_string())
}

/// A gallery card: the preset with only this OS's apps.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetView {
    id: &'static str,
    name: &'static str,
    summary: &'static str,
    style: Style,
    raw: bool,
    prompt: &'static str,
    apps: Vec<PresetApp>,
    sites: &'static [&'static str],
    builtin_email: bool,
}

#[tauri::command]
pub fn formatting_presets() -> Vec<PresetView> {
    let os = Os::current();
    presets::PRESETS
        .iter()
        .map(|p| PresetView {
            id: p.id,
            name: p.name,
            summary: p.summary,
            style: p.style,
            raw: p.raw,
            prompt: p.prompt,
            apps: p
                .apps
                .iter()
                .filter(|a| os.is_none_or(|os| a.os == os))
                .copied()
                .collect(),
            sites: p.sites,
            builtin_email: p.builtin_email,
        })
        .collect()
}

/// "Most used": apps from the last 90 days of dictations that are still installed.
#[tauri::command]
pub fn apps_most_used<R: Runtime>(app: AppHandle<R>) -> Result<Vec<UsedApp>, String> {
    let used = engine(&app)?
        .history
        .lock()
        .most_used_apps(now_ms())
        .map_err(|e| e.to_string())?;
    Ok(still_installed(used, &installed_apps()))
}

/// "All applications", sorted by name. Empty where the platform has no list.
#[tauri::command]
pub async fn apps_installed() -> Vec<AppRef> {
    tauri::async_runtime::spawn_blocking(installed_apps)
        .await
        .unwrap_or_default()
}

fn tokens(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// Whether a recorded app matches an installed one: the same id or name, or
/// every word of the recorded id appears in the installed id (X11 reports
/// `Gnome-terminal` for `org.gnome.Terminal.desktop`).
fn matches_installed(used: &UsedApp, inst: &AppRef) -> bool {
    if used.id.eq_ignore_ascii_case(&inst.id) || used.name.eq_ignore_ascii_case(&inst.name) {
        return true;
    }
    let want = tokens(&used.id);
    let have = tokens(&inst.id);
    !want.is_empty() && want.iter().all(|t| have.contains(t))
}

/// Drop uninstalled apps. With no installed list (unsupported platform), keep all.
fn still_installed(used: Vec<UsedApp>, installed: &[AppRef]) -> Vec<UsedApp> {
    if installed.is_empty() {
        return used;
    }
    used.into_iter()
        .filter(|u| installed.iter().any(|i| matches_installed(u, i)))
        .collect()
}

/// Parse one `.desktop` file. `stem` is the file name without `.desktop`.
fn parse_desktop_entry(stem: &str, content: &str) -> Option<AppRef> {
    let mut in_entry = false;
    let (mut name, mut wm_class, mut kind) = (None, None, None);
    for line in content.lines().map(str::trim) {
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        match (k.trim(), v.trim()) {
            ("Name", v) => name = Some(v.to_owned()),
            ("StartupWMClass", v) if !v.is_empty() => wm_class = Some(v.to_owned()),
            ("Type", v) => kind = Some(v.to_owned()),
            ("NoDisplay" | "Hidden", "true") => return None,
            _ => {}
        }
    }
    if kind.as_deref() != Some("Application") {
        return None;
    }
    let name = name.filter(|n| !n.is_empty())?;
    let id = wm_class.unwrap_or_else(|| stem.to_owned());
    Some(AppRef { id, name })
}

/// Read `CFBundleIdentifier` and the display name from an XML `Info.plist`.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn parse_info_plist(bundle_name: &str, content: &str) -> Option<AppRef> {
    let value = |key: &str| {
        let after = content.split_once(&format!("<key>{key}</key>"))?.1;
        let s = after.trim_start().strip_prefix("<string>")?;
        let v = s.split_once("</string>")?.0.trim();
        (!v.is_empty()).then(|| v.to_owned())
    };
    let id = value("CFBundleIdentifier")?;
    let name = value("CFBundleDisplayName")
        .or_else(|| value("CFBundleName"))
        .unwrap_or_else(|| bundle_name.to_owned());
    Some(AppRef { id, name })
}

fn finish(mut apps: Vec<AppRef>) -> Vec<AppRef> {
    apps.retain(|a| !vocalis_core::apps::is_excluded(&a.id, &a.name));
    apps.sort_by_key(|a| a.name.to_lowercase());
    let mut out: Vec<AppRef> = Vec::with_capacity(apps.len());
    for a in apps {
        if !out.iter().any(|o| o.id.eq_ignore_ascii_case(&a.id)) {
            out.push(a);
        }
    }
    out
}

#[cfg(target_os = "linux")]
fn installed_apps() -> Vec<AppRef> {
    let home = std::env::var("HOME").unwrap_or_default();
    let data_home = std::env::var("XDG_DATA_HOME")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| format!("{home}/.local/share"));
    let data_dirs = std::env::var("XDG_DATA_DIRS")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    let mut dirs: Vec<String> = std::iter::once(data_home.clone())
        .chain(data_dirs.split(':').map(str::to_owned))
        .collect();
    dirs.push(format!("{data_home}/flatpak/exports/share"));
    dirs.push("/var/lib/flatpak/exports/share".into());
    dirs.push("/var/lib/snapd/desktop".into());
    let mut apps = Vec::new();
    for d in dirs {
        let dir = std::path::Path::new(&d).join("applications");
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            let path = e.path();
            if path.extension().and_then(|x| x.to_str()) != Some("desktop") {
                continue;
            }
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_owned();
            if let Ok(content) = std::fs::read_to_string(&path) {
                apps.extend(parse_desktop_entry(&stem, &content));
            }
        }
    }
    finish(apps)
}

#[cfg(target_os = "macos")]
fn installed_apps() -> Vec<AppRef> {
    let home = std::env::var("HOME").unwrap_or_default();
    let mut apps = Vec::new();
    for d in [
        "/Applications".to_owned(),
        "/System/Applications".to_owned(),
        format!("{home}/Applications"),
    ] {
        let Ok(entries) = std::fs::read_dir(std::path::Path::new(&d)) else {
            continue;
        };
        for e in entries.flatten() {
            let path = e.path();
            if path.extension().and_then(|x| x.to_str()) != Some("app") {
                continue;
            }
            let bundle = path.file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_owned();
            if let Ok(content) = std::fs::read_to_string(path.join("Contents/Info.plist")) {
                apps.extend(parse_info_plist(&bundle, &content));
            }
        }
    }
    finish(apps)
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn installed_apps() -> Vec<AppRef> {
    finish(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_entries() {
        let code = "[Desktop Entry]\nName=Visual Studio Code\nType=Application\nStartupWMClass=Code\n[Desktop Action new]\nName=New Window\n";
        assert_eq!(
            parse_desktop_entry("code", code),
            Some(AppRef {
                id: "Code".into(),
                name: "Visual Studio Code".into()
            })
        );
        let term = "[Desktop Entry]\nType=Application\nName=Terminal\n";
        assert_eq!(
            parse_desktop_entry("org.gnome.Terminal", term).unwrap().id,
            "org.gnome.Terminal"
        );
        assert_eq!(
            parse_desktop_entry("x", "[Desktop Entry]\nName=X\nType=Application\nNoDisplay=true\n"),
            None
        );
        assert_eq!(parse_desktop_entry("x", "[Desktop Entry]\nName=X\nType=Link\n"), None);
        assert_eq!(parse_desktop_entry("x", "[Desktop Entry]\nType=Application\n"), None);
    }

    #[test]
    fn info_plists() {
        let plist = "<plist><dict>\n<key>CFBundleIdentifier</key>\n\t<string>com.tinyspeck.slackmacgap</string>\n<key>CFBundleName</key><string>Slack</string></dict></plist>";
        assert_eq!(
            parse_info_plist("Slack", plist),
            Some(AppRef {
                id: "com.tinyspeck.slackmacgap".into(),
                name: "Slack".into()
            })
        );
        assert_eq!(parse_info_plist("X", "bplist00"), None);
    }

    #[test]
    fn installed_list_drops_browsers_and_duplicates() {
        let a = |id: &str, name: &str| AppRef {
            id: id.into(),
            name: name.into(),
        };
        let out = finish(vec![
            a("Slack", "Slack"),
            a("firefox", "Firefox"),
            a("Code", "Visual Studio Code"),
            a("slack", "Slack (Flatpak)"),
            a("app.vocalis.desktop", "Vocalis"),
        ]);
        assert_eq!(out, vec![a("Slack", "Slack"), a("Code", "Visual Studio Code")]);
    }

    #[test]
    fn most_used_keeps_installed_apps_only() {
        let used = |id: &str, name: &str| UsedApp {
            id: id.into(),
            name: name.into(),
            count: 1,
        };
        let inst = |id: &str, name: &str| AppRef {
            id: id.into(),
            name: name.into(),
        };
        let installed = [inst("org.gnome.Terminal", "Terminal"), inst("Slack", "Slack")];
        let out = still_installed(
            vec![
                used("Gnome-terminal", "Gnome-terminal"),
                used("slack", "Slack"),
                used("Gimp", "Gimp"),
            ],
            &installed,
        );
        assert_eq!(
            out.iter().map(|u| u.id.as_str()).collect::<Vec<_>>(),
            ["Gnome-terminal", "slack"]
        );
        assert_eq!(still_installed(vec![used("Gimp", "Gimp")], &[]).len(), 1);
    }
}
