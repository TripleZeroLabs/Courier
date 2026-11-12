//! User settings stored as JSON in %APPDATA%\dev.personal.courier\config.json.
//! Unknown or invalid values fall back to defaults; the file is (re)written
//! with every field so it is easy to discover and edit by hand. The settings
//! window (settings.rs) edits the same file and applies changes live; edits
//! made by hand take effect on the next launch.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{AppHandle, Manager};

use crate::theme::Theme;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Global show/hide hotkey, e.g. "Ctrl+Alt+M", "Ctrl+Shift+Space".
    /// Empty (the default) = no global hotkey.
    pub hotkey: String,
    /// Hide to tray when Esc is pressed (off by default: Messenger uses Esc
    /// to close its own dialogs).
    pub esc_hides_window: bool,
    /// Hide Facebook's non-Messenger UI (see src-tauri/inject/). Toggle at
    /// runtime with Ctrl+Shift+D.
    pub declutter: bool,
    /// Page zoom factor, persisted across restarts. Clamped to 0.5..=3.0.
    pub zoom: f64,
    /// Chat list: "drawer" (auto-hides at the left edge) or "pinned"
    /// (Facebook's normal layout). Toggle with Ctrl+Shift+S or the tray.
    pub sidebar: String,
    /// Show Windows toasts for Messenger notifications.
    pub notifications: bool,
    /// Colors, sizes, light/dark mode and custom CSS (see theme.rs).
    pub theme: Theme,
}

pub const SIDEBAR_DRAWER: &str = "drawer";
pub const SIDEBAR_PINNED: &str = "pinned";
const MAX_HOTKEY: usize = 64;

impl Default for Config {
    fn default() -> Self {
        Self {
            hotkey: String::new(),
            esc_hides_window: false,
            declutter: true,
            zoom: 1.0,
            sidebar: SIDEBAR_DRAWER.into(),
            notifications: true,
            theme: Theme::default(),
        }
    }
}

impl Config {
    pub fn sanitize(mut self) -> Self {
        if !self.zoom.is_finite() {
            self.zoom = 1.0;
        }
        self.zoom = self.zoom.clamp(0.5, 3.0);
        if self.sidebar != SIDEBAR_PINNED {
            self.sidebar = SIDEBAR_DRAWER.into();
        }
        self.hotkey = self.hotkey.trim().chars().take(MAX_HOTKEY).collect();
        self.theme = self.theme.sanitize();
        self
    }
}

/// Replace the whole config (from the settings window) and persist it.
pub fn replace(app: &AppHandle, new: Config) -> Config {
    update(app, |c| *c = new)
}

/// The live config, managed as Tauri state.
pub struct Shared(pub Mutex<Config>);

pub fn path(app: &AppHandle) -> tauri::Result<PathBuf> {
    Ok(app.path().app_config_dir()?.join("config.json"))
}

pub fn load(app: &AppHandle) -> Config {
    let Ok(p) = path(app) else {
        return Config::default();
    };
    let cfg = std::fs::read_to_string(&p)
        .ok()
        .and_then(|s| serde_json::from_str::<Config>(&s).ok())
        .unwrap_or_default()
        .sanitize();
    // Write back so new fields appear in the file with their defaults.
    let _ = save_to(&p, &cfg);
    cfg
}

/// Snapshot of the current config.
pub fn get(app: &AppHandle) -> Config {
    app.state::<Shared>().0.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Modify the config and persist it.
pub fn update(app: &AppHandle, f: impl FnOnce(&mut Config)) -> Config {
    let state = app.state::<Shared>();
    let mut cfg = state.0.lock().unwrap_or_else(|e| e.into_inner());
    f(&mut cfg);
    *cfg = cfg.clone().sanitize();
    if let Ok(p) = path(app) {
        let _ = save_to(&p, &cfg);
    }
    cfg.clone()
}

fn save_to(p: &Path, cfg: &Config) -> std::io::Result<()> {
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let json = serde_json::to_string_pretty(cfg).map_err(std::io::Error::other)?;
    std::fs::write(p, json)
}
