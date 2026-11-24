//! Tells WebView2 when nobody can see alo's window, so that the page counts as
//! hidden: it stops drawing and pauses the videos (alo-sk's video.svelte.js),
//! and a game keeps the graphics card. Voice, timers and push-to-talk go on.
//!
//! WebView2 does not find this out by itself: the page stayed "visible" while
//! the window was minimized, in the tray or behind a full-screen game. So a
//! thread looks twice a second and sets the controller's IsVisible. Hidden
//! means: the window is hidden (tray) or minimized, or the app in front fills
//! the whole screen alo's window is on (a game, a full-screen video).

use std::time::Duration;

use tauri::{AppHandle, Manager};

pub fn start(app: AppHandle) {
    std::thread::Builder::new()
        .name("visibility".into())
        .spawn(move || {
            let mut shown = true;
            loop {
                std::thread::sleep(Duration::from_millis(500));
                let Some(window) = app.get_webview_window("main") else { continue };
                let Ok(hwnd) = window.hwnd() else { continue };
                let now = !hidden(hwnd.0 as isize);
                if now != shown {
                    shown = now;
                    let _ = window.with_webview(move |webview| unsafe {
                        let _ = webview.controller().SetIsVisible(now);
                    });
                }
            }
        })
        .expect("failed to start the visibility thread");
}

#[cfg(windows)]
fn hidden(hwnd: isize) -> bool {
    use windows_sys::Win32::Foundation::{HWND, RECT};
    use windows_sys::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONULL};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetAncestor, GetClassNameW, GetForegroundWindow, GetWindowRect, IsIconic, IsWindowVisible, GA_ROOTOWNER,
    };

    let hwnd = hwnd as HWND;
    unsafe {
        if IsWindowVisible(hwnd) == 0 || IsIconic(hwnd) != 0 {
            return true;
        }
        let front = GetForegroundWindow();
        if front.is_null() || front == hwnd || GetAncestor(front, GA_ROOTOWNER) == hwnd {
            return false;
        }
        // The desktop and the taskbar fill the screen too, but hide nothing.
        let mut class = [0u16; 32];
        let len = GetClassNameW(front, class.as_mut_ptr(), class.len() as i32).max(0) as usize;
        let class = String::from_utf16_lossy(&class[..len]);
        if matches!(class.as_str(), "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd") {
            return false;
        }
        // In front on the same screen, and filling all of it.
        let screen = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONULL);
        if screen.is_null() || MonitorFromWindow(front, MONITOR_DEFAULTTONULL) != screen {
            return false;
        }
        let mut info: MONITORINFO = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        let mut rect: RECT = std::mem::zeroed();
        if GetMonitorInfoW(screen, &mut info) == 0 || GetWindowRect(front, &mut rect) == 0 {
            return false;
        }
        let m = info.rcMonitor;
        rect.left <= m.left && rect.top <= m.top && rect.right >= m.right && rect.bottom >= m.bottom
    }
}

#[cfg(not(windows))]
fn hidden(_hwnd: isize) -> bool {
    false
}
