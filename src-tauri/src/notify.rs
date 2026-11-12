//! Native Windows toasts for Messenger's web notifications.
//!
//! WebView2 raises `NotificationReceived` whenever the page calls
//! `new Notification(...)`. We take it over natively (no JavaScript shim and no
//! IPC exposed to the page): read the title/body, cap their length, show a
//! Windows toast, and when the toast is clicked, bring the window forward and
//! report the click back to the page so Facebook opens that conversation.
//!
//! Facebook activity (likes, comments...) arrives through the same event; it
//! is recognised and dropped so only messages produce toasts (see
//! classify_then_show).
//!
//! Message text goes only into the toast. It is never logged or stored.
//! The page still has to be granted notification permission through
//! WebView2's normal permission prompt.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};

use tauri::{AppHandle, Manager, Url, WebviewWindow};
use tauri_winrt_notification::{Sound, Toast};
use webview2_com::Microsoft::Web::WebView2::Win32::{
    ICoreWebView2, ICoreWebView2Controller, ICoreWebView2Notification, ICoreWebView2Profile4,
    ICoreWebView2_13, ICoreWebView2_24, COREWEBVIEW2_PERMISSION_KIND_NOTIFICATIONS,
    COREWEBVIEW2_PERMISSION_STATE_ALLOW,
};
use webview2_com::{take_pwstr, NotificationReceivedEventHandler, SetPermissionStateCompletedHandler};
use windows::core::{w, Interface, PWSTR};

use crate::tray;

const MAX_TITLE: usize = 128;
const MAX_BODY: usize = 512;
/// How many recent notifications we keep, so a click on a toast can be
/// reported back to the page. Older ones are reported closed and dropped.
const KEEP: usize = 32;

static SEQ: AtomicU64 = AtomicU64::new(1);

thread_local! {
    // WebView2 COM objects live on the UI thread; only touched from there.
    static RECENT: RefCell<VecDeque<(u64, ICoreWebView2Notification)>> =
        const { RefCell::new(VecDeque::new()) };
}

/// Hook native notifications on the given webview window.
pub fn install(window: &WebviewWindow) -> tauri::Result<()> {
    let app = window.app_handle().clone();
    window.with_webview(move |pw| {
        if let Err(e) = unsafe { hook(pw.controller(), app) } {
            // Old WebView2 runtimes lack ICoreWebView2_24.
            eprintln!("[notify] native notification hook unavailable: {e}");
        }
    })
}

/// Grant notification permission to https://www.facebook.com in this
/// WebView2 profile. Facebook only shows notifications when
/// `Notification.permission` is already "granted", and it rarely asks.
/// This is the only permission Courier pre-grants.
unsafe fn grant_notification_permission(core: &ICoreWebView2) -> windows::core::Result<()> {
    unsafe {
        let profile: ICoreWebView2Profile4 = core.cast::<ICoreWebView2_13>()?.Profile()?.cast()?;
        let done = SetPermissionStateCompletedHandler::create(Box::new(|result| {
            #[cfg(debug_assertions)]
            eprintln!("[notify] notification permission pre-granted: {:?}", result.map(|_| ()));
            #[cfg(not(debug_assertions))]
            let _ = result;
            Ok(())
        }));
        profile.SetPermissionState(
            COREWEBVIEW2_PERMISSION_KIND_NOTIFICATIONS,
            w!("https://www.facebook.com"),
            COREWEBVIEW2_PERMISSION_STATE_ALLOW,
            &done,
        )
    }
}

unsafe fn hook(controller: ICoreWebView2Controller, app: AppHandle) -> windows::core::Result<()> {
    let core = unsafe { controller.CoreWebView2()? };
    if let Err(e) = unsafe { grant_notification_permission(&core) } {
        eprintln!("[notify] could not pre-grant notification permission: {e}");
    }
    let core24: ICoreWebView2_24 = core.cast()?;

    let handler = NotificationReceivedEventHandler::create(Box::new(move |_sender, args| {
        let Some(args) = args else { return Ok(()) };
        unsafe {
            // We render the notification ourselves (or drop it).
            args.SetHandled(true)?;

            let mut origin = PWSTR::null();
            args.SenderOrigin(&mut origin)?;
            if !origin_allowed(&take_pwstr(origin)) {
                return Ok(());
            }
            // Turned off in settings: swallow it (handled, never shown).
            if !crate::config::get(&app).notifications {
                return Ok(());
            }

            let n = args.Notification()?;
            let mut t = PWSTR::null();
            n.Title(&mut t)?;
            let mut b = PWSTR::null();
            n.Body(&mut b)?;
            let title = clamp(take_pwstr(t), MAX_TITLE);
            let body = clamp(take_pwstr(b), MAX_BODY);
            #[cfg(debug_assertions)]
            debug_describe(&n, &title, &body);

            let id = SEQ.fetch_add(1, Ordering::Relaxed);
            RECENT.with(|r| {
                let mut r = r.borrow_mut();
                r.push_back((id, n.clone()));
                while r.len() > KEEP {
                    if let Some((_, old)) = r.pop_front() {
                        let _ = old.ReportClosed();
                    }
                }
            });

            classify_then_show(&app, id, title, body);
        }
        Ok(())
    }));

    let mut token = 0i64;
    unsafe { core24.add_NotificationReceived(&handler, &mut token) }
}

/// Dev builds only: describes a notification without its text, to work out
/// how Messenger notifications differ from other Facebook ones. Logs the tag,
/// icon/badge hosts and paths (no query), text lengths, and whether the title
/// is literally "Facebook"/"Messenger".
#[cfg(debug_assertions)]
unsafe fn debug_describe(n: &ICoreWebView2Notification, title: &str, body: &str) {
    fn uri(u: windows::core::Result<()>, p: PWSTR) -> String {
        if u.is_err() {
            return "?".into();
        }
        let s = take_pwstr(p);
        match Url::parse(&s) {
            Ok(u) => format!("{}{}", u.host_str().unwrap_or(""), &u.path()[..u.path().len().min(30)]),
            Err(_) if s.is_empty() => "-".into(),
            Err(_) => "(unparsed)".into(),
        }
    }
    unsafe {
        let mut tag = PWSTR::null();
        let tag = if n.Tag(&mut tag).is_ok() { take_pwstr(tag) } else { "?".into() };
        let tag: String = tag.chars().take(60).collect();
        let mut i = PWSTR::null();
        let icon = uri(n.IconUri(&mut i), i);
        let mut b = PWSTR::null();
        let badge = uri(n.BadgeUri(&mut b), b);
        let mut im = PWSTR::null();
        let image = uri(n.BodyImageUri(&mut im), im);
        let generic = matches!(title.trim().to_lowercase().as_str(), "facebook" | "messenger");
        eprintln!(
            "[notify] tag={tag:?} icon={icon} badge={badge} image={image} title_len={} generic_title={generic} body_len={}",
            title.chars().count(),
            body.chars().count()
        );
    }
}

/// How long to wait before deciding, so Facebook's notifications button has
/// updated (it may update just before or just after the notification).
const CLASSIFY_DELAY: std::time::Duration = std::time::Duration::from_millis(2000);
/// A rise in Facebook's notification count this recent (ms) means the
/// notification was Facebook activity, not a message.
const FB_ACTIVITY_WINDOW_MS: i64 = 10_000;
/// Milliseconds since Facebook's notification count last went up, or -1
/// (see declutter.js, "Facebook-notification tracker").
const FB_NOTIF_AGE_JS: &str =
    "(function () { var t = window.__courierFbNotifIncreasedAt; return t ? Date.now() - t : -1; })()";

fn is_facebook_activity(age_ms: i64) -> bool {
    (0..=FB_ACTIVITY_WINDOW_MS).contains(&age_ms)
}

/// Messenger only: Facebook delivers its own activity (likes, comments...)
/// through the same web notifications. Those raise the count on Facebook's
/// notifications button; messages don't. So wait briefly, check whether that
/// count just went up, and show the toast only if it didn't.
fn classify_then_show(app: &AppHandle, id: u64, title: String, body: String) {
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(CLASSIFY_DELAY);
        let Some(w) = app.get_webview_window("main") else {
            let a = app.clone();
            let _ = app.run_on_main_thread(move || finish(&a, id, &title, &body, false));
            return;
        };
        let a = app.clone();
        let result = w.eval_with_callback(FB_NOTIF_AGE_JS, move |json| {
            let age = serde_json::from_str::<f64>(&json).map(|v| v as i64).unwrap_or(-1);
            let fb = is_facebook_activity(age);
            let (a2, title, body) = (a.clone(), title.clone(), body.clone());
            let _ = a.run_on_main_thread(move || finish(&a2, id, &title, &body, fb));
        });
        if result.is_err() {
            // Couldn't ask the page; err on the side of showing it.
            #[cfg(debug_assertions)]
            eprintln!("[notify] #{id}: could not check notification type; showing it");
        }
    });
}

/// UI thread: show the toast for a message, or quietly drop Facebook activity.
fn finish(app: &AppHandle, id: u64, title: &str, body: &str, facebook_activity: bool) {
    let n = RECENT.with(|r| r.borrow().iter().find(|(i, _)| *i == id).map(|(_, n)| n.clone()));
    #[cfg(debug_assertions)]
    eprintln!("[notify] #{id}: {}", if facebook_activity { "Facebook activity, not shown" } else { "message" });
    if facebook_activity {
        RECENT.with(|r| r.borrow_mut().retain(|(i, _)| *i != id));
        if let Some(n) = n {
            let _ = unsafe { n.ReportClosed() };
        }
        return;
    }
    if show_toast(app, id, title, body) {
        if let Some(n) = n {
            let _ = unsafe { n.ReportShown() };
        }
    }
}

fn origin_allowed(origin: &str) -> bool {
    Url::parse(origin)
        .ok()
        .filter(|u| u.scheme() == "https")
        .and_then(|u| u.host_str().map(str::to_owned))
        .is_some_and(|h| h == "facebook.com" || h.ends_with(".facebook.com"))
}

fn clamp(s: String, max: usize) -> String {
    let s: String = s.chars().filter(|c| !c.is_control() || *c == '\n').collect();
    if s.chars().count() <= max {
        s
    } else {
        let mut out: String = s.chars().take(max - 1).collect();
        out.push('…');
        out
    }
}

/// Toasts need an AppUserModelID with a Start-menu shortcut. The installer
/// registers Courier's identifier; builds run from `target\` (dev, or an
/// uninstalled release exe) borrow PowerShell's ID so toasts still appear.
fn app_id(app: &AppHandle) -> String {
    let from_target_dir = std::env::current_exe()
        .map(|p| p.components().any(|c| c.as_os_str().eq_ignore_ascii_case("target")))
        .unwrap_or(true);
    if from_target_dir {
        Toast::POWERSHELL_APP_ID.to_owned()
    } else {
        app.config().identifier.clone()
    }
}

fn show_toast(app: &AppHandle, id: u64, title: &str, body: &str) -> bool {
    let click_app = app.clone();
    let result = Toast::new(&app_id(app))
        .title(if title.is_empty() { "Courier" } else { title })
        .text1(body)
        .sound(Some(Sound::Default))
        .on_activated(move |_action| {
            let app = click_app.clone();
            let _ = click_app.run_on_main_thread(move || on_clicked(&app, id));
            Ok(())
        })
        .show();
    #[cfg(debug_assertions)]
    eprintln!("[notify] toast #{id} shown: {}", result.is_ok());
    result.is_ok()
}

fn on_clicked(app: &AppHandle, id: u64) {
    tray::show_main(app);
    let n = RECENT.with(|r| {
        let mut r = r.borrow_mut();
        let pos = r.iter().position(|(i, _)| *i == id)?;
        r.remove(pos).map(|(_, n)| n)
    });
    if let Some(n) = n {
        // Lets the page run its own click handler (opens that chat).
        let _ = unsafe { n.ReportClicked() };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origins() {
        assert!(origin_allowed("https://www.facebook.com"));
        assert!(!origin_allowed("http://www.facebook.com"));
        assert!(!origin_allowed("https://facebook.com.evil.example"));
        assert!(!origin_allowed("https://example.com"));
    }

    #[test]
    fn facebook_activity_window() {
        assert!(!is_facebook_activity(-1)); // never went up
        assert!(is_facebook_activity(0));
        assert!(is_facebook_activity(3_000));
        assert!(!is_facebook_activity(60_000)); // an old rise doesn't count
    }

    #[test]
    fn clamping() {
        assert_eq!(clamp("hi\u{7}".into(), 10), "hi");
        assert_eq!(clamp("abcdef".into(), 4).chars().count(), 4);
    }
}
