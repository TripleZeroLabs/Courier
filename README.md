<p align="center">
  <img src="src-tauri/icons/128x128.png" alt="Courier" width="96" height="96">
</p>

<h1 align="center">Courier</h1>

<p align="center">
  <b>Messenger lost its desktop app. So we built one.</b><br>
  A fast, private, open-source Windows app for Facebook Messenger.
</p>

<p align="center">
  <a href="LICENSE">MIT licensed</a> ·
  Windows 10/11 ·
  ~1.3 MB installer ·
  Built with <a href="https://tauri.app">Tauri</a> + Rust
</p>

---

On **December 15, 2025**, Meta shut down the Messenger desktop apps for
Windows and Mac and sent everyone back to the website. Your chats ended up
in a browser tab squeezed in next to forty others, with the Facebook feed one
click away and notifications at the mercy of your browser.

**Courier brings the desktop app back** and makes it better than the old one.
It's a real window with a taskbar badge, native notifications and a tray
icon, plus features the official app never had. It's completely open source.

<!-- Screenshot goes here. Use a test account or blur names: never publish real conversations. -->

## Why you'll like it

- 💬 **Just Messenger.** Facebook's top bar, feed and other distractions are
  hidden. It looks and feels like a dedicated chat app.
- 🔔 **Notifications for messages only.** Native Windows toasts for new
  messages. Likes, comments and other Facebook activity stay quiet. Click a
  toast to jump straight into that chat.
- 🔴 **A badge that means something.** The taskbar badge counts unread
  *chats*, not Facebook notifications. It shows whenever you're not looking
  at Courier and clears when you are.
- 👀 **Unread stays unread.** Got a message while Courier was in the
  background? It stays unread until you actually come back. No more "seen"
  on things you never saw.
- 🗂️ **Chat list drawer.** Collapse the chat list to a thin strip; hover the
  edge to slide it out. More room for the conversation, even in a
  side-by-side snapped window.
- 🎨 **Make it yours.** Light/dark mode regardless of your Facebook setting,
  colour pickers for accents and bubbles, bubble shape and spacing, corner
  radii, and a custom CSS box for everything else.
- 🪟 **A proper desktop citizen.** Lives in the tray, opens once (no
  duplicate windows), remembers its size and position, optional Start with
  Windows and global hotkey, zoom that sticks.
- ⚡ **Tiny and quick.** Uses Windows' built-in WebView2 instead of shipping
  a whole browser. The installer is about 1.3 MB.

## Private by design

Courier is a better window around Facebook's own web page. That's all.

- **The Facebook page can't touch your computer.** It's granted zero access
  to Courier's internals: no files, no shell, no settings. Dev builds verify
  this on every launch.
- **No telemetry, no tracking, no accounts.** Courier never phones home.
- **Never sees your password.** You log in on Facebook's own page.
- **Never reads or stores your messages.** Notification text goes straight
  to the Windows toast and nowhere else.
- **Your own browser profile.** Separate from Edge or Chrome.

Every bit of it is in this repo for you to check. The full security model is
in the [guide](docs/GUIDE.md#security-and-privacy).

## Get it

**Install:** download `Courier_x.y.z_x64-setup.exe` from
[Releases](../../releases) and run it. It installs just for you; no admin
rights needed. Windows SmartScreen will warn the first time because the
installer isn't code-signed yet: click **More info → Run anyway**.

**Build it yourself:** you need Rust, the Visual Studio C++ Build Tools and
Node.js ([details](docs/GUIDE.md#prerequisites)).

```powershell
npm install
npm run tauri dev     # run with DevTools
npm run build         # installer lands in ./installer/
```

## Handy shortcuts

| Keys | |
|---|---|
| `Ctrl+,` | Settings |
| `Ctrl+Shift+S` | Toggle the chat list drawer |
| `Ctrl+=` / `Ctrl+-` / `Ctrl+0` | Zoom |
| `Ctrl+R` | Reload |
| Close button | Hide to tray (Quit from the tray menu) |

## Help make it great

Facebook changes its web page often, and most fixes are a one-line selector
or CSS variable. Contributions are very welcome: bug reports, fixes, themes,
ideas. Start with the [guide](docs/GUIDE.md), which explains how everything
fits together and the few house rules (no automation, no reading messages,
no telemetry, and the Facebook page never gets access to your computer).

## The fine print

Courier is an independent, unofficial project. It is **not affiliated with,
endorsed by, or sponsored by Meta Platforms, Inc.** "Facebook" and
"Messenger" are trademarks of Meta, named here only to say what Courier
works with. Courier's name and icon are its own.

Courier shows Meta's own web page and restyles it on your computer, much like
a browser extension. Your use of Facebook is still subject to Meta's terms.
Facebook can change its site at any time, which may break features until
Courier catches up. Provided as is, without warranty, under the
[MIT License](LICENSE).
