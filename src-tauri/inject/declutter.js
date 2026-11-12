/* =========================================================================
   Courier declutter script
   -------------------------------------------------------------------------
   Runs in every facebook.com page before Facebook's own scripts (Tauri
   initialization script). It:
     1. injects declutter.css as <style id="courier-declutter">, and
        re-adds it if Facebook ever removes it;
     2. runs RULES (below) for things CSS can't select, e.g. text matching;
     3. exposes window.__courierSetDeclutter(on) for the Ctrl+Shift+D toggle
        and window.__courierSetSidebar(mode) for Ctrl+Shift+S, and
        window.__courierSetTheme(theme) for the settings window;
     4. shows a loading screen until Messenger has rendered.

   It does not read, store or send any message content. It only adds a
   style element and toggles a data attribute on elements matched by RULES.

   The four arguments on the last line are placeholders that Courier fills
   in at launch (the stylesheet text, whether declutter is on, the chat
   list mode, "drawer" or "pinned", and the theme: { css, mode }).
   ========================================================================= */
(function (CSS, ENABLED, SIDEBAR, THEME) {
  "use strict";
  // Main frame only, Facebook only (WebView2 also runs this in iframes).
  if (window.top !== window) return;
  if (!/(^|\.)facebook\.com$/.test(location.hostname)) return;

  /* ---------------------------------------------------------------------
     Attention. When Courier isn't the active window (open behind or beside
     another app, minimized, or in the tray), Courier calls
     __courierSetAttention(false). Facebook's scripts then read the page as
     hidden and unfocused (document.visibilityState, document.hidden,
     document.hasFocus()), so Facebook doesn't mark the open conversation as
     read while you aren't looking. The page keeps drawing normally.
     Installed before Facebook's scripts run; the real browser values are
     used whenever Courier is active.
     --------------------------------------------------------------------- */
  var attentive = true;
  (function () {
    var proto = Document.prototype;
    var vis = Object.getOwnPropertyDescriptor(proto, "visibilityState");
    var hid = Object.getOwnPropertyDescriptor(proto, "hidden");
    var hasFocus = proto.hasFocus;
    try {
      if (vis && vis.get) Object.defineProperty(proto, "visibilityState", {
        configurable: true, enumerable: vis.enumerable,
        get: function () { return attentive ? vis.get.call(this) : "hidden"; }
      });
      if (hid && hid.get) Object.defineProperty(proto, "hidden", {
        configurable: true, enumerable: hid.enumerable,
        get: function () { return attentive ? hid.get.call(this) : true; }
      });
      proto.hasFocus = function () { return attentive && hasFocus.call(this); };
    } catch (e) { /* leave the browser's values alone */ }
  })();
  window.__courierSetAttention = function (on) {
    on = !!on;
    if (on === attentive) return;
    attentive = on;
    // Let Facebook's listeners re-read the state.
    document.dispatchEvent(new Event("visibilitychange"));
  };

  var STYLE_ID = "courier-declutter";
  var HIDDEN_ATTR = "data-courier-hidden";
  var enabled = !!ENABLED;

  /* ---------------------------------------------------------------------
     RULES: each function returns a list of elements to hide.
     Add a rule only when CSS can't express it. Keep each one cheap: it
     runs at most ~4 times a second while the page changes.
     Example (English UI only):
       function () {
         return Array.from(document.querySelectorAll('[role="complementary"]'))
           .filter(function (el) { return /Sponsored/.test(el.getAttribute("aria-label") || ""); });
       },
     --------------------------------------------------------------------- */
  var RULES = [];

  /* ---------------------------------------------------------------------
     Chat list drawer (see declutter.css, block 3). Marks the column that
     holds the chat list and its row container. Structure only: the
     navigation landmark that contains the chat grid, walked up until its
     parent also contains the conversation (role="main").
     --------------------------------------------------------------------- */
  var DRAWER_ATTR = "data-courier-drawer";
  var DRAWER_HOST_ATTR = "data-courier-drawer-host";

  function markDrawer() {
    var nav = document.querySelector('[role="navigation"]:has([role="grid"])');
    var main = document.querySelector('[role="main"]');
    var col = null;
    // In narrow windows Facebook shows one pane at a time. While it shows
    // only the chat list (the conversation area is collapsed), the list IS
    // the page, so don't turn it into a drawer.
    var conversationShown = main && main.getBoundingClientRect().width >= 120;
    if (nav && main && !nav.contains(main) && conversationShown) {
      col = nav;
      while (col.parentElement && !col.parentElement.contains(main)) col = col.parentElement;
      if (!col.parentElement || col.parentElement === document.body) col = null;
    }
    var old = document.querySelectorAll("[" + DRAWER_ATTR + "], [" + DRAWER_HOST_ATTR + "]");
    for (var i = 0; i < old.length; i++) {
      if (old[i] !== col && old[i] !== (col && col.parentElement)) {
        old[i].removeAttribute(DRAWER_ATTR);
        old[i].removeAttribute(DRAWER_HOST_ATTR);
      }
    }
    if (col) {
      if (!col.hasAttribute(DRAWER_ATTR)) col.setAttribute(DRAWER_ATTR, "");
      if (!col.parentElement.hasAttribute(DRAWER_HOST_ATTR)) col.parentElement.setAttribute(DRAWER_HOST_ATTR, "");
    }
  }

  /* ---------------------------------------------------------------------
     Theme (built natively from the settings; see src/theme.rs): a
     stylesheet of CSS-variable overrides plus custom CSS, and an optional
     forced light/dark mode. Facebook switches modes with the classes
     __fb-light-mode / __fb-dark-mode on <html> (the one place Courier
     relies on a Facebook class name; it has been stable for years). Not
     affected by Ctrl+Shift+D.
     --------------------------------------------------------------------- */
  var THEME_ID = "courier-theme";
  var LIGHT = "__fb-light-mode", DARK = "__fb-dark-mode";
  var themeMode = "facebook";
  var fbMode = null; // Facebook's own choice, restored when mode is "facebook"
  var modeObserver = null;

  function applyMode() {
    var cl = document.documentElement.classList;
    var current = cl.contains(DARK) ? DARK : cl.contains(LIGHT) ? LIGHT : null;
    var want = themeMode === "light" ? LIGHT : themeMode === "dark" ? DARK : fbMode;
    if (!want || current === want) return;
    cl.remove(LIGHT, DARK);
    cl.add(want);
  }

  function setTheme(theme) {
    theme = theme || {};
    var s = document.getElementById(THEME_ID);
    if (!s) {
      s = document.createElement("style");
      s.id = THEME_ID;
    }
    s.textContent = theme.css || "";
    // Last in <head>/<html>, so it wins over Facebook's sheets at equal priority.
    (document.head || document.documentElement).appendChild(s);

    themeMode = theme.mode === "light" || theme.mode === "dark" ? theme.mode : "facebook";
    var cl = document.documentElement.classList;
    if (fbMode === null) fbMode = cl.contains(DARK) ? DARK : cl.contains(LIGHT) ? LIGHT : null;
    applyMode();
    if (!modeObserver) {
      // Facebook may set its class later (or change it from its own
      // settings): remember its choice, then re-apply ours.
      modeObserver = new MutationObserver(function () {
        var cl2 = document.documentElement.classList;
        if (themeMode === "facebook") {
          fbMode = cl2.contains(DARK) ? DARK : cl2.contains(LIGHT) ? LIGHT : fbMode;
        } else {
          modeObserver.disconnect();
          applyMode();
          modeObserver.observe(document.documentElement, { attributes: true, attributeFilter: ["class"] });
        }
      });
      modeObserver.observe(document.documentElement, { attributes: true, attributeFilter: ["class"] });
    }
  }

  function ensureTheme() {
    var s = document.getElementById(THEME_ID);
    if (s && s.nextElementSibling && s.parentNode === document.head) {
      // Keep it last if Facebook appended stylesheets after it.
      document.head.appendChild(s);
    } else if (!s && currentTheme) {
      setTheme(currentTheme);
    }
  }
  var currentTheme = THEME;

  /* ---------------------------------------------------------------------
     Facebook-notification tracker, used to keep toasts Messenger-only.
     Facebook's (hidden) top bar has a notifications button whose count goes
     up when Facebook activity arrives (likes, comments...), but not for
     messages. Once a second this reads that count and records when it last
     went up in window.__courierFbNotifIncreasedAt. When a web notification
     arrives, Courier (notify.rs) checks how recently that happened: just
     now means the notification is Facebook activity, and it isn't shown.
     Reads one number; same element as the badge logic in unread.rs.
     --------------------------------------------------------------------- */
  function fbNotifCount() {
    var els = document.querySelectorAll('[role="banner"] [role="button"][aria-label], [role="banner"] a[aria-label]');
    var n = 0;
    for (var i = 0; i < els.length; i++) {
      var label = els[i].getAttribute("aria-label") || "";
      if (/messenger|chat/i.test(label)) continue;
      var c = 0, lm = /(\d{1,6})/.exec(label);
      if (lm) c = parseInt(lm[1], 10);
      else {
        var leaves = els[i].querySelectorAll("span, div");
        for (var k = 0; k < leaves.length && k < 40; k++) {
          var tm = /^(\d{1,6})\+?$/.exec((leaves[k].textContent || "").trim());
          if (tm) { c = parseInt(tm[1], 10); break; }
        }
      }
      if (c > n) n = c;
    }
    return n;
  }
  var lastFbNotif = null;
  function trackFbNotifications() {
    var n = fbNotifCount();
    if (lastFbNotif !== null && n > lastFbNotif) window.__courierFbNotifIncreasedAt = Date.now();
    lastFbNotif = n;
  }

  function setSidebar(mode) {
    document.documentElement.setAttribute("data-courier-sidebar", mode === "pinned" ? "pinned" : "drawer");
  }

  function ensureStyle() {
    var s = document.getElementById(STYLE_ID);
    if (!s) {
      s = document.createElement("style");
      s.id = STYLE_ID;
      s.textContent = CSS;
      (document.head || document.documentElement).appendChild(s);
    }
    s.disabled = !enabled;
  }

  function applyRules() {
    for (var i = 0; i < RULES.length; i++) {
      var els;
      try { els = RULES[i]() || []; } catch (e) { continue; }
      for (var j = 0; j < els.length; j++) {
        if (!els[j].hasAttribute(HIDDEN_ATTR)) els[j].setAttribute(HIDDEN_ATTR, "");
      }
    }
  }

  // Throttle: however many DOM changes happen, work runs at most every 250 ms.
  var scheduled = false;
  function schedule() {
    if (scheduled) return;
    scheduled = true;
    setTimeout(function () {
      scheduled = false;
      ensureStyle();
      ensureTheme();
      markDrawer();
      if (enabled) applyRules();
    }, 250);
  }

  function start() {
    setSidebar(SIDEBAR);
    setTheme(currentTheme);
    ensureStyle();
    markDrawer();
    applyRules();
    // The drawer column can be re-rendered at any time, so watch the whole
    // tree (child additions/removals only; the work above is throttled).
    new MutationObserver(schedule).observe(document.documentElement, { childList: true, subtree: true });
    setInterval(trackFbNotifications, 1000);
    // Facebook re-renders the layout when the window width changes.
    window.addEventListener("resize", schedule);
  }

  window.__courierSetSidebar = function (mode) {
    setSidebar(mode);
  };

  window.__courierSetTheme = function (theme) {
    currentTheme = theme;
    setTheme(theme);
  };

  /* ---------------------------------------------------------------------
     Loading screen: covers the page (Courier's bubble with animated dots)
     from the first moment the document exists until Messenger has rendered,
     so Facebook's own loading states never flash. It is a pseudo-element on
     <html>, so it needs no DOM nodes of its own and Facebook's scripts never
     see it. Shown on every full page load (startup, Ctrl+R), not on
     in-app navigation between chats.
     Ready = Messenger's main area and its chat links exist, or 2.5 s after
     the page's load event (login pages etc.), or 12 s at most.
     --------------------------------------------------------------------- */
  var SPLASH_ATTR = "data-courier-loading";
  var SPLASH_SVG = "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 100 100'>" +
    "<defs><linearGradient id='g' gradientUnits='userSpaceOnUse' x1='40' y1='0' x2='70' y2='100'>" +
    "<stop offset='0' stop-color='#00B2FF'/><stop offset='1' stop-color='#005CE6'/></linearGradient></defs>" +
    "<g fill='url(#g)'><circle cx='50' cy='47' r='45'/><polygon points='15,66 40,88 10,98'/></g>" +
    [30, 50, 70].map(function (x, i) {
      return "<circle cx='" + x + "' cy='47' r='6.5' fill='#fff' opacity='.35'>" +
        "<animate attributeName='opacity' values='.35;1;.35' dur='1.2s' begin='" + (i * 0.2) +
        "s' repeatCount='indefinite'/></circle>";
    }).join("") + "</svg>";
  var SPLASH_CSS =
    "html[" + SPLASH_ATTR + "]::after{content:'';position:fixed;inset:0;z-index:2147483647;" +
    "background:#fff url(\"data:image/svg+xml," + encodeURIComponent(SPLASH_SVG) + "\") center/72px 72px no-repeat;" +
    "transition:opacity .2s ease-out;pointer-events:auto}" +
    "html[" + SPLASH_ATTR + "='out']::after{opacity:0;pointer-events:none}" +
    "@media (prefers-color-scheme: dark){html[" + SPLASH_ATTR + "]::after{background-color:#18191a}}";

  function startSplash() {
    var root = document.documentElement;
    if (!/^\/(messages|$)/.test(location.pathname)) return; // only the Messenger UI
    root.setAttribute(SPLASH_ATTR, "");
    var s = document.createElement("style");
    s.id = "courier-splash";
    s.textContent = SPLASH_CSS;
    root.appendChild(s);

    var started = Date.now(), loadedAt = 0;
    window.addEventListener("load", function () { loadedAt = Date.now(); });
    var timer = setInterval(function () {
      var now = Date.now();
      var ready = (document.querySelector('[role="main"]') &&
                   document.querySelector('a[href*="/messages/"], [role="grid"]')) ||
                  (loadedAt && now - loadedAt > 2500) ||
                  now - started > 12000;
      if (!ready) return;
      clearInterval(timer);
      root.setAttribute(SPLASH_ATTR, "out");
      setTimeout(function () {
        root.removeAttribute(SPLASH_ATTR);
        if (s.parentNode) s.parentNode.removeChild(s);
      }, 250);
    }, 100);
  }

  window.__courierSetDeclutter = function (on) {
    enabled = !!on;
    ensureStyle();
    if (enabled) applyRules();
  };

  function boot() {
    try { startSplash(); } catch (e) { /* never block the page */ }
    start();
  }
  if (document.documentElement) {
    boot();
  } else {
    new MutationObserver(function (_, obs) {
      if (document.documentElement) { obs.disconnect(); boot(); }
    }).observe(document, { childList: true });
  }
})(__CSS__, __ENABLED__, __SIDEBAR__, __THEME__);
