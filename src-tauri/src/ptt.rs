//! Global push-to-talk.
//!
//! A web page sees keys only while it has focus. Here a thread asks Windows
//! for the state of the one push-to-talk key (GetAsyncKeyState) every 10 ms
//! and tells the page with a `ptt` event (true = down, false = up). Unlike a
//! registered hotkey this does not swallow the key: it still types in other
//! apps, as in Discord. While alo's own window has focus the page handles the
//! key itself (and knows when a text field has focus), so the thread stays
//! quiet then.
//!
//! The page names the key as `KeyboardEvent.code` (`KeyV`, `F13`,
//! `ShiftLeft`...), which is the physical key. Keys whose virtual-key code
//! depends on the keyboard layout go through their scan code, so `KeyQ` is
//! the same physical key on a Turkish or Azerbaijani layout.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};

#[derive(Default)]
pub struct Ptt {
    vk: Mutex<Option<u16>>,
}

/// The page calls this whenever the push-to-talk key or the mode changes.
/// `None` (voice activity, or no key) stops watching. Returns false for a key
/// this app cannot watch.
#[tauri::command]
pub fn set_ptt_key(state: tauri::State<'_, Arc<Ptt>>, code: Option<String>) -> bool {
    let vk = code.as_deref().and_then(vk_for_code);
    *state.vk.lock().unwrap() = vk;
    code.is_none() || vk.is_some()
}

pub fn start(app: AppHandle, ptt: Arc<Ptt>) {
    std::thread::Builder::new()
        .name("ptt".into())
        .spawn(move || {
            let mut down = false;
            // Whether the page was told "down" by us, so it also hears "up".
            let mut told = false;
            loop {
                let vk = *ptt.vk.lock().unwrap();
                let now = vk.is_some_and(is_down);
                if now != down {
                    down = now;
                    if down {
                        let focused = app
                            .get_webview_window("main")
                            .and_then(|w| w.is_focused().ok())
                            .unwrap_or(false);
                        if !focused {
                            let _ = app.emit_to("main", "ptt", true);
                            told = true;
                        }
                    } else if told {
                        let _ = app.emit_to("main", "ptt", false);
                        told = false;
                    }
                }
                std::thread::sleep(Duration::from_millis(if vk.is_some() { 10 } else { 250 }));
            }
        })
        .expect("failed to start the push-to-talk thread");
}

#[cfg(windows)]
fn is_down(vk: u16) -> bool {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
    (unsafe { GetAsyncKeyState(vk as i32) } as u16) & 0x8000 != 0
}

#[cfg(not(windows))]
fn is_down(_vk: u16) -> bool {
    false
}

#[cfg(windows)]
fn from_scan_code(scan: u32) -> Option<u16> {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{MapVirtualKeyW, MAPVK_VSC_TO_VK_EX};
    match unsafe { MapVirtualKeyW(scan, MAPVK_VSC_TO_VK_EX) } {
        0 => None,
        vk => Some(vk as u16),
    }
}

#[cfg(not(windows))]
fn from_scan_code(_scan: u32) -> Option<u16> {
    None
}

/// `KeyboardEvent.code` to a Windows virtual-key code.
fn vk_for_code(code: &str) -> Option<u16> {
    // Keys that move with the layout: by scan code (set 1).
    let scan = match code {
        "Digit1" => 0x02, "Digit2" => 0x03, "Digit3" => 0x04, "Digit4" => 0x05, "Digit5" => 0x06,
        "Digit6" => 0x07, "Digit7" => 0x08, "Digit8" => 0x09, "Digit9" => 0x0A, "Digit0" => 0x0B,
        "Minus" => 0x0C, "Equal" => 0x0D,
        "KeyQ" => 0x10, "KeyW" => 0x11, "KeyE" => 0x12, "KeyR" => 0x13, "KeyT" => 0x14,
        "KeyY" => 0x15, "KeyU" => 0x16, "KeyI" => 0x17, "KeyO" => 0x18, "KeyP" => 0x19,
        "BracketLeft" => 0x1A, "BracketRight" => 0x1B,
        "KeyA" => 0x1E, "KeyS" => 0x1F, "KeyD" => 0x20, "KeyF" => 0x21, "KeyG" => 0x22,
        "KeyH" => 0x23, "KeyJ" => 0x24, "KeyK" => 0x25, "KeyL" => 0x26,
        "Semicolon" => 0x27, "Quote" => 0x28, "Backquote" => 0x29, "Backslash" => 0x2B,
        "KeyZ" => 0x2C, "KeyX" => 0x2D, "KeyC" => 0x2E, "KeyV" => 0x2F, "KeyB" => 0x30,
        "KeyN" => 0x31, "KeyM" => 0x32,
        "Comma" => 0x33, "Period" => 0x34, "Slash" => 0x35, "IntlBackslash" => 0x56,
        _ => 0,
    };
    if scan != 0 {
        return from_scan_code(scan);
    }

    // Keys that do not move: their virtual-key code.
    if let Some(n) = code.strip_prefix('F').and_then(|n| n.parse::<u16>().ok()) {
        return (1..=24).contains(&n).then(|| 0x6F + n);
    }
    if let Some(n) = code.strip_prefix("Numpad").and_then(|n| n.parse::<u16>().ok()) {
        return (n <= 9).then(|| 0x60 + n);
    }
    let vk = match code {
        "Space" => 0x20, "Tab" => 0x09, "CapsLock" => 0x14, "Backspace" => 0x08, "Enter" => 0x0D,
        "ShiftLeft" => 0xA0, "ShiftRight" => 0xA1,
        "ControlLeft" => 0xA2, "ControlRight" => 0xA3,
        "AltLeft" => 0xA4, "AltRight" => 0xA5,
        "MetaLeft" => 0x5B, "MetaRight" => 0x5C, "ContextMenu" => 0x5D,
        "Insert" => 0x2D, "Delete" => 0x2E, "Home" => 0x24, "End" => 0x23,
        "PageUp" => 0x21, "PageDown" => 0x22,
        "ArrowLeft" => 0x25, "ArrowUp" => 0x26, "ArrowRight" => 0x27, "ArrowDown" => 0x28,
        "Pause" => 0x13, "ScrollLock" => 0x91, "NumLock" => 0x90, "PrintScreen" => 0x2C,
        "NumpadMultiply" => 0x6A, "NumpadAdd" => 0x6B, "NumpadSubtract" => 0x6D,
        "NumpadDecimal" => 0x6E, "NumpadDivide" => 0x6F, "NumpadEnter" => 0x0D,
        _ => return None,
    };
    Some(vk)
}

#[cfg(test)]
mod tests {
    use super::vk_for_code;

    #[test]
    fn fixed_keys() {
        assert_eq!(vk_for_code("F1"), Some(0x70));
        assert_eq!(vk_for_code("F24"), Some(0x87));
        assert_eq!(vk_for_code("F25"), None);
        assert_eq!(vk_for_code("Numpad5"), Some(0x65));
        assert_eq!(vk_for_code("ShiftLeft"), Some(0xA0));
        assert_eq!(vk_for_code("Nope"), None);
    }

    #[cfg(windows)]
    #[test]
    fn digits_by_scan_code() {
        // Digits are the same virtual keys on every common layout.
        assert_eq!(vk_for_code("Digit1"), Some(0x31));
    }
}
