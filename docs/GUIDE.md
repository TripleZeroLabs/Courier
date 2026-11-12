# Courier guide

Everything in detail: features, building, configuration, theming, keeping up
with Facebook's markup, and the security model. For the short version, see
the [README](../README.md).

Courier is an unofficial Windows desktop app for Facebook Messenger
(`https://www.facebook.com/messages`), built with Tauri v2 and WebView2. It
is just a better window around Meta's own web client. It does not automate
messaging, scrape conversations, or store credentials. You log in on
Facebook's own page, inside Courier's isolated browser profile. It is not
affiliated with or endorsed by Meta.

## Features

- Opens `facebook.com/messages` in its own window, with a WebView2 profile
  separate from Edge/Chrome. Your login persists across restarts.
- **Navigation guard:** Messenger, login, 2FA and call pages stay in the app.
  The Facebook home page is redirected back to `/messages`. Other Facebook
  pages and all outside links open in your default browser. Facebook's
  `l.facebook.com` link redirector is unwrapped first, so the browser opens
  the real destination.
- **Declutter:** hides Facebook's top bar so the window looks like a
  dedicated Messenger app. `Ctrl+Shift+D` toggles it until the next reload
  (it isn't saved, so Courier always starts decluttered).
- **Chat list drawer:** the chat list collapses to a thin strip at the left
  edge. Hover the strip to slide it out over the conversation; it hides again
  shortly after the mouse leaves (and stays open while you type in its search
  box). `Ctrl+Shift+S`, the tray or Settings switch back to Facebook's normal
  layout.
- **Themes:** light/dark mode independent of your Facebook setting, colour
  pickers (accent, your/their bubbles, backgrounds, text, dividers, focus
  ring…), sizes and spacing (bubble corners and padding, panel and button
  corners, borders, drawer width), and a custom CSS box. See *Themes*.
- **Settings window:** tray → **Settings…** or `Ctrl+,`. Changes apply
  immediately.
- **Loading screen:** on startup and reload, Courier's bubble with animated
  dots covers the page until Messenger has rendered.
- **Unread badge (chats only):** a taskbar overlay (1–9, or a dot for 10+)
  and an unread tray icon, shown whenever Courier isn't the active window
  (minimized, in the tray, or open behind/beside another app) and hidden
  while you're using it. The tray tooltip and a "(N) Courier" window title
  always show the count. Facebook's title count also includes Facebook
  notifications (likes, comments…), so Courier subtracts the count on
  Facebook's notifications button.
- **Messages stay unread while you're away:** when Courier isn't the active
  window, Facebook is told the page isn't being viewed, so it doesn't mark
  the open conversation as read. It does once you switch back to Courier.
- **Native Windows toasts for messages only** (can be turned off). Facebook
  sends its own activity (likes, comments…) through the same notifications;
  Courier holds each one for 2 seconds and drops it if Facebook's
  notification count just went up. Clicking a toast brings Courier forward
  and opens that chat.
- **Tray:** closing the window hides it to the tray. Left-click the tray icon
  to show/hide; right-click for Show/Hide, Reload, Auto-hide chat list,
  Start with Windows, Settings, Quit.
- Single instance (launching again focuses the existing window), and the
  window's size, position and maximized state are remembered.
- **Shortcuts** (inside the window):

  | Keys | Action |
  |---|---|
  | `Ctrl+R`, `F5` | Reload |
  | `Ctrl+=` / `Ctrl+-` / `Ctrl+0` | Zoom in / out / reset (remembered) |
  | `Ctrl+Shift+S` | Chat list: auto-hiding drawer / pinned (remembered) |
  | `Ctrl+Shift+D` | Toggle declutter (until reload) |
  | `Ctrl+,` | Settings |
  | `Esc` | Hide to tray (off by default, see Settings) |

- Optional global show/hide hotkey and Start with Windows (which launches
  hidden in the tray). Both are **off** by default.
- Voice/video calls open in their own Courier window. Mic and camera use
  WebView2's normal permission prompt. Downloads go to your Downloads folder.

## Prerequisites

- **Windows 10/11** with the **WebView2 Runtime** (preinstalled on Windows 11).
- **Rust** (stable, MSVC toolchain): <https://rustup.rs>
- **Visual Studio 2022 Build Tools** with the *Desktop development with C++*
  workload. Close Visual Studio before installing or updating it, or the
  installer refuses to run (exit code 8006).
- **Node.js** 18+ (only for the Tauri CLI).

## Build and run

```powershell
npm.cmd install            # once: installs the Tauri CLI
npm.cmd run tauri dev      # development build, with DevTools (F12)
npm.cmd run build          # release build + installer
```

(`npm.cmd` works even when PowerShell's execution policy blocks `npm.ps1`.)

The installer ends up in `installer\Courier_<version>_x64-setup.exe` at the
project root. Tauri writes it to `src-tauri\target\release\bundle\nsis\`, and
a post-build step (`tools/copy-installer.mjs`) copies it to `installer\`,
replacing older versions there. `installer\` is git-ignored. Running
`npm.cmd run tauri build` directly skips the copy.
It installs per-user (no admin rights needed). It isn't code-signed, so
Windows SmartScreen will warn the first time you run it.

Dev builds add two tray items for testing: **Test notification (dev)** and
**Dump page structure (dev)** (see *Updating the declutter selectors*). Dev
builds also print navigation decisions (host and path only) and unread counts
to the terminal, and run a security self-test 30 seconds after launch (see
*Security and privacy*). Release builds log nothing.

In dev builds, toasts are labelled "Windows PowerShell", because an
uninstalled exe has no registered app identity. The installed app shows
toasts as Courier.

## Where things live

| What | Where |
|---|---|
| Settings | `%APPDATA%\dev.personal.courier\config.json` |
| Browser profile (cookies, login, cache) | `%LOCALAPPDATA%\dev.personal.courier\webview\` |
| Window size/position | `%APPDATA%\dev.personal.courier\.window-state.json` |
| Start with Windows entry (if enabled) | `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` |

To log out completely or reset the app, quit Courier and delete the
`webview` folder.

Uninstalling removes the Start with Windows entry. It deletes the two
folders above only if you tick the uninstaller's "delete app data" box.

### config.json

The Settings window edits this file and applies changes live. You can also
edit it by hand while Courier is closed. Invalid values fall back to their
defaults.

```json
{
  "hotkey": "",
  "esc_hides_window": false,
  "declutter": true,
  "zoom": 1.0,
  "sidebar": "drawer",
  "notifications": true,
  "theme": {
    "mode": "facebook",
    "colors": { "accent": "#7d74ff" },
    "sizes": { "bubble_radius": 8 },
    "custom_css": ""
  }
}
```

- `hotkey`: global show/hide hotkey, e.g. `"Ctrl+Alt+M"`. Empty means none.
- `esc_hides_window`: `Esc` hides the window to the tray. Off by default
  because Messenger uses `Esc` to close its own dialogs.
- `declutter`: hide Facebook's top bar. `Ctrl+Shift+D` overrides it until
  the next reload without changing this file.
- `zoom`: 0.5–3.0, updated by the zoom shortcuts.
- `sidebar`: `"drawer"` (auto-hiding chat list) or `"pinned"`.
- `notifications`: Windows toasts on/off.
- `theme`: see below.

## Themes

Facebook styles nearly everything through named CSS variables
(`--surface-background`, `--chat-outgoing-message-bubble-background-color`,
`--chat-bubble-padding-horizontal`, …). Courier's theme overrides those,
which restyles the whole app without depending on Facebook's generated class
names. Everything is defined in
[`src-tauri/src/theme.rs`](../src-tauri/src/theme.rs):

- `COLOR_TOKENS` / `SIZE_TOKENS`: each Settings control, and the Facebook
  variables it sets. If Facebook renames a variable, fix it there. To find
  the current names, run a dev build: the structure dump lists every variable
  on `<html>` with its value.
- **Mode:** `facebook` follows your Facebook display setting. `light`/`dark`
  swap Facebook's own `__fb-light-mode`/`__fb-dark-mode` classes on `<html>`,
  the one place Courier relies on a Facebook class name.
- **Custom CSS** is applied last. Use Facebook's variables or ARIA roles
  (`[role="main"]`, `[role="navigation"]`) rather than class names, and
  `!important` where Facebook's styles win.

Padding and margins that Facebook doesn't expose as variables can only be
changed with custom CSS, and such rules may break when Facebook updates.
Some icons are coloured with CSS filters rather than variables, so they may
not follow a custom accent colour.

## Updating the declutter selectors

Facebook's class names are generated and change constantly, so all selectors
use ARIA roles, `data-*` attributes and page structure instead. Everything
lives in two files:

- [`src-tauri/inject/declutter.css`](../src-tauri/inject/declutter.css): the
  rules (top bar, chat list drawer). Each block says what it does and how to
  find it again when Facebook changes its markup.
- [`src-tauri/inject/declutter.js`](../src-tauri/inject/declutter.js): injects
  the CSS before Facebook's scripts run and re-adds it if removed, marks the
  drawer column, applies the theme, and shows the loading screen. It also
  holds a `RULES` list for anything CSS can't express, such as text
  matching. Its MutationObserver work is throttled to at most every 250 ms.

When something reappears or the layout breaks after a Facebook update:

1. Press `Ctrl+Shift+D` to confirm whether declutter is involved.
2. Run `npm.cmd run tauri dev`, press `F12`, and use the element picker on
   the part you want to hide. Look for a `role`, `data-pagelet` or other
   `data-*` attribute on it or a parent.
   - Or use the tray's **Dump page structure (dev)** item, which writes an
     outline of the page's landmarks to
     `%LOCALAPPDATA%\dev.personal.courier\structure-dump.txt`. It records
     tags, roles, `data-pagelet` values, sizes, the drawer's position, UI
     labels of top-bar and navigation buttons, and Facebook's CSS variables.
     Elsewhere it only records short, digit-free section labels such as
     "Chats". It never reads text, and it skips chat lists, conversations,
     regions and the message box. Dev builds also write it automatically
     30 seconds after launch and after every window resize.
3. Edit `declutter.css`. `tauri dev` rebuilds and restarts automatically.

## Security and privacy

Courier treats the Facebook page as untrusted.

**The Facebook page can call nothing in Courier.** Courier defines exactly
two IPC commands, `get_settings` and `save_settings`, and they exist only for
the local Settings window:

- `build.rs` declares both in the app manifest, so they are denied unless a
  capability grants them.
- The only capability,
  [`capabilities/settings.json`](../src-tauri/capabilities/settings.json),
  grants both to the window labelled `settings` and to local content only
  (`"local": true`; no `remote` URLs). It grants no core or plugin
  permissions.
- Each command also re-checks that the caller is the `settings` window
  showing Courier's bundled page (`settings.rs`).
- `save_settings` validates every value: known keys only, `#rrggbb` colours,
  clamped sizes and zoom, hotkey ≤64 chars, custom CSS ≤20,000 chars.

So the Facebook window has no capability at all, and no access to the
filesystem, shell, HTTP, process control, plugins, core commands or
Courier's settings. Dev builds verify this on every launch: 30 seconds in,
they make the Facebook page try to call `get_settings` and log the result,
which must be "denied" (`[security] …` in the terminal). Set
`COURIER_SELFTEST=1` to also check that the Settings window can load.

Everything else happens natively in Rust, observing WebView2 rather than
taking input from the page:

| Feature | How | What the page controls |
|---|---|---|
| Unread count | Reads the window title (`on_document_title_changed`) and takes only the leading `(N)`. When it's above 0, Rust runs a read-only snippet (`eval_with_callback`) that returns two numbers: that title count and the count on Facebook's notifications button. | Two numbers, capped at 6 digits |
| Notifications | WebView2 `NotificationReceived` event. After 2 s, Rust reads one number from the page (ms since Facebook's notification count last rose) to drop Facebook activity. | Title (≤128 chars) and body (≤512 chars), shown only in the toast. Notifications from any origin other than `https://*.facebook.com` are dropped. A page could suppress its own toasts, nothing more. |
| Attention | Window focus/minimize/hide events, checked against the real foreground window (`attention.rs`). Hidden/minimized: WebView2 `SetIsVisible(false)`. Visible but inactive: the injected script reports the page as hidden. | Nothing |
| Shortcuts | WebView2 `AcceleratorKeyPressed` event | Nothing; key presses only |
| Navigation | `on_navigation` / `on_new_window` | Which URL it tries to open. Only http(s) and mailto links reach the browser. `javascript:`, `file:` and custom protocol links are blocked. |

**Permissions:** notification permission is pre-granted to
`https://www.facebook.com`, and only in Courier's own profile. Every other
permission (microphone, camera, location, …) goes through WebView2's normal
prompt.

**Injected script:** `declutter.js` adds `<style>` elements (declutter,
theme, loading screen), sets `data-courier-*` attributes (hidden elements,
the drawer column, sidebar mode, loading state), and may swap Facebook's
light/dark class. While Courier isn't the active window it makes
`document.visibilityState`, `document.hidden` and `document.hasFocus()`
report "hidden" (so Facebook doesn't mark chats read), and once a second it
reads the number on Facebook's notifications button (for the toast filter).
It defines `window.__courierSetDeclutter`, `__courierSetSidebar`,
`__courierSetTheme` and `__courierSetAttention`, which Courier calls to
apply settings; anything on the page could call them too, but they only
restyle the page or change what the page itself sees. It reads no message
content and sends nothing anywhere.

**No telemetry, no credentials, no content logging.** Courier never sees
your password; login happens entirely in Facebook's page. Message text is
only passed to the Windows toast. Release builds don't log; dev builds log
navigation host and path and unread counts only.

**Local pages** are served with the CSP `default-src 'none'; script-src
'self'; style-src 'self'; img-src 'self'; connect-src ipc:
http://ipc.localhost`: no inline scripts or styles, nothing loaded from
outside the app.

## Project layout

```
package.json                  Tauri CLI only
dist/settings.html|css|js     the Settings window
dist/index.html               unused placeholder page
tools/make-icons.ps1          regenerates the icon set
tools/copy-installer.mjs      copies the built installer to installer/
installer/                    latest installer (created by npm run build)
src-tauri/
  tauri.conf.json             app config, CSP, bundler (NSIS, per-user)
  build.rs                    declares the two IPC commands (permission-gated)
  capabilities/settings.json  grants them to the Settings window only
  inject/declutter.css|js     declutter rules, drawer, theme, loading screen
  icons/                      app, tray, unread and badge icons
  src/main.rs                 setup, windows, plugins, permissions
  src/settings.rs             Settings window and its two commands
  src/theme.rs                theme tokens and CSS generation
  src/nav.rs                  navigation guard and popups
  src/unread.rs               title → badge/tray/title
  src/attention.rs            active/minimized/hidden → badge and page visibility
  src/notify.rs               WebView2 notifications → Windows toasts
  src/keys.rs                 in-window shortcuts
  src/tray.rs                 tray menu, show/hide, autostart toggle
  src/actions.rs              zoom, declutter, drawer, applying settings live
  src/declutter.rs            builds the injected script, dev dump, self-test
  src/config.rs               config.json
```

Icons are an original design: a blue gradient chat bubble with three white
dots (not Messenger's lightning-bolt logo, which is Meta's trademark).
Regenerate them with `powershell -ExecutionPolicy Bypass -File tools\make-icons.ps1`
and then `npx.cmd tauri icon tools\icon-source.png`.

## Contributing

Facebook changes its markup often; most fixes are a selector or a CSS
variable name. Please keep to the project's rules:

- No Facebook class names in selectors: ARIA roles, `data-*` attributes,
  structure and Facebook's CSS variables only (see *Updating the declutter
  selectors* and *Themes*).
- No new IPC for the Facebook page, no automation, no reading or storing
  message content, no telemetry.
- Never commit structure dumps, logs, config files or screenshots that show
  your conversations.
- Run `cargo test` in `src-tauri/` before sending changes.

## License

[MIT](../LICENSE). Third-party dependencies keep their own licenses (Tauri and
its plugins: MIT/Apache-2.0).
