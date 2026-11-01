//! Notifications over the game: a small window of their own, separate from Encore's main window.
//!
//! The window is transparent, always on top, lets clicks pass through to the game, never takes
//! focus, has no taskbar button, and is marked so Windows' screen capture leaves it out: it never
//! appears in recordings or Echoes (nor in screen sharing or screenshots). It stays hidden until
//! there is something to show and hides itself again when the last one fades.
//!
//! The interface decides what to show (it knows the settings, Do not disturb and what is being
//! recorded) and calls `show_notification`. This module only places the window: on the chosen
//! screen, at the chosen spot, big enough for three stacked notifications. The page inside
//! (ui/notify.html) draws them, runs their timers and stacks them.
//!
//! Games in exclusive fullscreen draw over every window, so notifications can't show there.

#![cfg_attr(not(windows), allow(dead_code, unused_imports, unused_variables))]

use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

pub const LABEL: &str = "notify";

/// Room for the card (or tab) and its glow, in logical pixels. Matches ui/notify.html.
const CARD_W: f64 = 400.0;
const TAB_W: f64 = 350.0;
const STACK_H: f64 = 300.0;
/// Gap between a card and the screen edge (cards only; edge tabs sit on the edge).
const MARGIN: f64 = 24.0;
/// The glow padding inside the window, on every side except an edge tab's screen side.
const PAD: f64 = 20.0;

/// Creates the (hidden) notification window. Called once at start-up.
pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let win = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("notify.html".into()))
        .title("Encore notifications")
        .inner_size(CARD_W, STACK_H)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false)
        // Created shown (off screen, and empty, so nothing is seen) and then hidden directly:
        // a page created hidden isn't drawn when the window is later shown without focus.
        .position(-32000.0, -32000.0)
        .visible(true)
        .build()?;
    let _ = win.set_ignore_cursor_events(true);
    #[cfg(windows)]
    if let Ok(hwnd) = win.hwnd() {
        win::prepare(hwnd.0 as _);
        win::hide(hwnd.0 as _);
    }
    Ok(())
}

/// A screen's place and size in physical pixels.
pub struct Screen {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

/// Shows one notification. `device` is the chosen screen's Windows name ("\\.\DISPLAY2"), or
/// None for the screen in use (the one holding the window in front, normally the game); a screen
/// that isn't connected falls back to that too. `item` goes to the page as it is (kind, title,
/// text, seconds).
#[tauri::command]
pub fn show_notification(app: AppHandle, device: Option<String>, pos: String, item: Value) -> Result<(), String> {
    let win = app.get_webview_window(LABEL).ok_or("The notification window isn't ready.")?;
    let monitors = app.available_monitors().unwrap_or_default();
    let chosen = device.and_then(|d| monitors.iter().find(|m| m.name().map(|n| n == &d).unwrap_or(false)));
    let screen = match chosen {
        Some(m) => Some(Screen { x: m.position().x, y: m.position().y, width: m.size().width, height: m.size().height }),
        None => {
            #[cfg(windows)]
            let s = win::screen_in_use();
            #[cfg(not(windows))]
            let s = None;
            s
        }
    };
    let screen = screen.ok_or("No screen to show the notification on.")?;
    // The screen's scale (125 %, 150 %…), so notifications are the same size on every screen.
    let scale = monitors
        .iter()
        .find(|m| m.position().x == screen.x && m.position().y == screen.y)
        .map(|m| m.scale_factor())
        .unwrap_or(1.0);
    let (x, y, w, h) = place(&screen, &pos, scale);
    let started = std::time::Instant::now();
    #[cfg(windows)]
    {
        let hwnd = win.hwnd().map_err(|e| e.to_string())?;
        win::show_at(hwnd.0 as _, x, y, w, h);
    }
    let kind = item.get("kind").and_then(|k| k.as_str()).unwrap_or("?").to_string();
    crate::hotkeys::note(&app, format!("notification {kind}: window shown at {x},{y} {w}x{h} ({pos}, scale {scale}) in {} ms", started.elapsed().as_millis()));
    win.emit_to(LABEL, "notify", serde_json::json!({ "pos": pos, "item": item })).map_err(|e| e.to_string())
}

/// Called by the page when a notification has been drawn, for the log.
#[tauri::command]
pub fn notification_drawn(app: AppHandle, kind: String, waited_ms: u64) {
    crate::hotkeys::note(&app, format!("notification {kind}: drawn {waited_ms} ms after it arrived"));
}

/// Called by the page when its last notification has faded.
#[tauri::command]
pub fn notifications_idle(app: AppHandle) {
    crate::hotkeys::note(&app, "notifications: window hidden".into());
    if let Some(win) = app.get_webview_window(LABEL) {
        #[cfg(windows)]
        if let Ok(hwnd) = win.hwnd() {
            win::hide(hwnd.0 as _);
        }
        #[cfg(not(windows))]
        let _ = win.hide();
    }
}

/// The window's place and size in physical pixels for a spot on a screen.
fn place(s: &Screen, pos: &str, scale: f64) -> (i32, i32, i32, i32) {
    let tab = pos == "ml" || pos == "mr";
    let w = ((if tab { TAB_W } else { CARD_W }) * scale).round() as i32;
    let h = (STACK_H * scale).round() as i32;
    let edge = ((MARGIN - PAD) * scale).round() as i32; // the glow padding counts towards the gap
    let (sx, sy, sw, sh) = (s.x, s.y, s.width as i32, s.height as i32);
    let left = sx + edge;
    let right = sx + sw - w - edge;
    let centre_x = sx + (sw - w) / 2;
    let top = sy + edge;
    let bottom = sy + sh - h - edge;
    let middle = sy + (sh - h) / 2;
    match pos {
        "tl" => (left, top, w, h),
        "tc" => (centre_x, top, w, h),
        "bl" => (left, bottom, w, h),
        "bc" => (centre_x, bottom, w, h),
        "br" => (right, bottom, w, h),
        "ml" => (sx, middle, w, h),
        "mr" => (sx + sw - w, middle, w, h),
        _ => (right, top, w, h), // "tr"
    }
}

#[cfg(windows)]
mod win {
    use super::Screen;
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTOPRIMARY};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowLongPtrW, SetWindowDisplayAffinity, SetWindowLongPtrW, SetWindowPos, ShowWindow, GWL_EXSTYLE,
        HWND_TOPMOST, SWP_NOACTIVATE, SWP_SHOWWINDOW, SW_HIDE, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    };

    /// Leaves the window out of screen capture (Windows 10 2004 and later).
    const WDA_EXCLUDEFROMCAPTURE: u32 = 0x11;

    pub fn prepare(hwnd: HWND) {
        unsafe {
            // Never takes focus from the game; no taskbar button or Alt+Tab entry.
            let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex | (WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW) as isize);
            SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE);
        }
    }

    pub fn show_at(hwnd: HWND, x: i32, y: i32, w: i32, h: i32) {
        unsafe {
            // Shown without activating, so the game keeps the keyboard and mouse.
            SetWindowPos(hwnd, HWND_TOPMOST, x, y, w, h, SWP_NOACTIVATE | SWP_SHOWWINDOW);
            // Set again each time: some graphics drivers reset it when the window first appears.
            SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE);
        }
    }

    /// The screen holding the window in front (the primary screen if there's none).
    pub fn screen_in_use() -> Option<Screen> {
        unsafe {
            let mon = MonitorFromWindow(GetForegroundWindow(), MONITOR_DEFAULTTOPRIMARY);
            let mut info: MONITORINFO = std::mem::zeroed();
            info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
            if GetMonitorInfoW(mon, &mut info) == 0 {
                return None;
            }
            let r = info.rcMonitor;
            Some(Screen { x: r.left, y: r.top, width: (r.right - r.left).max(0) as u32, height: (r.bottom - r.top).max(0) as u32 })
        }
    }

    pub fn hide(hwnd: HWND) {
        unsafe {
            ShowWindow(hwnd, SW_HIDE);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spots_sit_inside_the_screen() {
        let s = Screen { x: -1920, y: 0, width: 1920, height: 1080 };
        for pos in ["tl", "tc", "tr", "ml", "mr", "bl", "bc", "br"] {
            let (x, y, w, h) = place(&s, pos, 1.25);
            assert!(x >= -1920 && x + w <= 0 && y >= 0 && y + h <= 1080, "{pos}");
        }
        // Edge tabs touch their edge; cards keep a gap.
        assert_eq!(place(&s, "ml", 1.0).0, -1920);
        let (x, _, w, _) = place(&s, "mr", 1.0);
        assert_eq!(x + w, 0);
        assert_eq!(place(&s, "tl", 1.0).0, -1920 + 4);
    }
}
