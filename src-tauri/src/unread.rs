//! Unread chat count, shown as:
//!   * a taskbar overlay badge (1–9, or a dot for 10+) and the unread tray
//!     icon, both only while Courier isn't the active window (attention.rs),
//!   * the tray tooltip and a "(N) Courier" window title, always.
//!
//! Facebook prefixes the title with "(N) ", where N is unread chats plus
//! unread Facebook notifications; Courier subtracts the latter (see
//! COUNT_JS). While
//! a new message arrives it also briefly flashes titles like "Alex messaged
//! you"; those carry no count and are ignored rather than read as zero.
//! The title is read natively by Rust (on_document_title_changed), so the page
//! needs no IPC access for this. Titles are never logged.

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use tauri::image::Image;
use tauri::{AppHandle, Manager, WebviewWindow};

use crate::tray::{self, TRAY_ID};

const UNKNOWN: u32 = u32::MAX;
static LAST: AtomicU32 = AtomicU32::new(UNKNOWN);

/// `Some(n)` when the title states the unread count (0 for a plain
/// "Messenger | Facebook"-style title), `None` for transient titles.
pub fn parse(title: &str) -> Option<u32> {
    let t = title.trim_start();
    if let Some(rest) = t.strip_prefix('(') {
        if let Some(end) = rest.find(')') {
            let digits = &rest[..end];
            if !digits.is_empty() && digits.len() <= 6 && digits.bytes().all(|b| b.is_ascii_digit()) {
                return digits.parse().ok();
            }
        }
    }
    let lower = t.to_lowercase();
    if lower.contains("messenger") || lower.contains("facebook") || lower.contains("chats") {
        Some(0)
    } else {
        None
    }
}

/// Bumped on every title change, so only the latest delayed check runs.
static GEN: AtomicU64 = AtomicU64::new(0);

/// Facebook's title count is unread chats *plus* unread Facebook
/// notifications. The notification count is on the (hidden) top bar's
/// notifications button ("Notifications, N unread", with a numeric badge).
/// Returns `[title count, notification count]`; read in one snapshot so the
/// two numbers belong together. Only numbers leave the page.
const COUNT_JS: &str = r#"(function () {
  var m = /^\s*\((\d{1,6})\)/.exec(document.title);
  var total = m ? parseInt(m[1], 10) : 0;
  var notif = 0;
  var els = document.querySelectorAll('[role="banner"] [role="button"][aria-label], [role="banner"] a[aria-label]');
  for (var i = 0; i < els.length; i++) {
    var label = els[i].getAttribute("aria-label") || "";
    if (/messenger|chat/i.test(label)) continue; // a Messenger counter is what we want, not subtract
    var c = 0, lm = /(\d{1,6})/.exec(label);
    if (lm) c = parseInt(lm[1], 10);
    else {
      var leaves = els[i].querySelectorAll("span, div");
      for (var k = 0; k < leaves.length && k < 40; k++) {
        var tm = /^(\d{1,6})\+?$/.exec((leaves[k].textContent || "").trim());
        if (tm) { c = parseInt(tm[1], 10); break; }
      }
    }
    if (c > notif) notif = c;
  }
  return [total, notif];
})()"#;

/// Unread chats = title count − Facebook notification count.
fn messages_only(total: u32, notifications: u32) -> u32 {
    total.saturating_sub(notifications)
}

pub fn on_title_changed(window: &WebviewWindow, title: &str) {
    let generation = GEN.fetch_add(1, Ordering::SeqCst) + 1;
    match parse(title) {
        None => {}
        Some(0) => set(window.app_handle(), 0),
        Some(_) => {
            // Give the top bar's badge a moment to update along with the title.
            let w = window.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(700));
                if GEN.load(Ordering::SeqCst) != generation {
                    return; // a newer title arrived
                }
                let app = w.app_handle().clone();
                let _ = w.eval_with_callback(COUNT_JS, move |json| {
                    let Ok(v) = serde_json::from_str::<Vec<u64>>(&json) else { return };
                    let [total, notif] = v[..] else { return };
                    let cap = |n: u64| n.min(999_999) as u32;
                    #[cfg(debug_assertions)]
                    eprintln!("[unread] title {total}, facebook notifications {notif}");
                    set(&app, messages_only(cap(total), cap(notif)));
                });
            });
        }
    }
}

fn badge(n: u32) -> Option<Image<'static>> {
    let bytes: &'static [u8] = match n {
        0 => return None,
        1 => include_bytes!("../icons/badge-1.png"),
        2 => include_bytes!("../icons/badge-2.png"),
        3 => include_bytes!("../icons/badge-3.png"),
        4 => include_bytes!("../icons/badge-4.png"),
        5 => include_bytes!("../icons/badge-5.png"),
        6 => include_bytes!("../icons/badge-6.png"),
        7 => include_bytes!("../icons/badge-7.png"),
        8 => include_bytes!("../icons/badge-8.png"),
        9 => include_bytes!("../icons/badge-9.png"),
        _ => include_bytes!("../icons/badge-dot.png"),
    };
    Image::from_bytes(bytes).ok()
}

/// What the badge currently shows (UNKNOWN = not drawn yet).
static SHOWN: AtomicU32 = AtomicU32::new(UNKNOWN);

/// The badge (taskbar overlay and the tray's unread icon) is shown only
/// while you're not looking at Courier (see attention.rs). The count itself
/// stays in the window title and the tray tooltip either way.
fn badge_count(unread: u32, attentive: bool) -> u32 {
    if attentive { 0 } else { unread }
}

pub fn set(app: &AppHandle, n: u32) {
    if LAST.swap(n, Ordering::Relaxed) == n {
        return;
    }
    #[cfg(debug_assertions)]
    eprintln!("[unread] {n}");

    if let Some(w) = app.get_webview_window("main") {
        let _ = w.set_title(&if n > 0 { format!("({n}) Courier") } else { "Courier".into() });
    }
    if let Some(t) = app.tray_by_id(TRAY_ID) {
        let _ = t.set_tooltip(Some(if n > 0 { format!("Courier – {n} unread") } else { "Courier".into() }));
    }
    refresh(app);
}

/// Redraw the badge for the current count and attention state.
pub fn refresh(app: &AppHandle) {
    let unread = match LAST.load(Ordering::Relaxed) {
        UNKNOWN => 0,
        n => n,
    };
    let n = badge_count(unread, crate::attention::is_attentive());
    if SHOWN.swap(n, Ordering::Relaxed) == n {
        return;
    }
    #[cfg(debug_assertions)]
    eprintln!("[unread] badge {}", if n > 0 { n.to_string() } else { "hidden".into() });

    if let Some(w) = app.get_webview_window("main") {
        let _ = w.set_overlay_icon(badge(n));
    }
    if let Some(t) = app.tray_by_id(TRAY_ID) {
        let icon = if n > 0 {
            Image::from_bytes(include_bytes!("../icons/tray-unread.png")).ok()
        } else {
            Some(tray::tray_icon())
        };
        let _ = t.set_icon(icon);
    }
}

#[cfg(test)]
mod tests {
    use super::{badge_count, messages_only, parse};

    #[test]
    fn badge_hidden_only_while_attentive() {
        assert_eq!(badge_count(3, false), 3);
        assert_eq!(badge_count(3, true), 0);
        assert_eq!(badge_count(0, false), 0);
    }

    #[test]
    fn subtracts_facebook_notifications() {
        assert_eq!(messages_only(1, 1), 0);
        assert_eq!(messages_only(3, 1), 2);
        assert_eq!(messages_only(1, 5), 0);
    }

    #[test]
    fn titles() {
        assert_eq!(parse("(3) Messenger | Facebook"), Some(3));
        assert_eq!(parse("(12) Facebook"), Some(12));
        assert_eq!(parse("Messenger | Facebook"), Some(0));
        assert_eq!(parse("Chats"), Some(0));
        assert_eq!(parse("Alex messaged you"), None);
        assert_eq!(parse("(Alex) hi"), None);
        assert_eq!(parse("()"), None);
    }
}
