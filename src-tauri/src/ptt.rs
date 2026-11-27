//! Global push-to-talk, and the mute and deafen hotkeys.
//!
//! A web page sees keys only while it has focus. Here a thread asks Windows
//! for the state of the keys (GetAsyncKeyState) every 10 ms and tells the
//! page: a `ptt` event while the push-to-talk key goes down and up (true,
//! false), a `hotkey` event ("mute" or "deafen") when one of those is pushed.
//! Unlike a registered hotkey this does not swallow the key: it still types
//! in other apps, as in Discord. While alo's own window has focus the page
//! handles the keys itself (and knows when a text field has focus), so the
//! thread stays quiet then.
//!
//! The page names a key as `KeyboardEvent.code` (`KeyV`, `F13`,
//! `ShiftLeft`...), which is the physical key. Keys whose virtual-key code
//! depends on the keyboard layout go through their scan code, so `KeyQ` is
//! the same physical key on a Turkish or Azerbaijani layout. A hotkey adds
//! the modifiers before it: `Control+Shift+KeyM` (alo-sk's util/hotkey.js).
//! Push-to-talk may also be a mouse button: `Mouse3` (the wheel), `Mouse4`
//! and `Mouse5` (the side buttons).

use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};

#[derive(Default)]
pub struct Ptt {
    vk: Mutex<Option<u16>>,
    mute: Mutex<Option<Combo>>,
    deafen: Mutex<Option<Combo>>,
}

/// A key and exactly these modifiers, either side.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Combo {
    vk: u16,
    control: bool,
    alt: bool,
    shift: bool,
    meta: bool,
}

const VK_SHIFT: u16 = 0x10;
const VK_CONTROL: u16 = 0x11;
const VK_MENU: u16 = 0x12;
const VK_LWIN: u16 = 0x5B;
const VK_RWIN: u16 = 0x5C;

impl Combo {
    fn parse(text: &str) -> Option<Combo> {
        let mut parts: Vec<&str> = text.split('+').collect();
        let vk = vk_for_code(parts.pop()?)?;
        let mut combo = Combo { vk, control: false, alt: false, shift: false, meta: false };
        for part in parts {
            match part {
                "Control" => combo.control = true,
                "Alt" => combo.alt = true,
                "Shift" => combo.shift = true,
                "Meta" => combo.meta = true,
                _ => return None,
            }
        }
        Some(combo)
    }

    fn is_down(&self, down: impl Fn(u16) -> bool) -> bool {
        down(self.vk)
            && down(VK_CONTROL) == self.control
            && down(VK_MENU) == self.alt
            && down(VK_SHIFT) == self.shift
            && (down(VK_LWIN) || down(VK_RWIN)) == self.meta
    }
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

/// The page calls this whenever the mute or deafen hotkey changes; `None` for
/// no key. Returns false when one of them is a key this app cannot watch.
#[tauri::command]
pub fn set_hotkeys(state: tauri::State<'_, Arc<Ptt>>, mute: Option<String>, deafen: Option<String>) -> bool {
    let mute_combo = mute.as_deref().and_then(Combo::parse);
    let deafen_combo = deafen.as_deref().and_then(Combo::parse);
    *state.mute.lock().unwrap() = mute_combo;
    *state.deafen.lock().unwrap() = deafen_combo;
    (mute.is_none() || mute_combo.is_some()) && (deafen.is_none() || deafen_combo.is_some())
}

pub fn start(app: AppHandle, ptt: Arc<Ptt>) {
    std::thread::Builder::new()
        .name("ptt".into())
        .spawn(move || {
            let mut down = false;
            // Whether the page was told "down" by us, so it also hears "up".
            let mut told = false;
            // The hotkeys fire once when pushed, not again while held.
            let mut mute_down = false;
            let mut deafen_down = false;
            let focused = || {
                app.get_webview_window("main")
                    .and_then(|w| w.is_focused().ok())
                    .unwrap_or(false)
            };
            loop {
                let vk = *ptt.vk.lock().unwrap();
                let mute = *ptt.mute.lock().unwrap();
                let deafen = *ptt.deafen.lock().unwrap();

                let now = vk.is_some_and(is_down);
                if now != down {
                    down = now;
                    if down {
                        if !focused() {
                            let _ = app.emit_to("main", "ptt", true);
                            told = true;
                        }
                    } else if told {
                        let _ = app.emit_to("main", "ptt", false);
                        told = false;
                    }
                }

                for (combo, was, name) in [(mute, &mut mute_down, "mute"), (deafen, &mut deafen_down, "deafen")] {
                    let now = combo.is_some_and(|c| c.is_down(is_down));
                    if now && !*was && !focused() {
                        let _ = app.emit_to("main", "hotkey", name);
                    }
                    *was = now;
                }

                let watching = vk.is_some() || mute.is_some() || deafen.is_some();
                std::thread::sleep(Duration::from_millis(if watching { 10 } else { 250 }));
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
        // Mouse buttons for push-to-talk (alo-sk's util/format.js, mouseCode):
        // the wheel and the two side buttons. GetAsyncKeyState knows them too.
        "Mouse3" => 0x04, "Mouse4" => 0x05, "Mouse5" => 0x06,
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
    use super::{vk_for_code, Combo, VK_CONTROL, VK_LWIN, VK_SHIFT};

    #[test]
    fn combos() {
        let combo = Combo::parse("Control+Shift+F9").unwrap();
        assert_eq!(combo, Combo { vk: 0x78, control: true, alt: false, shift: true, meta: false });
        assert_eq!(Combo::parse("F9").map(|c| c.control), Some(false));
        assert_eq!(Combo::parse("Hyper+F9"), None);
        assert_eq!(Combo::parse("Control+Nope"), None);
        assert_eq!(Combo::parse(""), None);

        // Exactly the modifiers: Ctrl+Shift+F9 is not Ctrl+F9, nor Win+Ctrl+Shift+F9.
        let held = |keys: &'static [u16]| move |vk: u16| keys.contains(&vk);
        assert!(combo.is_down(held(&[VK_CONTROL, VK_SHIFT, 0x78])));
        assert!(!combo.is_down(held(&[VK_CONTROL, 0x78])));
        assert!(!combo.is_down(held(&[VK_CONTROL, VK_SHIFT, VK_LWIN, 0x78])));
        assert!(!combo.is_down(held(&[VK_CONTROL, VK_SHIFT])));
    }

    #[test]
    fn fixed_keys() {
        assert_eq!(vk_for_code("F1"), Some(0x70));
        assert_eq!(vk_for_code("F24"), Some(0x87));
        assert_eq!(vk_for_code("F25"), None);
        assert_eq!(vk_for_code("Numpad5"), Some(0x65));
        assert_eq!(vk_for_code("ShiftLeft"), Some(0xA0));
        assert_eq!(vk_for_code("Mouse3"), Some(0x04));
        assert_eq!(vk_for_code("Mouse4"), Some(0x05));
        assert_eq!(vk_for_code("Mouse5"), Some(0x06));
        assert_eq!(vk_for_code("Mouse1"), None);
        assert_eq!(vk_for_code("Nope"), None);
    }

    #[cfg(windows)]
    #[test]
    fn digits_by_scan_code() {
        // Digits are the same virtual keys on every common layout.
        assert_eq!(vk_for_code("Digit1"), Some(0x31));
    }
}
