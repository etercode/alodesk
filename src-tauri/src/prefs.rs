//! The app's own settings, which the site cannot keep: they are needed before
//! the window exists. For now only whether the window draws with the graphics
//! card (WebView2's GPU process). Off, a game keeps the card to itself; the
//! window then draws and decodes video on the processor instead.
//!
//! Kept in prefs.json in the app's config folder. A change takes effect at the
//! next start: the site offers "Restart alo" (alo-sk's settings).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Prefs {
    pub hardware_acceleration: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs { hardware_acceleration: true }
    }
}

/// What this run started with, to tell the site whether a restart is due.
pub struct Running(pub Prefs);

/// For the site: the saved choice, and what this run uses.
#[derive(Serialize)]
pub struct HardwareAcceleration {
    on: bool,
    running: bool,
}

fn file(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|dir| dir.join("prefs.json"))
}

pub fn load(app: &AppHandle) -> Prefs {
    file(app)
        .and_then(|path| std::fs::read(path).ok())
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn save(app: &AppHandle, prefs: Prefs) -> Result<(), String> {
    let path = file(app).ok_or("no config folder")?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_vec_pretty(&prefs).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_hardware_acceleration(app: AppHandle, running: tauri::State<'_, Running>) -> HardwareAcceleration {
    HardwareAcceleration { on: load(&app).hardware_acceleration, running: running.0.hardware_acceleration }
}

#[tauri::command]
pub fn set_hardware_acceleration(app: AppHandle, on: bool) -> Result<(), String> {
    save(&app, Prefs { hardware_acceleration: on, ..load(&app) })
}

/// "Restart alo" after a change: the same app starts again, then this one ends.
#[tauri::command]
pub fn restart_app(app: AppHandle) {
    app.restart();
}

#[cfg(test)]
mod tests {
    use super::Prefs;

    #[test]
    fn missing_or_old_files_mean_on() {
        assert!(Prefs::default().hardware_acceleration);
        let empty: Prefs = serde_json::from_str("{}").unwrap();
        assert!(empty.hardware_acceleration);
        let off: Prefs = serde_json::from_str(r#"{"hardwareAcceleration":false}"#).unwrap();
        assert!(!off.hardware_acceleration);
    }
}
