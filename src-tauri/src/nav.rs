//! Navigation guard and new-window handling.
//!
//! Every top-level navigation of a Courier window goes through `classify`:
//!   * Messenger, login, 2FA/checkpoint and call pages stay in the app.
//!   * The Facebook home page (where login redirects to) is sent back to /messages.
//!   * Other Facebook pages (profiles, feed, marketplace…) open in the default browser.
//!   * Facebook's outbound link shim (l.facebook.com/l.php?u=…) is unwrapped and
//!     the real target opens in the default browser.
//!   * Any other http(s)/mailto link opens in the default browser.
//!   * Anything else (javascript:, file:, custom protocol handlers) is blocked.
//!
//! Subframe navigations (iframes) are not routed through here.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::webview::{NewWindowFeatures, NewWindowResponse};
use tauri::{AppHandle, Manager, Url, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tauri_plugin_opener::OpenerExt;

pub const MESSAGES_URL: &str = "https://www.facebook.com/messages/";

/// Hosts that serve the Messenger web client and its login flow.
const APP_HOSTS: &[&str] = &[
    "facebook.com",
    "www.facebook.com",
    "web.facebook.com",
    "m.facebook.com",
    "secure.facebook.com",
];

/// Path prefixes on APP_HOSTS that may load inside the app.
/// `/foo` matches `/foo`, `/foo/...` and `/foo.php`.
const IN_APP_PATHS: &[&str] = &[
    "/messages",
    "/groupcall",             // voice/video call windows
    "/videocall",
    "/login",
    "/logout",
    "/checkpoint",            // security checks, 2FA
    "/two_step_verification", // 2FA
    "/two_factor",
    "/recover",               // password recovery
    "/confirmemail",
    "/device-based",          // "continue as" device login
    "/auth_platform",
    "/cookie",                // cookie consent (EU)
    "/privacy/consent",
];

/// Paths that just mean "Facebook home": redirect them to /messages.
const HOME_PATHS: &[&str] = &["/", "/home.php", "/index.php"];

/// Whole hosts allowed in the app regardless of path (account/security settings
/// that the login and 2FA flows can hand off to).
const IN_APP_ANY_PATH_HOSTS: &[&str] = &["accountscenter.facebook.com"];

/// Facebook's outbound-link redirector hosts.
const LINK_SHIM_HOSTS: &[&str] = &["l.facebook.com", "lm.facebook.com"];

#[derive(Debug)]
pub enum Decision {
    Allow,
    ToMessages,
    External(Url),
    Block,
}

pub fn classify(url: &Url) -> Decision {
    match url.scheme() {
        // about:blank (popups start here); blob: is same-origin by construction.
        "about" | "blob" => return Decision::Allow,
        "http" | "https" => {}
        "mailto" => return Decision::External(url.clone()),
        _ => return Decision::Block,
    }

    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    let path = url.path();

    if LINK_SHIM_HOSTS.contains(&host.as_str()) {
        let target = url
            .query_pairs()
            .find(|(k, _)| k == "u")
            .and_then(|(_, v)| Url::parse(&v).ok())
            .filter(|u| matches!(u.scheme(), "http" | "https"));
        return Decision::External(target.unwrap_or_else(|| url.clone()));
    }

    if host == "messenger.com" || host.ends_with(".messenger.com") {
        // messenger.com was retired in April 2026; it just redirects anyway.
        return Decision::ToMessages;
    }

    if APP_HOSTS.contains(&host.as_str()) {
        if HOME_PATHS.contains(&path) {
            return Decision::ToMessages;
        }
        if IN_APP_PATHS.iter().any(|p| path_has_prefix(path, p)) {
            return Decision::Allow;
        }
        return Decision::External(url.clone());
    }

    if IN_APP_ANY_PATH_HOSTS.contains(&host.as_str()) {
        return Decision::Allow;
    }

    Decision::External(url.clone())
}

fn path_has_prefix(path: &str, prefix: &str) -> bool {
    match path.strip_prefix(prefix) {
        Some(rest) => rest.is_empty() || rest.starts_with('/') || rest.starts_with(".php"),
        None => false,
    }
}

/// Debug builds print navigation decisions (host + path only, never the query
/// string or fragment, which can contain IDs). Release builds log nothing.
fn trace(kind: &str, url: &Url, d: &Decision) {
    // Print only the decision's name: External(..) carries a full URL
    // (query string included) that must not be logged.
    #[cfg(debug_assertions)]
    eprintln!(
        "[nav] {kind:<10} {:<10} {}://{}{}",
        match d {
            Decision::Allow => "Allow",
            Decision::ToMessages => "ToMessages",
            Decision::External(_) => "External",
            Decision::Block => "Block",
        },
        url.scheme(),
        url.host_str().unwrap_or_default(),
        url.path()
    );
    #[cfg(not(debug_assertions))]
    let _ = (kind, url, d);
}

pub fn webview_data_dir(app: &AppHandle) -> tauri::Result<PathBuf> {
    Ok(app.path().app_local_data_dir()?.join("webview"))
}

fn messages_url() -> Url {
    Url::parse(MESSAGES_URL).expect("valid constant URL")
}

/// Guards against a redirect loop if Facebook keeps bouncing /messages to the
/// home page (e.g. some logged-out states): after 3 redirects within 10 s,
/// the home page is allowed to load.
static REDIRECTS: Mutex<Option<(Instant, u32)>> = Mutex::new(None);

fn redirect_allowed() -> bool {
    let mut g = REDIRECTS.lock().unwrap_or_else(|e| e.into_inner());
    let now = Instant::now();
    let (start, n) = match *g {
        Some((t, n)) if now.duration_since(t) < Duration::from_secs(10) => (t, n + 1),
        _ => (now, 1),
    };
    *g = Some((start, n));
    n <= 3
}

fn open_external(app: &AppHandle, url: &Url) {
    let _ = app.opener().open_url(url.as_str(), None::<&str>);
}

/// `on_navigation` handler. Returning false cancels the navigation.
pub fn on_navigation(app: &AppHandle, label: &str, url: &Url) -> bool {
    let d = classify(url);
    trace("navigate", url, &d);
    match d {
        Decision::Allow => true,
        Decision::ToMessages => {
            if !redirect_allowed() {
                return true;
            }
            // Navigate after this handler returns rather than re-entering the
            // webview from inside its own navigation callback.
            let app2 = app.clone();
            let label = label.to_owned();
            let _ = app.run_on_main_thread(move || {
                if let Some(w) = app2.get_webview_window(&label) {
                    let _ = w.navigate(messages_url());
                }
            });
            false
        }
        Decision::External(u) => {
            open_external(app, &u);
            false
        }
        Decision::Block => false,
    }
}

/// `on_new_window` handler (window.open, target=_blank).
pub fn on_new_window(app: &AppHandle, url: Url, features: NewWindowFeatures) -> NewWindowResponse<tauri::Wry> {
    let d = classify(&url);
    trace("new-window", &url, &d);
    match d {
        // In-app Facebook pages (mainly call windows) get their own Courier
        // window, sharing the session and the same navigation guard.
        Decision::Allow => match build_popup(app, features) {
            Ok(window) => NewWindowResponse::Create { window },
            Err(_) => NewWindowResponse::Deny,
        },
        Decision::ToMessages => {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
            NewWindowResponse::Deny
        }
        Decision::External(u) => {
            open_external(app, &u);
            NewWindowResponse::Deny
        }
        Decision::Block => NewWindowResponse::Deny,
    }
}

static POPUP_SEQ: AtomicU32 = AtomicU32::new(1);

fn build_popup(app: &AppHandle, features: NewWindowFeatures) -> tauri::Result<WebviewWindow> {
    let label = format!("popup-{}", POPUP_SEQ.fetch_add(1, Ordering::Relaxed));
    let blank = Url::parse("about:blank").expect("valid constant URL");
    let (a1, a2) = (app.clone(), app.clone());
    let nav_label = label.clone();
    WebviewWindowBuilder::new(app, &label, WebviewUrl::External(blank))
        .title("Courier")
        .window_features(features)
        .data_directory(webview_data_dir(app)?)
        .on_navigation(move |u| on_navigation(&a1, &nav_label, u))
        .on_new_window(move |u, f| on_new_window(&a2, u, f))
        .on_document_title_changed(|w, title| {
            let _ = w.set_title(if title.is_empty() { "Courier" } else { &title });
        })
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Decision {
        classify(&Url::parse(s).unwrap())
    }

    #[test]
    fn rules() {
        assert!(matches!(d("https://www.facebook.com/messages/t/123"), Decision::Allow));
        assert!(matches!(d("https://www.facebook.com/messages"), Decision::Allow));
        assert!(matches!(d("https://www.facebook.com/login.php?next=x"), Decision::Allow));
        assert!(matches!(d("https://www.facebook.com/checkpoint/?next"), Decision::Allow));
        assert!(matches!(d("https://www.facebook.com/groupcall/ROOM:123/"), Decision::Allow));
        assert!(matches!(d("https://www.facebook.com/"), Decision::ToMessages));
        assert!(matches!(d("https://www.messenger.com/t/1"), Decision::ToMessages));
        assert!(matches!(d("https://www.facebook.com/marketplace/"), Decision::External(_)));
        assert!(matches!(d("https://www.facebook.com/messagesfoo"), Decision::External(_)));
        assert!(matches!(d("https://evil.example/facebook.com"), Decision::External(_)));
        assert!(matches!(d("https://facebook.com.evil.example/messages"), Decision::External(_)));
        assert!(matches!(d("javascript:alert(1)"), Decision::Block));
        assert!(matches!(d("file:///C:/Windows"), Decision::Block));
        match d("https://l.facebook.com/l.php?u=https%3A%2F%2Fexample.com%2Fa&h=x") {
            Decision::External(u) => assert_eq!(u.as_str(), "https://example.com/a"),
            other => panic!("{other:?}"),
        }
        match d("https://l.facebook.com/l.php?u=javascript%3Aalert(1)") {
            Decision::External(u) => assert_eq!(u.host_str(), Some("l.facebook.com")),
            other => panic!("{other:?}"),
        }
    }
}
