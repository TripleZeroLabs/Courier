//! Builds the declutter initialization script from inject/declutter.{css,js},
//! plus a dev-only "dump page structure" helper for finding selectors.

const CSS: &str = include_str!("../inject/declutter.css");
const JS: &str = include_str!("../inject/declutter.js");

pub fn init_script(cfg: &crate::config::Config) -> String {
    let css = serde_json::to_string(CSS).expect("string serializes");
    let sidebar = serde_json::to_string(&cfg.sidebar).expect("string serializes");
    JS.replace("__CSS__", &css)
        .replace("__ENABLED__", if cfg.declutter { "true" } else { "false" })
        .replace("__SIDEBAR__", &sidebar)
        .replace("__THEME__", &theme_json(&cfg.theme))
}

/// `{ "css": ..., "mode": ... }` as a JS literal (JSON is valid JS).
pub fn theme_json(theme: &crate::theme::Theme) -> String {
    serde_json::json!({ "css": theme.css(), "mode": theme.mode }).to_string()
}

/// Dev builds only: writes an outline of the page's landmarks (tag, role,
/// data-pagelet, size/position, and aria-labels of top-level landmarks only)
/// to %LOCALAPPDATA%\dev.personal.courier\structure-dump.txt.
/// It deliberately skips everything inside chats, conversations and lists, and
/// never reads text content, so no messages or contact names are included.
#[cfg(debug_assertions)]
pub fn dump_structure(app: &tauri::AppHandle) {
    use tauri::Manager;
    const DUMP_JS: &str = r#"(function () {
  var LANDMARKS = ["banner","navigation","main","complementary","region","contentinfo","dialog","search","tablist","toolbar"];
  var LABEL_OK = ["banner","navigation","complementary","search","tablist","toolbar","contentinfo"];
  var out = [];
  var body = getComputedStyle(document.body);
  out.push("--header-height on body: " + (body.getPropertyValue("--header-height") || "(unset)"));
  out.push("viewport: " + innerWidth + "x" + innerHeight);
  var tm = /^\s*\((\d{1,6})\)/.exec(document.title);
  out.push("title count prefix: " + (tm ? "(" + tm[1] + ")" : "none"));
  var dr = document.querySelector("[data-courier-drawer]");
  var mn = document.querySelector('[role="main"]');
  function box(e) { var r = e.getBoundingClientRect(); return Math.round(r.x) + "," + Math.round(r.y) + " " + Math.round(r.width) + "x" + Math.round(r.height); }
  out.push("sidebar mode: " + document.documentElement.getAttribute("data-courier-sidebar") +
    "; drawer: " + (dr ? box(dr) + " position=" + getComputedStyle(dr).position + " transform=" + getComputedStyle(dr).transform +
      " host=" + getComputedStyle(dr.parentElement).display +
      " margin=" + getComputedStyle(dr).margin +
      " visibility=" + getComputedStyle(dr).visibility + " opacity=" + getComputedStyle(dr).opacity +
      " z=" + getComputedStyle(dr).zIndex +
      " at-left-edge=" + (function () {
        var at = document.elementFromPoint(3, Math.round(innerHeight / 2));
        if (!at) return "nothing";
        if (dr.contains(at)) return "drawer";
        return at.tagName.toLowerCase() + (at.getAttribute("role") ? "[role=" + at.getAttribute("role") + "]" : "") +
          (mn && mn.contains(at) ? " inside main" : "");
      })() : "not found") +
    "; main: " + (mn ? box(mn) : "not found"));
  var SKIP = ["grid","list","listbox","row","feed","dialog","article","textbox","log","region"];
  // Outside the top bar/navigation, only labels that look like counters or
  // section names are recorded (never contact or conversation names).
  // Short, word-only labels: no digits (they can carry message text, e.g.
  // the pinned-message banner) and nothing long.
  var COUNTER_LABEL = /^(?=.{1,40}$)[^\d]*\b(unread|notifications?|messenger|chats?|inbox|requests?|archive|marketplace|communit\w*)\b[^\d]*$/i;
  var CONTROL = /^(a|button)$/i;
  var CONTROL_ROLES = ["button","link","tab","menuitem"];
  function walk(el, depth, chrome) {
    if (out.length > 600) return;
    var role = el.getAttribute("role");
    // Never look inside conversations, chat lists or dialogs.
    if (SKIP.indexOf(role) >= 0 || el.isContentEditable ||
        /^\/messages\/(t|e2ee)\//.test(el.getAttribute("href") || "")) {
      out.push("  ".repeat(depth) + "<" + el.tagName.toLowerCase() + " role=" + (role || "-") + "> (contents skipped)");
      return;
    }
    // Buttons/links in the top bar and navigation: record their UI labels
    // (e.g. "Notifications", "Messenger") and any purely numeric badge text.
    if (CONTROL.test(el.tagName) || CONTROL_ROLES.indexOf(role) >= 0) {
      var cl = el.getAttribute("aria-label");
      if (cl && !chrome && !COUNTER_LABEL.test(cl)) cl = "";
      // A numeric badge: any descendant whose whole text is a small number.
      var digits = "";
      var leaves = el.querySelectorAll("span, div");
      for (var k = 0; k < leaves.length && k < 40 && !digits; k++) {
        var tx = (leaves[k].textContent || "").trim();
        if (/^\d{1,3}\+?$/.test(tx)) digits = tx;
      }
      if (cl || digits) {
        out.push("  ".repeat(depth) + "[control" + (role ? " role=" + role : "") +
          (cl ? ' aria-label="' + cl.slice(0, 60) + '"' : "") +
          (digits ? " badge=" + digits : "") +
          (el.getAttribute("href") ? " href=" + el.getAttribute("href").split("?")[0].slice(0, 40) : "") + "]");
      }
    }
    var pagelet = el.getAttribute("data-pagelet");
    var interesting = (role && LANDMARKS.indexOf(role) >= 0) || pagelet;
    var nextDepth = depth;
    if (interesting) {
      var r = el.getBoundingClientRect();
      var line = "  ".repeat(depth) + "<" + el.tagName.toLowerCase();
      if (role) line += " role=" + role;
      if (pagelet) line += " data-pagelet=" + pagelet;
      var label = el.getAttribute("aria-label");
      if (label && role && LABEL_OK.indexOf(role) >= 0) line += ' aria-label="' + label.slice(0, 40) + '"';
      line += "> " + Math.round(r.x) + "," + Math.round(r.y) + " " + Math.round(r.width) + "x" + Math.round(r.height);
      if (r.width === 0 || r.height === 0) line += " (not visible)";
      out.push(line);
      nextDepth = depth + 1;
    }
    var childChrome = chrome || role === "banner" || role === "navigation";
    for (var c = el.firstElementChild; c; c = c.nextElementSibling) walk(c, nextDepth, childChrome);
  }
  walk(document.body, 0, false);
  // Facebook's theme variables (names and values only), for theming.
  var cs = getComputedStyle(document.documentElement), vars = [];
  for (var v = 0; v < cs.length; v++) {
    if (cs[v].indexOf("--") === 0) vars.push(cs[v] + ": " + cs.getPropertyValue(cs[v]).trim().slice(0, 60));
  }
  vars.sort();
  out.push("");
  out.push("html classes: " + document.documentElement.className.split(/\s+/).filter(function (c) { return c.indexOf("__fb") === 0; }).join(" "));
  out.push("css variables on <html> (" + vars.length + "):");
  out = out.concat(vars.slice(0, 600));
  return out.join("\n");
})()"#;

    let Some(w) = app.get_webview_window("main") else { return };
    let Ok(dir) = app.path().app_local_data_dir() else { return };
    let file = dir.join("structure-dump.txt");
    let _ = w.eval_with_callback(DUMP_JS, move |json| {
        let text = serde_json::from_str::<String>(&json).unwrap_or(json);
        match std::fs::write(&file, text) {
            Ok(()) => eprintln!("[declutter] structure written to {}", file.display()),
            Err(e) => eprintln!("[declutter] could not write structure dump: {e}"),
        }
    });
}

/// Dev builds only: write the structure dump 1.5 s after the last resize.
#[cfg(debug_assertions)]
pub fn dump_after_resize(app: &tauri::AppHandle) {
    use std::sync::atomic::{AtomicU64, Ordering};
    static GEN: AtomicU64 = AtomicU64::new(0);
    let generation = GEN.fetch_add(1, Ordering::SeqCst) + 1;
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(1500));
        if GEN.load(Ordering::SeqCst) == generation {
            dump_structure(&app);
        }
    });
}

/// Dev builds only: a security self-test. From the Facebook page, try to
/// call Courier's settings command; it must be denied. Logs the outcome.
#[cfg(debug_assertions)]
pub fn ipc_probe(app: &tauri::AppHandle) {
    use tauri::Manager;
    let Some(w) = app.get_webview_window("main") else { return };
    let _ = w.eval(
        "window.__courierIpcProbe = 'pending';\
         try { window.__TAURI_INTERNALS__.invoke('get_settings').then(\
           function () { window.__courierIpcProbe = 'ALLOWED (this is a bug)'; },\
           function (e) { window.__courierIpcProbe = 'denied: ' + String(e).slice(0, 120); }); }\
         catch (e) { window.__courierIpcProbe = 'no IPC: ' + String(e).slice(0, 120); }",
    );
    std::thread::sleep(std::time::Duration::from_secs(3));
    let _ = w.eval_with_callback("String(window.__courierIpcProbe)", |r| {
        eprintln!("[security] facebook page calling get_settings: {r}");
    });

    // And the settings window must be able to load its settings. This opens
    // the window for a few seconds, so only with COURIER_SELFTEST=1.
    if std::env::var("COURIER_SELFTEST").as_deref() != Ok("1") {
        return;
    }
    crate::settings::open(app);
    std::thread::sleep(std::time::Duration::from_secs(4));
    if let Some(s) = app.get_webview_window(crate::settings::LABEL) {
        let s2 = s.clone();
        let _ = s.eval_with_callback(
            "document.querySelectorAll('#colors .token').length + ' color controls, ' + \
             document.querySelectorAll('#sizes .token').length + ' size controls, status: \"' + \
             document.querySelector('#status').textContent + '\"'",
            move |r| {
                eprintln!("[security] settings window loaded: {r}");
                let _ = s2.close();
            },
        );
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn script_has_no_placeholders() {
        let s = super::init_script(&crate::config::Config::default());
        for p in ["__CSS__", "__ENABLED__", "__SIDEBAR__", "__THEME__"] {
            assert!(!s.contains(p), "{p} left in script");
        }
        assert!(s.contains(", true, \"drawer\", {\"css\":\"\",\"mode\":\"facebook\"});"));
        assert!(s.contains("})(\"/*"));
    }
}
