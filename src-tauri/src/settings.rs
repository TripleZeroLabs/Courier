//! The Settings window: a local page (dist/settings.html) with exactly two
//! IPC commands, `get_settings` and `save_settings`.
//!
//! Security: these are the only commands Courier defines. build.rs declares
//! them in the app manifest so they're denied unless a capability grants
//! them, and capabilities/settings.json grants them to the window labelled
//! "settings" only, which loads Courier's bundled page. The Facebook window
//! has no capability at all. Each command also re-checks the calling window's
//! label and that it is showing Courier's own local page, and every value is
//! validated (Config::sanitize, theme.rs) before it is saved or applied.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tauri_plugin_autostart::ManagerExt as _;

use crate::config::{self, Config};
use crate::theme::{COLOR_TOKENS, MODES, SIZE_TOKENS};
use crate::{actions, tray};

pub const LABEL: &str = "settings";

#[derive(Serialize)]
struct ColorToken {
    key: &'static str,
    label: &'static str,
}

#[derive(Serialize)]
struct SizeToken {
    key: &'static str,
    label: &'static str,
    min: u32,
    max: u32,
    default: u32,
}

#[derive(Serialize)]
pub struct SettingsView {
    config: Config,
    autostart: bool,
    modes: &'static [&'static str],
    colors: Vec<ColorToken>,
    sizes: Vec<SizeToken>,
    version: String,
}

#[derive(Deserialize)]
pub struct SettingsIn {
    config: Config,
    autostart: bool,
}

fn view(app: &AppHandle) -> SettingsView {
    SettingsView {
        config: config::get(app),
        autostart: app.autolaunch().is_enabled().unwrap_or(false),
        modes: MODES,
        colors: COLOR_TOKENS.iter().map(|t| ColorToken { key: t.0, label: t.1 }).collect(),
        sizes: SIZE_TOKENS
            .iter()
            .map(|t| SizeToken { key: t.0, label: t.1, min: t.3, max: t.4, default: t.5 })
            .collect(),
        version: app.package_info().version.to_string(),
    }
}

/// Defence in depth on top of the capability: only Courier's own settings
/// page, in the settings window, may call these.
fn check_caller(window: &WebviewWindow) -> Result<(), String> {
    let local = window
        .url()
        .map(|u| {
            let bundled = u.scheme() == "tauri" || u.host_str() == Some("tauri.localhost");
            // `tauri dev` serves dist/ from its own server on 127.0.0.1.
            let dev_server = cfg!(debug_assertions) && u.scheme() == "http" && u.host_str() == Some("127.0.0.1");
            bundled || dev_server
        })
        .unwrap_or(false);
    if window.label() == LABEL && local {
        Ok(())
    } else {
        #[cfg(debug_assertions)]
        eprintln!(
            "[settings] refused caller: label={:?} url={:?}",
            window.label(),
            window.url().map(|u| format!("{}://{}", u.scheme(), u.host_str().unwrap_or("")))
        );
        Err("not allowed".into())
    }
}

#[tauri::command]
pub fn get_settings(window: WebviewWindow) -> Result<SettingsView, String> {
    check_caller(&window)?;
    Ok(view(window.app_handle()))
}

#[tauri::command]
pub fn save_settings(window: WebviewWindow, settings: SettingsIn) -> Result<SettingsView, String> {
    check_caller(&window)?;
    let app = window.app_handle();
    let old = config::get(app);
    let cfg = config::replace(app, settings.config.sanitize());

    actions::apply_all(app, &cfg);
    if cfg.hotkey != old.hotkey {
        crate::reregister_hotkey(app, &cfg.hotkey);
    }
    let al = app.autolaunch();
    if settings.autostart != al.is_enabled().unwrap_or(false) {
        let _ = if settings.autostart { al.enable() } else { al.disable() };
    }
    tray::sync_autostart_item(app);
    Ok(view(app))
}

/// Tray → "Settings…" (or Ctrl+,): open the window, or focus it if open.
pub fn open(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    let built = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("settings.html".into()))
        .title("Courier Settings")
        .inner_size(620.0, 760.0)
        .min_inner_size(480.0, 420.0)
        .center()
        .build();
    if let Err(e) = built {
        eprintln!("[settings] could not open the settings window: {e}");
    }
}
