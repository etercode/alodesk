# alodesk

The alo desktop apps. Each is a window that shows the live site,
https://alo.devl0pr.com, so every alo-sk deploy reaches the apps at once; a
new app version is needed only when the shell changes, and the installed apps
fetch it themselves (see "Updates").

- **Windows**: a Tauri 2 app (`src-tauri/`), 2 MB.
- **Linux**: an Electron app (`electron/`, "Linux app" below). On Linux, Tauri
  would use WebKitGTK, which cannot run alo's voice (WebCodecs Opus) and video
  reliably; Electron brings Chrome's engine. macOS comes later, the same way.

## Download

Get them from the [latest release](https://github.com/etercode/alodesk/releases/latest):

- **Windows**: `alo-setup.exe`. It installs for your Windows user only (no
  admin rights) and puts alo in the Start menu. It is not code-signed yet, so
  Windows SmartScreen may say "Windows protected your PC" the first time.
  Click **More info**, then **Run anyway**.
- **Pardus, Debian, Ubuntu, Mint**: `alo-linux-amd64.deb`. Open it (the
  package installer asks for your password), or `sudo apt install ./alo-linux-amd64.deb`.
- **Other Linux**: `alo-linux-x86_64.AppImage`. Allow it to run as a program
  and open it (it needs FUSE 2: `libfuse2`).

## What the Windows app adds to the website

| feature | how |
| --- | --- |
| push-to-talk while another app has focus | `src-tauri/src/ptt.rs` watches the one key (GetAsyncKeyState) and sends a `ptt` event; alo-sk listens in `src/lib/state/native.js`. The key still types in other apps. |
| mute and deafen keys while another app has focus | the same thread watches the two hotkeys from alo's Voice settings (`Control+Shift+KeyM`: exactly these modifiers, either side) and sends a `hotkey` event, `mute` or `deafen`, once for each push. From 0.1.2. |
| microphone and camera without a prompt | for alo.devl0pr.com only (`allow_media` in `lib.rs`); other pages get WebView2's prompt. Windows' privacy settings still apply. |
| tray icon | close hides to the tray; the menu shows the version (or "Restart to update") and has Open, Reload, Start with Windows and Quit |
| updates of itself | from the latest GitHub release, signed; see "Updates" |
| start with Windows | a check box in the tray menu, off at first; starts hidden in the tray |
| `alo://invite/CODE`, `alo://open` | opens the invite in the window, or brings the window to the front (the installer registers `alo://`) |
| sign-in with feelm | opens in the default browser, because Google refuses to sign in inside an app's window. The browser leaves the one-time code at alo-api, and the waiting app takes it (alo-api's README, "Sign-in with feelm"). |
| notifications | `tauri-plugin-notification` stands in for the browser's Notification |
| links | alo opens in the window, every other link in the default browser |
| one copy | a second start shows the running one |
| remembered size and place | `tauri-plugin-window-state` |
| quiet behind a game | WebView2 does not notice by itself that nobody sees the window. `src-tauri/src/visibility.rs` looks twice a second: hidden in the tray, minimized, or under an app that fills its whole screen (a game) makes the page "hidden" (the controller's IsVisible), so it stops drawing and pauses videos, and the game keeps the graphics card. Voice, timers and push-to-talk go on (the page keeps its priority and its timers' pace). From 0.1.3. |
| hardware acceleration | on at first; the site's Settings → Appearance turns it off (`--disable-gpu`, then the window draws on the processor) when games lose FPS. Kept in `prefs.json` in the app's config folder (`src-tauri/src/prefs.rs`), used at the next start; the site offers "Restart alo". From 0.1.3. |

Only `https://alo.devl0pr.com/*` may call the app (`capabilities/remote.json`):
the push-to-talk key, the mute and deafen keys, hardware acceleration and a
restart, events and notifications. The site's address is in
`src-tauri/src/lib.rs` (`ORIGIN`, `INSIDE`, `is_alo`) and in that capability.

## Linux app (`electron/`)

The same things, through Electron (`src/main.js`); the site sees
`window.aloDesktop` (`src/preload.js`) instead of `window.__TAURI__`, and
alo-sk's `native.js` speaks to either.

| feature | how |
| --- | --- |
| push-to-talk, mute and deafen keys while another app has focus | `src/keys.js` with uiohook (libuiohook) hears the keys of the X11 session without taking them. **Under Wayland** (GNOME on Wayland) an app cannot hear other apps' keys; there the keys work while alo has focus. Pardus Xfce is X11. |
| microphone, camera, notifications without a prompt | for alo only (`setPermissionRequestHandler`) |
| screen share | our own picker of screens and windows (`src/picker.html`) on X11; the system's dialog under Wayland |
| tray icon | close hides to it; Open, Reload, Start with the computer (`~/.config/autostart/alo.desktop`), Quit. GNOME shows tray icons only with the AppIndicator extension; starting alo again always shows the window. |
| `alo://` links, one copy, links in the browser, feelm sign-in, size and place | as on Windows; the .deb registers `alo://` |
| quiet when hidden | in the tray or minimized the page counts as hidden (Chromium's own; `backgroundThrottling` on), so it stops drawing and pauses videos; switches keep its timers and priority for voice. From 0.1.3. |
| hardware acceleration | as on Windows (`app.disableHardwareAcceleration()`), kept in `prefs.json` in the app's user data folder |
| updates of itself | `src/updates.js`, electron-updater, from `latest-linux.yml` in the latest release. The AppImage installs a newer version at start; the .deb needs the computer's password, so it only offers "Restart to update" in the tray. Files are checked by their SHA-512. |

Build and run it on Linux (WSL works, with WSLg):

```bash
cd electron
npm ci
npm test                                    # key names
ALO_ORIGIN=http://localhost:5178 npm start  # against a local alo; DevTools on 9230
npm run dist                                # dist/alo_X.Y.Z_amd64.deb and .AppImage
```

Releases build it on GitHub (`.github/workflows/linux.yml`): publishing a
release starts it, and it adds `alo_X.Y.Z_amd64.deb`, `alo_X.Y.Z_x86_64.AppImage`,
`latest-linux.yml`, and the same two files as `alo-linux-amd64.deb` and
`alo-linux-x86_64.AppImage` for the site's download button. For an existing
release: `gh workflow run linux.yml -f tag=vX.Y.Z`.

## Build (Windows)

Needs Rust (MSVC toolchain), the Visual Studio C++ Build Tools and WebView2
(Windows 11 has it). No Node.

```bash
cargo install tauri-cli --version "^2" --locked
cargo tauri icon app-icon.svg      # only after changing the icon
cargo tauri dev                    # run it
cargo tauri build                  # installer in src-tauri/target/release/bundle/nsis
cd src-tauri && cargo test         # push-to-talk keys, links, permissions
```

A debug build (`cargo tauri dev`) also opens the DevTools protocol on
127.0.0.1:9229 for testing; a release build does not.

## Updates (Windows)

From 0.1.2 the app updates itself (`src-tauri/src/update.rs`, with
`tauri-plugin-updater`). It reads `latest.json` from the latest GitHub
release:

- **at start**, a newer version is downloaded and installed at once (a small
  progress bar, no questions), and alo starts again. Nobody is in a call yet.
- **while it runs**, it looks every six hours and only downloads. The first
  line of the tray menu becomes "Restart to update to X", with a notification,
  so an update never ends a call. Otherwise the next start installs it.

Every download is checked against the public key in `tauri.conf.json`
(`plugins.updater.pubkey`); a file with a wrong signature is refused. Apps
older than 0.1.2 have no updater: their people install 0.1.2 by hand once.

## Release

Raise `version` in `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` and
`electron/package.json` (the script checks they agree), then:

```powershell
.\release.ps1 -Notes "What changed, in a sentence"
```

It builds, signs the installer with the private key, and publishes release
`vX.Y.Z` with three files: `alo_X.Y.Z_x64-setup.exe`, `alo-setup.exe` (the
"Download the desktop app" button on alo.devl0pr.com links to
`releases/latest/download/alo-setup.exe`) and `latest.json` (what installed
apps check). `-BuildOnly` stops before the release. The published release
starts the Linux build on GitHub, which adds the Linux files a few minutes later.

The private key is `%USERPROFILE%\.tauri\alo-updater.key` (no password), and
never goes into this repository. **Keep a copy of it somewhere safe:**
without it no update can reach the installed apps again, and a new key means
every person installs by hand once more. Anybody with it could sign an
update, so it stays private. `TAURI_SIGNING_PRIVATE_KEY` (a path or the key
itself) and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` override where it is read.

## License

MIT
