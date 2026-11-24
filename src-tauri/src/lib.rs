//! The alo desktop app: the live site (alo.devl0pr.com) in a window, plus what
//! a browser tab cannot do: push-to-talk while another app has focus, a tray
//! icon, start with Windows, `alo://invite/CODE` links, and updates of itself.

mod prefs;
mod ptt;
mod update;
#[cfg(windows)]
mod visibility;

use std::sync::Arc;

use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Url, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt as _};
use tauri_plugin_deep_link::DeepLinkExt;
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_window_state::StateFlags;

/// The site the window shows. Keep in step with capabilities/remote.json.
const ORIGIN: &str = "https://alo.devl0pr.com";
/// Pages that open inside the window. Any other link opens in the default
/// browser, the feelm.org sign-in too: Google refuses to sign in inside an
/// app's window. The browser leaves the code at alo-api and the page in the
/// window takes it from there (alo-sk's account.svelte.js); the browser's
/// "Open alo" button (`alo://open`) only brings this window to the front.
const INSIDE: &[&str] = &["alo.devl0pr.com"];
/// Given by autostart: start in the tray, without the window.
const MINIMIZED: &str = "--minimized";

pub fn run() {
    tauri::Builder::default()
        // First: a second start (or a clicked alo:// link) goes to this one.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show(app)))
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec![MINIMIZED]),
        ))
        // Size and place, never "hidden": quitting from the tray must not
        // start the next session invisible.
        .plugin(
            tauri_plugin_window_state::Builder::new()
                .with_state_flags(StateFlags::all() & !StateFlags::VISIBLE)
                .build(),
        )
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(Arc::new(ptt::Ptt::default()))
        .manage(update::Pending::default())
        .invoke_handler(tauri::generate_handler![
            ptt::set_ptt_key,
            ptt::set_hotkeys,
            prefs::get_hardware_acceleration,
            prefs::set_hardware_acceleration,
            prefs::restart_app
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            let minimized = std::env::args().any(|arg| arg == MINIMIZED);
            let prefs = prefs::load(&handle);
            app.manage(prefs::Running(prefs));
            window(&handle, !minimized, prefs)?;
            tray(&handle)?;

            // alo:// links: the installer registers the scheme; this also
            // covers a build started from cargo.
            #[cfg(any(windows, target_os = "linux"))]
            let _ = app.deep_link().register_all();
            if let Ok(Some(urls)) = app.deep_link().get_current() {
                open_links(&handle, &urls);
            }
            let links = handle.clone();
            app.deep_link().on_open_url(move |event| open_links(&links, &event.urls()));

            let state = app.state::<Arc<ptt::Ptt>>().inner().clone();
            ptt::start(handle.clone(), state);
            #[cfg(windows)]
            visibility::start(handle.clone());
            update::start(handle);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running alo");
}

fn window(app: &AppHandle, visible: bool, prefs: prefs::Prefs) -> tauri::Result<()> {
    let outside = app.clone();
    let window = WebviewWindowBuilder::new(app, "main", WebviewUrl::External(ORIGIN.parse().unwrap()))
        .title("alo")
        .inner_size(1280.0, 800.0)
        .min_inner_size(940.0, 560.0)
        .visible(visible)
        .additional_browser_args(&browser_args(prefs.hardware_acceleration))
        .on_navigation(move |url| {
            if inside(url) {
                return true;
            }
            let _ = outside.opener().open_url(url.as_str(), None::<&str>);
            false
        })
        .build()?;
    #[cfg(windows)]
    allow_media(&window);
    window
        // The close button hides to the tray; "Quit" in the tray ends alo.
        .on_window_event({
            let app = app.clone();
            move |event| {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.hide();
                    }
                }
            }
        });
    Ok(())
}

/// The microphone and the camera for alo itself, without WebView2's "Allow?"
/// prompt, as in Discord. Any other page still gets WebView2's own prompt.
/// Windows' privacy settings for the microphone and camera still apply.
#[cfg(windows)]
fn allow_media(window: &tauri::WebviewWindow) {
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        COREWEBVIEW2_PERMISSION_KIND, COREWEBVIEW2_PERMISSION_KIND_CAMERA,
        COREWEBVIEW2_PERMISSION_KIND_MICROPHONE, COREWEBVIEW2_PERMISSION_STATE_ALLOW,
    };
    use webview2_com::PermissionRequestedEventHandler;

    let _ = window.with_webview(|webview| unsafe {
        let Ok(core) = webview.controller().CoreWebView2() else {
            return;
        };
        let handler = PermissionRequestedEventHandler::create(Box::new(|_, args| {
            let Some(args) = args else { return Ok(()) };
            let mut kind = COREWEBVIEW2_PERMISSION_KIND::default();
            args.PermissionKind(&mut kind)?;
            let mut uri = windows_core::PWSTR::null();
            args.Uri(&mut uri)?;
            let uri = webview2_com::take_pwstr(uri);
            let media = kind == COREWEBVIEW2_PERMISSION_KIND_MICROPHONE
                || kind == COREWEBVIEW2_PERMISSION_KIND_CAMERA;
            if media && is_alo(&uri) {
                args.SetState(COREWEBVIEW2_PERMISSION_STATE_ALLOW)?;
            }
            Ok(())
        }));
        let mut token = 0i64;
        let _ = core.add_PermissionRequested(&handler, &mut token);
    });
}

/// A page of alo itself (https, the exact host).
fn is_alo(uri: &str) -> bool {
    uri.parse::<Url>()
        .is_ok_and(|url| url.scheme() == "https" && url.host_str() == Some("alo.devl0pr.com"))
}

/// Tauri's own defaults, plus: timers keep their pace and the page keeps its
/// priority while the window is in the tray or behind a game, so voice and
/// push-to-talk never lag. The page itself does count as hidden then
/// (visibility.rs): it stops drawing and pauses the videos, and a game keeps
/// the graphics card. Without hardware
/// acceleration WebView2 draws on the processor. A debug build also opens the
/// DevTools protocol on 127.0.0.1:9229 for testing.
fn browser_args(hardware_acceleration: bool) -> String {
    let mut args = String::from(concat!(
        "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection ",
        "--disable-background-timer-throttling ",
        "--disable-renderer-backgrounding ",
        "--autoplay-policy=no-user-gesture-required"
    ));
    if !hardware_acceleration {
        args.push_str(" --disable-gpu");
    }
    if cfg!(debug_assertions) {
        args.push_str(" --remote-debugging-port=9229");
    }
    args
}

fn inside(url: &Url) -> bool {
    match url.scheme() {
        "https" => url.host_str().is_some_and(|host| INSIDE.contains(&host)),
        "about" | "blob" | "data" | "tauri" => true,
        "http" => url.host_str() == Some("tauri.localhost"),
        _ => false,
    }
}

fn show(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// `alo://open` brings the window to the front.
/// `alo://invite/CODE` opens that invite in the window.
fn open_links(app: &AppHandle, urls: &[Url]) {
    for url in urls {
        if url.scheme() == "alo" && url.host_str() == Some("open") {
            show(app);
        } else if let Some(target) = link_target(url) {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.navigate(target);
                show(app);
            }
        }
    }
}

fn token(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && value.chars().all(|c| c.is_ascii_alphanumeric() || "-_.~".contains(c))
}

fn link_target(url: &Url) -> Option<Url> {
    if url.scheme() != "alo" {
        return None;
    }
    match (url.host_str()?, url.path().trim_matches('/')) {
        ("invite", code) if token(code, 64) => format!("{ORIGIN}/invite/{code}").parse().ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(link: &str) -> Option<String> {
        link_target(&link.parse().unwrap()).map(|u| u.to_string())
    }

    #[test]
    fn links() {
        assert_eq!(target("alo://invite/AbC-12").as_deref(), Some("https://alo.devl0pr.com/invite/AbC-12"));
        assert_eq!(target("alo://open"), None);
        assert_eq!(target("alo://auth/feelm?code=x1y2&state=desktop-ab12"), None);
        assert_eq!(target("alo://invite/..%2Fadmin"), None);
        assert_eq!(target("alo://admin/users"), None);
        assert_eq!(target("https://alo.devl0pr.com/invite/x"), None);
    }

    #[test]
    fn media_only_for_alo() {
        assert!(is_alo("https://alo.devl0pr.com/channels/1/2"));
        assert!(!is_alo("http://alo.devl0pr.com/"));
        assert!(!is_alo("https://alo.devl0pr.com.evil.example/"));
        assert!(!is_alo("https://feelm.org/"));
    }

    #[test]
    fn inside_the_window() {
        assert!(inside(&"https://alo.devl0pr.com/channels/@me".parse().unwrap()));
        assert!(!inside(&"https://feelm.org/connect/alo?state=x".parse().unwrap()));
        assert!(!inside(&"http://alo.devl0pr.com/".parse().unwrap()));
    }
}

fn tray(app: &AppHandle) -> tauri::Result<()> {
    // The version, until an update waits (update.rs).
    let version = format!("alo {}", app.package_info().version);
    let line = MenuItem::with_id(app, "update", version, false, None::<&str>)?;
    app.manage(update::TrayLine(line.clone()));
    let open = MenuItem::with_id(app, "open", "Open alo", true, None::<&str>)?;
    let reload = MenuItem::with_id(app, "reload", "Reload", true, None::<&str>)?;
    let autostart = CheckMenuItem::with_id(
        app,
        "autostart",
        "Start with Windows",
        true,
        app.autolaunch().is_enabled().unwrap_or(false),
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, "quit", "Quit alo", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let top = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&line, &top, &open, &reload, &autostart, &separator, &quit])?;

    let toggle = autostart.clone();
    let mut tray = TrayIconBuilder::with_id("alo")
        .tooltip("alo")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "open" => show(app),
            // Back to alo's start, whatever the window shows.
            "reload" => {
                if let (Some(window), Ok(start)) = (app.get_webview_window("main"), ORIGIN.parse()) {
                    let _ = window.navigate(start);
                    show(app);
                }
            }
            "autostart" => {
                let launch = app.autolaunch();
                let on = launch.is_enabled().unwrap_or(false);
                let _ = if on { launch.disable() } else { launch.enable() };
                let _ = toggle.set_checked(launch.is_enabled().unwrap_or(!on));
            }
            "quit" => app.exit(0),
            "update" => update::install_pending(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}
