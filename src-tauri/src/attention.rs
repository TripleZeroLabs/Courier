//! Is the user looking at Courier?
//!
//! "Attentive" = the main window is shown, not minimized, and is the active
//! (foreground) window. Anything else (hidden in the tray, minimized, or open
//! but behind/beside another app) counts as not looking. That drives:
//!
//!   * the unread badge: shown unless attentive (unread.rs);
//!   * what Facebook sees: when not attentive, the page is told it is hidden,
//!     so Facebook doesn't mark the open conversation as read and keeps
//!     messages unread (and notifying) until you come back:
//!       - hidden/minimized: WebView2's own visibility (SetIsVisible), which
//!         also lets it stop painting a window nobody can see;
//!       - visible but not active: only the page's view of it
//!         (document.visibilityState / hidden / hasFocus, see declutter.js),
//!         so the window keeps drawing normally next to other apps.
//!
//! Window focus events can be noisy (focus moving between the window and the
//! webview inside it), so the state is re-checked against the real
//! foreground window shortly after any event.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::time::Duration;

use tauri::{AppHandle, Manager, WebviewWindow};

use crate::unread;

const UNKNOWN: u8 = 2;
/// Last state pushed out: attentive (1) / not (0) / never (2).
static LAST_ATTENTIVE: AtomicU8 = AtomicU8::new(UNKNOWN);
/// Last visibility given to WebView2.
static WEBVIEW_VISIBLE: AtomicBool = AtomicBool::new(true);
/// Whether attention is currently attentive (for unread.rs).
static ATTENTIVE: AtomicBool = AtomicBool::new(false);

pub fn is_attentive() -> bool {
    ATTENTIVE.load(Ordering::SeqCst)
}

/// Re-evaluate shortly (after focus has settled), on the main thread.
pub fn schedule_update(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(120));
        let a = app.clone();
        let _ = app.run_on_main_thread(move || update(&a));
    });
}

/// Re-evaluate now and apply any change.
pub fn update(app: &AppHandle) {
    let Some(w) = app.get_webview_window("main") else { return };
    let shown = w.is_visible().unwrap_or(false) && !w.is_minimized().unwrap_or(false);
    let attentive = shown && is_foreground(&w);

    // WebView2 visibility: only when the window can't be seen at all.
    if WEBVIEW_VISIBLE.swap(shown, Ordering::SeqCst) != shown {
        set_webview_visible(&w, shown);
    }

    ATTENTIVE.store(attentive, Ordering::SeqCst);
    if LAST_ATTENTIVE.swap(attentive as u8, Ordering::SeqCst) == attentive as u8 {
        return;
    }
    let _ = w.eval(format!("window.__courierSetAttention && window.__courierSetAttention({attentive})"));
    unread::refresh(app);

    #[cfg(debug_assertions)]
    {
        eprintln!("[attention] {} (shown={shown})", if attentive { "active" } else { "not active" });
        let probe = w.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            let _ = probe.eval_with_callback(
                "'visibilityState=' + document.visibilityState + ' hidden=' + document.hidden + ' hasFocus=' + document.hasFocus()",
                |r| eprintln!("[attention] page sees: {r}"),
            );
        });
    }
}

#[cfg(windows)]
fn is_foreground(w: &WebviewWindow) -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{GetAncestor, GetForegroundWindow, GA_ROOTOWNER};
    let Ok(ours) = w.hwnd() else { return false };
    let fg = unsafe { GetForegroundWindow() };
    if fg.0.is_null() {
        return false;
    }
    let root = unsafe { GetAncestor(fg, GA_ROOTOWNER) };
    // Compare raw handles (tauri may use a different `windows` crate version).
    root.0 as isize == ours.0 as isize || fg.0 as isize == ours.0 as isize
}

#[cfg(not(windows))]
fn is_foreground(w: &WebviewWindow) -> bool {
    w.is_focused().unwrap_or(false)
}

#[cfg(windows)]
fn set_webview_visible(w: &WebviewWindow, visible: bool) {
    let _ = w.with_webview(move |pw| unsafe {
        let _ = pw.controller().SetIsVisible(visible);
    });
}

#[cfg(not(windows))]
fn set_webview_visible(_w: &WebviewWindow, _visible: bool) {}
