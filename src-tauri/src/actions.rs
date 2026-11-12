//! App actions shared by the tray menu, keyboard shortcuts and the global
//! hotkey: zoom and the declutter toggle. (Show/hide/reload live in tray.rs.)

use tauri::{AppHandle, Manager};

use crate::config;

/// Zoom steps, matching Chromium's.
const ZOOM_STEPS: &[f64] = &[0.5, 0.67, 0.75, 0.8, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0];

pub enum Zoom {
    In,
    Out,
    Reset,
}

fn next_zoom(current: f64, z: Zoom) -> f64 {
    match z {
        Zoom::Reset => 1.0,
        Zoom::In => ZOOM_STEPS.iter().copied().find(|s| *s > current + 1e-3).unwrap_or(3.0),
        Zoom::Out => ZOOM_STEPS.iter().rev().copied().find(|s| *s < current - 1e-3).unwrap_or(0.5),
    }
}

pub fn zoom(app: &AppHandle, z: Zoom) {
    let cfg = config::update(app, |c| c.zoom = next_zoom(c.zoom, z));
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.set_zoom(cfg.zoom);
    }
}

/// Declutter on/off for this page. `None` = not toggled yet (use config).
static DECLUTTER_OVERRIDE: std::sync::Mutex<Option<bool>> = std::sync::Mutex::new(None);

/// Ctrl+Shift+D. Deliberately not saved: it's for checking whether declutter
/// is involved when something looks wrong, and a saved "off" would bring
/// Facebook's top bar back on every launch. The permanent setting is
/// `declutter` in config.json. A reload also returns to that setting.
pub fn toggle_declutter(app: &AppHandle) {
    let on = {
        let mut o = DECLUTTER_OVERRIDE.lock().unwrap_or_else(|e| e.into_inner());
        let on = !o.unwrap_or_else(|| config::get(app).declutter);
        *o = Some(on);
        on
    };
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.eval(format!("window.__courierSetDeclutter && window.__courierSetDeclutter({on})"));
    }
}

/// Ctrl+Shift+S / tray: switch the chat list between "drawer" and "pinned".
/// Saved in config.json.
pub fn toggle_sidebar(app: &AppHandle) {
    let cfg = config::update(app, |c| {
        c.sidebar = if c.sidebar == config::SIDEBAR_PINNED {
            config::SIDEBAR_DRAWER.into()
        } else {
            config::SIDEBAR_PINNED.into()
        };
    });
    if let Some(w) = app.get_webview_window("main") {
        apply_sidebar(&w, &cfg.sidebar);
    }
    crate::tray::sync_sidebar_item(app, &cfg.sidebar);
}

pub fn apply_sidebar(w: &tauri::WebviewWindow, mode: &str) {
    let mode = serde_json::to_string(mode).unwrap_or_else(|_| "\"drawer\"".into());
    let _ = w.eval(format!("window.__courierSetSidebar && window.__courierSetSidebar({mode})"));
}

/// Push the page-side settings (chat list mode, theme, declutter) into the
/// running page.
pub fn apply_page_settings(w: &tauri::WebviewWindow, cfg: &config::Config) {
    apply_sidebar(w, &cfg.sidebar);
    let theme = crate::declutter::theme_json(&cfg.theme);
    let _ = w.eval(format!("window.__courierSetTheme && window.__courierSetTheme({theme})"));
    let _ = w.eval(format!("window.__courierSetDeclutter && window.__courierSetDeclutter({})", cfg.declutter));
    // A new document starts out "attentive"; give it the real state.
    let attentive = crate::attention::is_attentive();
    let _ = w.eval(format!("window.__courierSetAttention && window.__courierSetAttention({attentive})"));
}

/// After the settings window saves: apply everything live.
pub fn apply_all(app: &AppHandle, cfg: &config::Config) {
    reset_declutter_override();
    if let Some(w) = app.get_webview_window("main") {
        apply_page_settings(&w, cfg);
        let _ = w.set_zoom(cfg.zoom);
    }
    crate::tray::sync_sidebar_item(app, &cfg.sidebar);
}

/// Called on every full page load: the injected script starts from the
/// config value again, so forget any Ctrl+Shift+D override.
pub fn reset_declutter_override() {
    *DECLUTTER_OVERRIDE.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_steps() {
        assert_eq!(next_zoom(1.0, Zoom::In), 1.1);
        assert_eq!(next_zoom(1.0, Zoom::Out), 0.9);
        assert_eq!(next_zoom(3.0, Zoom::In), 3.0);
        assert_eq!(next_zoom(0.5, Zoom::Out), 0.5);
        assert_eq!(next_zoom(1.03, Zoom::In), 1.1);
        assert_eq!(next_zoom(2.2, Zoom::Reset), 1.0);
    }
}
