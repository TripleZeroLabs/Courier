//! In-window keyboard shortcuts, handled natively through WebView2's
//! AcceleratorKeyPressed event (so the page gets no IPC for this):
//!
//!   Ctrl+R / F5          reload
//!   Ctrl+= / Ctrl++      zoom in        (persisted in config.json)
//!   Ctrl+- / Ctrl+-      zoom out
//!   Ctrl+0               reset zoom
//!   Ctrl+Shift+D         toggle declutter (until reload)
//!   Ctrl+Shift+S         chat list: auto-hiding drawer / pinned (saved)
//!   Ctrl+,               open Settings
//!   Esc                  hide to tray   (only if esc_hides_window is true)

use tauri::{AppHandle, Manager, WebviewWindow};
use webview2_com::AcceleratorKeyPressedEventHandler;
use webview2_com::Microsoft::Web::WebView2::Win32::{
    ICoreWebView2Controller, COREWEBVIEW2_KEY_EVENT_KIND, COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_CONTROL, VK_MENU, VK_SHIFT};

use crate::actions::{self, Zoom};
use crate::{config, tray};

#[derive(Debug, Clone, Copy, PartialEq)]
enum Action {
    Reload,
    ZoomIn,
    ZoomOut,
    ZoomReset,
    ToggleDeclutter,
    ToggleSidebar,
    OpenSettings,
    Hide,
}

const VK_R: u32 = 0x52;
const VK_D: u32 = 0x44;
const VK_S: u32 = 0x53;
const VK_OEM_COMMA: u32 = 0xBC;
const VK_0: u32 = 0x30;
const VK_NUMPAD0: u32 = 0x60;
const VK_ADD: u32 = 0x6B;
const VK_SUBTRACT: u32 = 0x6D;
const VK_OEM_PLUS: u32 = 0xBB;
const VK_OEM_MINUS: u32 = 0xBD;
const VK_ESCAPE: u32 = 0x1B;
const VK_F5: u32 = 0x74;

fn map(vk: u32, ctrl: bool, shift: bool, alt: bool, esc_hides: bool) -> Option<Action> {
    if alt {
        return None;
    }
    match (ctrl, shift, vk) {
        (true, false, VK_R) | (false, false, VK_F5) => Some(Action::Reload),
        (true, _, VK_OEM_PLUS | VK_ADD) => Some(Action::ZoomIn),
        (true, false, VK_OEM_MINUS | VK_SUBTRACT) => Some(Action::ZoomOut),
        (true, false, VK_0 | VK_NUMPAD0) => Some(Action::ZoomReset),
        (true, true, VK_D) => Some(Action::ToggleDeclutter),
        (true, true, VK_S) => Some(Action::ToggleSidebar),
        (true, false, VK_OEM_COMMA) => Some(Action::OpenSettings),
        (false, false, VK_ESCAPE) if esc_hides => Some(Action::Hide),
        _ => None,
    }
}

fn run(app: &AppHandle, a: Action) {
    match a {
        Action::Reload => tray::reload_main(app),
        Action::ZoomIn => actions::zoom(app, Zoom::In),
        Action::ZoomOut => actions::zoom(app, Zoom::Out),
        Action::ZoomReset => actions::zoom(app, Zoom::Reset),
        Action::ToggleDeclutter => actions::toggle_declutter(app),
        Action::ToggleSidebar => actions::toggle_sidebar(app),
        Action::OpenSettings => crate::settings::open(app),
        Action::Hide => tray::hide_main(app),
    }
}

pub fn install(window: &WebviewWindow) -> tauri::Result<()> {
    let app = window.app_handle().clone();
    window.with_webview(move |pw| {
        if let Err(e) = unsafe { hook(pw.controller(), app) } {
            eprintln!("[keys] shortcut hook unavailable: {e}");
        }
    })
}

unsafe fn hook(controller: ICoreWebView2Controller, app: AppHandle) -> windows::core::Result<()> {
    let handler = AcceleratorKeyPressedEventHandler::create(Box::new(move |_c, args| {
        let Some(args) = args else { return Ok(()) };
        unsafe {
            let mut kind = COREWEBVIEW2_KEY_EVENT_KIND::default();
            args.KeyEventKind(&mut kind)?;
            if kind != COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN {
                return Ok(());
            }
            let mut vk = 0u32;
            args.VirtualKey(&mut vk)?;
            let down = |k: i32| GetKeyState(k) < 0;
            let ctrl = down(VK_CONTROL.0 as i32);
            let shift = down(VK_SHIFT.0 as i32);
            let alt = down(VK_MENU.0 as i32);
            let esc_hides = config::get(&app).esc_hides_window;

            if let Some(action) = map(vk, ctrl, shift, alt, esc_hides) {
                args.SetHandled(true)?;
                // Run after this callback returns, not inside WebView2's event.
                let app2 = app.clone();
                let _ = app.run_on_main_thread(move || run(&app2, action));
            }
        }
        Ok(())
    }));
    let mut token = 0i64;
    unsafe { controller.add_AcceleratorKeyPressed(&handler, &mut token) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mapping() {
        assert_eq!(map(VK_R, true, false, false, false), Some(Action::Reload));
        assert_eq!(map(VK_OEM_PLUS, true, true, false, false), Some(Action::ZoomIn));
        assert_eq!(map(VK_OEM_MINUS, true, false, false, false), Some(Action::ZoomOut));
        assert_eq!(map(VK_0, true, false, false, false), Some(Action::ZoomReset));
        assert_eq!(map(VK_D, true, true, false, false), Some(Action::ToggleDeclutter));
        assert_eq!(map(VK_S, true, true, false, false), Some(Action::ToggleSidebar));
        assert_eq!(map(VK_S, true, false, false, false), None);
        assert_eq!(map(VK_OEM_COMMA, true, false, false, false), Some(Action::OpenSettings));
        assert_eq!(map(VK_ESCAPE, false, false, false, false), None);
        assert_eq!(map(VK_ESCAPE, false, false, false, true), Some(Action::Hide));
        assert_eq!(map(VK_R, true, false, true, false), None);
        assert_eq!(map(VK_D, true, false, false, false), None);
    }
}
