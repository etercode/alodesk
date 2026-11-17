# alodesk

The alo desktop app for Windows. It is a Tauri 2 window that shows the live
site, https://alo.devl0pr.com, so every alo-sk deploy reaches the app at
once; a new app version is needed only when this shell changes. Linux and
macOS people use the website.

## Download

Get `alo_<version>_x64-setup.exe` from the
[latest release](https://github.com/etercode/alodesk/releases/latest) and run
it. It installs for your Windows user only (no admin rights) and puts alo in
the Start menu.

The installer is not code-signed yet, so Windows SmartScreen may say "Windows
protected your PC" the first time. Click **More info**, then **Run anyway**.

## What it adds to the website

| feature | how |
| --- | --- |
| push-to-talk while another app has focus | `src-tauri/src/ptt.rs` watches the one key (GetAsyncKeyState) and sends a `ptt` event; alo-sk listens in `src/lib/state/native.js`. The key still types in other apps. |
| microphone and camera without a prompt | for alo.devl0pr.com only (`allow_media` in `lib.rs`); other pages get WebView2's prompt. Windows' privacy settings still apply. |
| tray icon | close hides to the tray; the menu has Open, Reload, Start with Windows and Quit |
| start with Windows | a check box in the tray menu, off at first; starts hidden in the tray |
| `alo://invite/CODE`, `alo://open` | opens the invite in the window, or brings the window to the front (the installer registers `alo://`) |
| sign-in with feelm | opens in the default browser, because Google refuses to sign in inside an app's window. The browser leaves the one-time code at alo-api, and the waiting app takes it (alo-api's README, "Sign-in with feelm"). |
| notifications | `tauri-plugin-notification` stands in for the browser's Notification |
| links | alo opens in the window, every other link in the default browser |
| one copy | a second start shows the running one |
| remembered size and place | `tauri-plugin-window-state` |

Only `https://alo.devl0pr.com/*` may call the app (`capabilities/remote.json`):
the push-to-talk key, events and notifications. The site's address is in
`src-tauri/src/lib.rs` (`ORIGIN`, `INSIDE`, `is_alo`) and in that capability.

## Build

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

## Release

Raise `version` in `src-tauri/tauri.conf.json` and `src-tauri/Cargo.toml`,
build, then publish the installer twice: under its own name, and as
`alo-setup.exe`, which the "Download the desktop app" button on
alo.devl0pr.com links to (`releases/latest/download/alo-setup.exe`):

```bash
v=0.1.1
cp src-tauri/target/release/bundle/nsis/alo_${v}_x64-setup.exe /tmp/alo-setup.exe
gh release create v$v src-tauri/target/release/bundle/nsis/alo_${v}_x64-setup.exe /tmp/alo-setup.exe --title "alo $v"
```

## License

MIT
