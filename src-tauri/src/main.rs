// No console window in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod actions;
mod attention;
mod config;
mod declutter;
#[cfg(windows)]
mod keys;
mod nav;
#[cfg(windows)]
mod notify;
mod settings;
mod theme;
mod tray;
mod unread;

use std::sync::Mutex;

use tauri::webview::{PageLoadEvent, PermissionKind, PermissionResponse};
use tauri::{AppHandle, Manager, Url, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

/// Passed by the autostart entry: start hidden in the tray.
const MINIMIZED_ARG: &str = "--minimized";

fn create_main_window(app: &AppHandle, cfg: &config::Config) -> tauri::Result<WebviewWindow> {
    let url = Url::parse(nav::MESSAGES_URL).expect("valid constant URL");
    let (a1, a2) = (app.clone(), app.clone());

    let window = WebviewWindowBuilder::new(app, "main", WebviewUrl::External(url))
        .title("Courier")
        .inner_size(1100.0, 780.0)
        .min_inner_size(420.0, 400.0)
        // Start hidden so the saved size/position (restored by the
        // window-state plugin during build) is applied before it appears.
        .visible(false)
        // Dedicated WebView2 profile: %LOCALAPPDATA%\dev.personal.courier\webview
        // Isolated from Edge/Chrome, persists cookies/login across restarts.
        .data_directory(nav::webview_data_dir(app)?)
        // Declutter CSS/JS (src-tauri/inject/), runs before Facebook's scripts.
        .initialization_script(declutter::init_script(cfg))
        .on_navigation(move |u| nav::on_navigation(&a1, "main", u))
        .on_new_window(move |u, f| nav::on_new_window(&a2, u, f))
        // Unread count comes from the "(N) …" title prefix; read natively.
        .on_document_title_changed(|w, title| unread::on_title_changed(&w, &title))
        // A full load re-runs the injected script with the config value.
        .on_page_load(|w, p| match p.event() {
            PageLoadEvent::Started => actions::reset_declutter_override(),
            // The init script carries the launch-time settings; re-apply the
            // current ones in case they were changed since.
            PageLoadEvent::Finished => actions::apply_page_settings(&w, &config::get(w.app_handle())),
        })
        // Notifications are allowed (see also notify.rs, which pre-grants them
        // for facebook.com and checks every notification's origin).
        // Everything else, including mic/camera for calls, gets WebView2's
        // normal permission prompt.
        .on_permission_request(|_, kind| match kind {
            PermissionKind::Notifications => PermissionResponse::Allow,
            _ => PermissionResponse::Default,
        })
        .build()?;

    let _ = window.set_zoom(cfg.zoom);
    // Before the first document exists, the webview shows its background
    // colour; match the loading screen (declutter.js) so nothing flashes.
    if window.theme().ok() == Some(tauri::Theme::Dark) {
        let _ = window.set_background_color(Some(tauri::window::Color(0x18, 0x19, 0x1a, 0xff)));
    }

    #[cfg(windows)]
    {
        notify::install(&window)?;
        keys::install(&window)?;
    }

    // Closing the main window hides it to the tray instead of quitting.
    // Popup windows (calls) close normally.
    let handle = app.clone();
    window.on_window_event(move |event| match event {
        WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            tray::hide_main(&handle);
        }
        // Activated/deactivated, minimized/restored: re-check attention
        // (badge, and whether Facebook may mark chats as read).
        WindowEvent::Focused(_) => attention::schedule_update(&handle),
        WindowEvent::Resized(_) => {
            attention::schedule_update(&handle);
            // Dev builds: re-dump the layout after resizing (layout debugging).
            #[cfg(debug_assertions)]
            declutter::dump_after_resize(&handle);
        }
        _ => {}
    });

    Ok(window)
}

/// Settings window changed the hotkey: drop the old one, register the new.
pub fn reregister_hotkey(app: &AppHandle, hotkey: &str) {
    let _ = app.global_shortcut().unregister_all();
    register_hotkey(app, hotkey);
}

fn register_hotkey(app: &AppHandle, hotkey: &str) {
    let hotkey = hotkey.trim();
    if hotkey.is_empty() {
        return; // Disabled (the default).
    }
    let gs = app.global_shortcut();
    let result = gs.on_shortcut(hotkey, |app, _shortcut, event| {
        if event.state() == ShortcutState::Pressed {
            tray::toggle_main(app);
        }
    });
    if let Err(e) = result {
        // Invalid syntax, or another app already owns this combination.
        eprintln!("[hotkey] could not register {hotkey:?}: {e}");
    }
}

fn main() {
    let start_minimized = std::env::args().any(|a| a == MINIMIZED_ARG);

    tauri::Builder::default()
        // Must be registered first: a second launch exits immediately and
        // this callback runs in the already-running instance.
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            // An autostart launch while already running shouldn't pop the window.
            if !args.iter().any(|a| a == MINIMIZED_ARG) {
                tray::show_main(app);
            }
        }))
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(tray::state_flags())
                .with_filter(|label| label == "main")
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        // Off by default; toggled from the tray. Registers
        // HKCU\...\Run\Courier = "<exe> --minimized".
        .plugin(tauri_plugin_autostart::Builder::new().arg(MINIMIZED_ARG).build())
        // The only IPC commands, granted to the local "settings" window only
        // (capabilities/settings.json); see settings.rs.
        .invoke_handler(tauri::generate_handler![settings::get_settings, settings::save_settings])
        .setup(move |app| {
            let handle = app.handle().clone();
            let cfg = config::load(&handle);
            app.manage(config::Shared(Mutex::new(cfg.clone())));

            tray::create(&handle)?;
            create_main_window(&handle, &cfg)?;
            register_hotkey(&handle, &cfg.hotkey);
            // Dev builds only: write the page-structure dump once Messenger
            // has settled (same as the tray's "Dump page structure (dev)").
            #[cfg(debug_assertions)]
            {
                let h = handle.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(30));
                    declutter::dump_structure(&h);
                    declutter::ipc_probe(&h);
                });
            }
            if !start_minimized {
                tray::show_main(&handle);
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Courier");
}
