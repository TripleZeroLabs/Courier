//! System tray icon, its menu, and the show/hide logic shared with the
//! single-instance handler, global hotkey, shortcuts and notifications.

use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};
use tauri_plugin_autostart::ManagerExt as _;
use tauri_plugin_window_state::{AppHandleExt, StateFlags};

use crate::config;

pub const TRAY_ID: &str = "courier-tray";

/// Window-state fields that are saved/restored. VISIBLE is deliberately
/// excluded: visibility is controlled by the app (tray, autostart).
pub fn state_flags() -> StateFlags {
    StateFlags::SIZE | StateFlags::POSITION | StateFlags::MAXIMIZED
}

pub fn tray_icon() -> Image<'static> {
    Image::from_bytes(include_bytes!("../icons/tray.png")).expect("embedded tray icon")
}

/// Bring the main window to the front (restoring it if minimized/hidden).
pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        if w.is_minimized().unwrap_or(false) {
            let _ = w.unminimize();
        }
        let _ = w.set_focus();
    }
    crate::attention::update(app);
    crate::attention::schedule_update(app); // again once focus has settled
}

pub fn hide_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        // Persist geometry now; hidden windows don't emit move/resize events.
        let _ = app.save_window_state(state_flags());
        let _ = w.hide();
    }
    crate::attention::update(app);
}

/// Visible and not minimized → hide; otherwise → show and focus.
pub fn toggle_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let shown = w.is_visible().unwrap_or(false) && !w.is_minimized().unwrap_or(false);
        if shown {
            hide_main(app);
        } else {
            show_main(app);
        }
    }
}

pub fn reload_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.reload();
    }
}

pub fn quit(app: &AppHandle) {
    let _ = app.save_window_state(state_flags());
    app.exit(0);
}

/// The tray's "Auto-hide chat list" item, kept so Ctrl+Shift+S can update it.
struct SidebarItem(CheckMenuItem<tauri::Wry>);

pub fn sync_sidebar_item(app: &AppHandle, mode: &str) {
    if let Some(item) = app.try_state::<SidebarItem>() {
        let _ = item.0.set_checked(mode != config::SIDEBAR_PINNED);
    }
}

/// The tray's "Start with Windows" item, kept so the settings window can
/// update it.
struct AutostartItem(CheckMenuItem<tauri::Wry>);

pub fn sync_autostart_item(app: &AppHandle) {
    if let Some(item) = app.try_state::<AutostartItem>() {
        let _ = item.0.set_checked(app.autolaunch().is_enabled().unwrap_or(false));
    }
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let show_hide = MenuItem::with_id(app, "show_hide", "Show / Hide", true, None::<&str>)?;
    let settings_item = MenuItem::with_id(app, "settings", "Settings…\tCtrl+,", true, None::<&str>)?;
    let reload = MenuItem::with_id(app, "reload", "Reload", true, None::<&str>)?;
    let drawer_on = config::get(app).sidebar != config::SIDEBAR_PINNED;
    let sidebar = CheckMenuItem::with_id(
        app,
        "sidebar",
        "Auto-hide chat list\tCtrl+Shift+S",
        true,
        drawer_on,
        None::<&str>,
    )?;
    app.manage(SidebarItem(sidebar.clone()));
    let autostart_on = app.autolaunch().is_enabled().unwrap_or(false);
    let autostart = CheckMenuItem::with_id(app, "autostart", "Start with Windows", true, autostart_on, None::<&str>)?;
    app.manage(AutostartItem(autostart.clone()));
    let quit_item = MenuItem::with_id(app, "quit", "Quit Courier", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &show_hide,
            &reload,
            &PredefinedMenuItem::separator(app)?,
            &sidebar,
            &autostart,
            &settings_item,
            &PredefinedMenuItem::separator(app)?,
            &quit_item,
        ],
    )?;

    // Dev builds only: testing helpers.
    #[cfg(debug_assertions)]
    {
        menu.insert(
            &MenuItem::with_id(app, "test_notification", "Test notification (dev)", true, None::<&str>)?,
            2,
        )?;
        menu.insert(
            &MenuItem::with_id(app, "dump_structure", "Dump page structure (dev)", true, None::<&str>)?,
            3,
        )?;
    }

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(tray_icon())
        .tooltip("Courier")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "show_hide" => toggle_main(app),
            "reload" => reload_main(app),
            "sidebar" => crate::actions::toggle_sidebar(app),
            "settings" => crate::settings::open(app),
            "autostart" => {
                let al = app.autolaunch();
                let want = !al.is_enabled().unwrap_or(false);
                let _ = if want { al.enable() } else { al.disable() };
                // Reflect the real state, whether or not the change worked.
                let _ = autostart.set_checked(al.is_enabled().unwrap_or(false));
            }
            "quit" => quit(app),
            #[cfg(debug_assertions)]
            "test_notification" => {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.eval(
                        "new Notification('Courier test', { body: 'If you can read this, toasts work.' })",
                    );
                }
            }
            #[cfg(debug_assertions)]
            "dump_structure" => crate::declutter::dump_structure(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle_main(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}
