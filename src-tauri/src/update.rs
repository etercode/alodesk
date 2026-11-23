//! The app updates itself from the latest GitHub release (README, "Release").
//!
//! At start it installs a newer version at once: nobody is in a call yet, and
//! the installer (passive: a progress bar, no questions) starts alo again.
//! While it runs it looks every six hours and only downloads; then the tray
//! menu offers "Restart to update", so an update never ends a call. The
//! updater checks the download against the public key in tauri.conf.json.
//!
//! This is the app around the site only: changes to alo.devl0pr.com reach the
//! window on the next load without any of this.

use std::sync::Mutex;
use std::time::Duration;

use tauri::menu::MenuItem;
use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_updater::{Update, UpdaterExt};

const EVERY: Duration = Duration::from_secs(6 * 60 * 60);

/// A downloaded update that waits for "Restart to update".
#[derive(Default)]
pub struct Pending(Mutex<Option<(Update, Vec<u8>)>>);

/// The tray menu's first line: the version, or "Restart to update".
pub struct TrayLine(pub MenuItem<Wry>);

pub fn start(app: AppHandle) {
    // A build from `cargo tauri dev` is not installed: nothing to update.
    if cfg!(debug_assertions) {
        return;
    }
    std::thread::Builder::new()
        .name("update".into())
        .spawn(move || {
            if let Some((update, bytes)) = tauri::async_runtime::block_on(fetch(&app)) {
                // On Windows this ends the app; the installer starts it again.
                if let Err(error) = update.install(&bytes) {
                    eprintln!("update: {error}");
                }
            }
            loop {
                std::thread::sleep(EVERY);
                if app.state::<Pending>().0.lock().unwrap().is_some() {
                    continue;
                }
                if let Some((update, bytes)) = tauri::async_runtime::block_on(fetch(&app)) {
                    let version = update.version.clone();
                    *app.state::<Pending>().0.lock().unwrap() = Some((update, bytes));
                    let line = &app.state::<TrayLine>().0;
                    let _ = line.set_text(format!("Restart to update to {version}"));
                    let _ = line.set_enabled(true);
                    let _ = app
                        .notification()
                        .builder()
                        .title(format!("alo {version} is ready"))
                        .body("Right-click alo's tray icon and choose \"Restart to update\" when you are not in a call.")
                        .show();
                }
            }
        })
        .expect("failed to start the update thread");
}

/// A newer version, downloaded and checked, or None.
async fn fetch(app: &AppHandle) -> Option<(Update, Vec<u8>)> {
    let update = match app.updater().ok()?.check().await {
        Ok(update) => update?,
        Err(error) => {
            eprintln!("update check: {error}");
            return None;
        }
    };
    match update.download(|_, _| {}, || {}).await {
        Ok(bytes) => Some((update, bytes)),
        Err(error) => {
            eprintln!("update download: {error}");
            None
        }
    }
}

/// "Restart to update" in the tray menu.
pub fn install_pending(app: &AppHandle) {
    let pending = app.state::<Pending>().0.lock().unwrap().take();
    if let Some((update, bytes)) = pending {
        if let Err(error) = update.install(&bytes) {
            eprintln!("update: {error}");
        }
    }
}
