//! Notifications over the game: a small window of their own, drawn by Encore itself.
//!
//! The pictures are made on the processor (notify_draw.rs), so showing a notification takes no
//! graphics card time from Encore: with a game using all of the card and the capture at top
//! priority, a web page drawn by the card never got its turn and the notifications never
//! appeared. Here Windows only has to place a finished picture, as it does for any window.
//!
//! The window is a plain Windows "layered" window: see-through where the picture is, always on
//! top, clicks pass through to the game, never takes focus, no taskbar button, and marked so
//! screen capture leaves it out (never in recordings or Echoes, nor screen sharing or
//! screenshots). It runs on its own thread, hidden until there's something to show. While
//! notifications show it is put back on top every POKE, and anything still above it is logged.
//!
//! The interface decides what to show (settings, Do not disturb, its own window in front) and
//! calls `show_notification`. Games in exclusive fullscreen draw over every window, so
//! notifications can't show there.

#![cfg_attr(not(windows), allow(dead_code, unused_imports, unused_variables))]

use crate::notify_draw::{self, Kind, Spot};
use serde_json::Value;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tauri::AppHandle;

/// Gap between a card and the screen edge (cards only; edge tabs sit on the edge).
const MARGIN: f64 = 24.0;
/// The glow room inside the window, on every side except an edge tab's screen side.
const PAD: f64 = 20.0;
/// While notifications show: a new frame this often, and back on top this often.
const FRAME: Duration = Duration::from_millis(33);
const POKE: Duration = Duration::from_millis(200);
/// While nothing shows, the window still answers Windows' messages this often.
const IDLE: Duration = Duration::from_millis(250);

/// A screen's place and size in physical pixels.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Screen {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

struct Request {
    screen: Screen,
    pos: String,
    scale: f64,
    kind: Kind,
    title: String,
    text: String,
    seconds: f32,
}

static TX: OnceLock<Mutex<Sender<Request>>> = OnceLock::new();

/// Starts the notification window's thread. Called once at start-up.
pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let (tx, rx) = channel::<Request>();
    let _ = TX.set(Mutex::new(tx));
    let app = app.clone();
    std::thread::Builder::new().name("notifications".into()).spawn(move || run(app, rx)).map(|_| ()).map_err(|e| tauri::Error::Io(e))
}

/// Shows one notification. `device` is the chosen screen's Windows name ("\\.\DISPLAY2"), or
/// None for the screen in use: the screen of `window` (the game being recorded) if given, else
/// the one holding the window in front. A screen that isn't connected falls back the same way.
/// `item`: kind, title, text, seconds.
#[tauri::command]
pub fn show_notification(app: AppHandle, device: Option<String>, window: Option<u64>, pos: String, item: Value) -> Result<(), String> {
    let monitors = app.available_monitors().unwrap_or_default();
    let chosen = device.and_then(|d| monitors.iter().find(|m| m.name().map(|n| n == &d).unwrap_or(false)));
    let screen = match chosen {
        Some(m) => Some(Screen { x: m.position().x, y: m.position().y, width: m.size().width, height: m.size().height }),
        None => {
            #[cfg(windows)]
            let s = window.and_then(win::screen_of_window).or_else(win::screen_in_use);
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
    let get = |k: &str| item.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let kind_name = get("kind");
    let seconds = item.get("seconds").and_then(|v| v.as_f64()).unwrap_or(5.0).clamp(2.0, 10.0) as f32;
    // The size chosen in Settings, on top of the screen's scale.
    let size = item.get("size").and_then(|v| v.as_f64()).unwrap_or(100.0).clamp(100.0, 200.0) / 100.0;
    let scale = scale * size;
    let on = if chosen.is_some() { "chosen screen" } else if window.is_some() { "game's screen" } else { "screen in use" };
    crate::hotkeys::note(&app, format!("notification {kind_name}: {pos} on the {on} ({},{} {}x{}, scale {scale})", screen.x, screen.y, screen.width, screen.height));
    let req = Request { screen, pos, scale, kind: Kind::parse(&kind_name), title: get("title"), text: get("text"), seconds };
    TX.get()
        .and_then(|t| t.lock().ok().map(|t| t.send(req).is_ok()))
        .filter(|ok| *ok)
        .map(|_| ())
        .ok_or_else(|| "The notification window isn't running.".to_string())
}

/// The window's place and size in physical pixels for a spot on a screen.
fn place(s: &Screen, pos: &str, scale: f64) -> (i32, i32, i32, i32) {
    let spot = Spot::parse(pos);
    let w = (spot.window_w() as f64 * scale).round() as i32;
    let h = (notify_draw::WINDOW_H as f64 * scale).round() as i32;
    let edge = ((MARGIN - PAD) * scale).round() as i32; // the glow room counts towards the gap
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

/// The notification window's thread: takes requests, draws frames while anything shows.
#[cfg(not(windows))]
fn run(_app: AppHandle, rx: Receiver<Request>) {
    while rx.recv().is_ok() {}
}

#[cfg(windows)]
fn run(app: AppHandle, rx: Receiver<Request>) {
    use notify_draw::{Shown, Stack};
    use std::sync::mpsc::RecvTimeoutError;
    use std::time::Instant;

    notify_draw::warm_up();
    let Some(mut window) = win::Overlay::new() else {
        crate::hotkeys::note(&app, "notifications: couldn't create the window".into());
        return;
    };
    let clock = Instant::now();
    let mut stack: Option<Stack> = None;
    let mut placed: Option<(Screen, String, f64)> = None;
    let mut rect = (0, 0, 0, 0);
    let mut last_poke = Instant::now();
    let mut checked = false;
    loop {
        window.pump();
        let busy = stack.as_ref().map(|s| !s.is_empty()).unwrap_or(false);
        match rx.recv_timeout(if busy { FRAME } else { IDLE }) {
            Ok(req) => {
                let now = clock.elapsed().as_secs_f32();
                let here = (req.screen, req.pos.clone(), req.scale);
                // Another spot or screen: start afresh there.
                if placed.as_ref() != Some(&here) || stack.is_none() {
                    stack = Some(Stack::new(Spot::parse(&req.pos), req.scale as f32));
                    rect = place(&req.screen, &req.pos, req.scale);
                    placed = Some(here);
                }
                let started = Instant::now();
                let spot = Spot::parse(&req.pos);
                match notify_draw::draw_item(req.kind, &req.title, &req.text, spot, req.scale as f32) {
                    Some(sprite) => {
                        if let Some(st) = stack.as_mut() {
                            st.add(Shown::new(req.kind, sprite, req.seconds, now), now);
                        }
                        crate::hotkeys::note(&app, format!("notification drawn in {} ms", started.elapsed().as_millis()));
                    }
                    None => crate::hotkeys::note(&app, "notification: couldn't draw it".into()),
                }
                if !window.visible {
                    checked = false;
                    last_poke = Instant::now();
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        let Some(st) = stack.as_mut() else { continue };
        let now = clock.elapsed().as_secs_f32();
        st.update(now);
        if st.is_empty() {
            if window.visible {
                window.hide();
            }
            continue;
        }
        if let Some(frame) = st.frame(now) {
            window.show_frame(&frame, rect.0, rect.1);
        }
        if last_poke.elapsed() >= POKE {
            last_poke = Instant::now();
            window.raise();
            if !checked {
                checked = true;
                let above = window.windows_above();
                if !above.is_empty() {
                    crate::hotkeys::note(&app, format!("notifications: covered by {}", above.join(", ")));
                }
            }
        }
    }
}

#[cfg(windows)]
mod win {
    use super::Screen;
    use resvg::tiny_skia::Pixmap;
    use std::ffi::c_void;
    use std::mem::{size_of, zeroed};
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::Foundation::{CloseHandle, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
    use windows_sys::Win32::Graphics::Gdi::{
        CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, GetMonitorInfoW, MonitorFromWindow, ReleaseDC,
        SelectObject, AC_SRC_ALPHA, AC_SRC_OVER, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, BLENDFUNCTION, DIB_RGB_COLORS, HBITMAP,
        HDC, HGDIOBJ, MONITORINFO, MONITOR_DEFAULTTOPRIMARY,
    };
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GetForegroundWindow, GetWindow, GetWindowLongPtrW, GetWindowRect,
        GetWindowThreadProcessId, IsWindow, IsWindowVisible, PeekMessageW, RegisterClassW, SetWindowDisplayAffinity, SetWindowPos,
        ShowWindow, TranslateMessage, UpdateLayeredWindow, GWL_EXSTYLE, GW_HWNDPREV, HWND_TOPMOST, MSG, PM_REMOVE, SWP_NOACTIVATE,
        SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SW_HIDE, ULW_ALPHA, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
        WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
    };

    /// Leaves the window out of screen capture (Windows 10 2004 and later).
    const WDA_EXCLUDEFROMCAPTURE: u32 = 0x11;

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
    }

    pub struct Overlay {
        hwnd: HWND,
        pub visible: bool,
        /// The picture handed to Windows: a memory DC holding a bitmap we write pixels into.
        dc: HDC,
        bitmap: HBITMAP,
        old: HGDIOBJ,
        bits: *mut u8,
        size: (i32, i32),
    }

    impl Overlay {
        pub fn new() -> Option<Overlay> {
            unsafe {
                let class = wide("EncoreNotifications");
                let title = wide("Encore notifications");
                let instance = GetModuleHandleW(null());
                let mut wc: WNDCLASSW = zeroed();
                wc.lpfnWndProc = Some(wndproc);
                wc.hInstance = instance;
                wc.lpszClassName = class.as_ptr();
                RegisterClassW(&wc);
                let hwnd = CreateWindowExW(
                    WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                    class.as_ptr(),
                    title.as_ptr(),
                    WS_POPUP,
                    0,
                    0,
                    1,
                    1,
                    null_mut(),
                    null_mut(),
                    instance,
                    null(),
                );
                if hwnd.is_null() {
                    return None;
                }
                SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE);
                let dc = CreateCompatibleDC(null_mut());
                Some(Overlay { hwnd, visible: false, dc, bitmap: null_mut(), old: null_mut(), bits: null_mut(), size: (0, 0) })
            }
        }

        /// Answers Windows' messages (it must, or programs that message every window would wait).
        pub fn pump(&mut self) {
            unsafe {
                let mut msg: MSG = zeroed();
                while PeekMessageW(&mut msg, null_mut(), 0, 0, PM_REMOVE) != 0 {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        }

        fn ensure_bitmap(&mut self, w: i32, h: i32) -> bool {
            if self.size == (w, h) && !self.bitmap.is_null() {
                return true;
            }
            unsafe {
                if !self.bitmap.is_null() {
                    SelectObject(self.dc, self.old);
                    DeleteObject(self.bitmap as HGDIOBJ);
                    self.bitmap = null_mut();
                }
                let mut bmi: BITMAPINFO = zeroed();
                bmi.bmiHeader = BITMAPINFOHEADER {
                    biSize: size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: w,
                    biHeight: -h, // top-down
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB,
                    biSizeImage: 0,
                    biXPelsPerMeter: 0,
                    biYPelsPerMeter: 0,
                    biClrUsed: 0,
                    biClrImportant: 0,
                };
                let mut bits: *mut c_void = null_mut();
                let bitmap = CreateDIBSection(self.dc, &bmi, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
                if bitmap.is_null() || bits.is_null() {
                    return false;
                }
                self.old = SelectObject(self.dc, bitmap as HGDIOBJ);
                self.bitmap = bitmap;
                self.bits = bits as *mut u8;
                self.size = (w, h);
            }
            true
        }

        /// Puts a frame on screen at (x, y), showing the window (without focus) if hidden.
        pub fn show_frame(&mut self, frame: &Pixmap, x: i32, y: i32) {
            let (w, h) = (frame.width() as i32, frame.height() as i32);
            if !self.ensure_bitmap(w, h) {
                return;
            }
            unsafe {
                // RGBA (premultiplied) to the BGRA Windows wants.
                let src = frame.data();
                let dst = std::slice::from_raw_parts_mut(self.bits, src.len());
                for (d, s) in dst.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
                    d[0] = s[2];
                    d[1] = s[1];
                    d[2] = s[0];
                    d[3] = s[3];
                }
                let screen = GetDC(null_mut());
                let at = POINT { x, y };
                let size = SIZE { cx: w, cy: h };
                let from = POINT { x: 0, y: 0 };
                let blend = BLENDFUNCTION { BlendOp: AC_SRC_OVER as u8, BlendFlags: 0, SourceConstantAlpha: 255, AlphaFormat: AC_SRC_ALPHA as u8 };
                UpdateLayeredWindow(self.hwnd, screen, &at, &size, self.dc, &from, 0, &blend, ULW_ALPHA);
                ReleaseDC(null_mut(), screen);
                if !self.visible {
                    SetWindowPos(self.hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW);
                    // Set again each time it appears: some graphics drivers reset it.
                    SetWindowDisplayAffinity(self.hwnd, WDA_EXCLUDEFROMCAPTURE);
                    self.visible = true;
                }
            }
        }

        pub fn hide(&mut self) {
            unsafe {
                ShowWindow(self.hwnd, SW_HIDE);
            }
            self.visible = false;
        }

        /// Back to the top of the always-on-top windows, without moving or activating.
        pub fn raise(&self) {
            unsafe {
                SetWindowPos(self.hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
            }
        }

        /// Visible windows above this one that overlap it: "program.exe (always on top)".
        pub fn windows_above(&self) -> Vec<String> {
            let mut out = Vec::new();
            unsafe {
                let mut mine: RECT = zeroed();
                if GetWindowRect(self.hwnd, &mut mine) == 0 {
                    return out;
                }
                let mut h = GetWindow(self.hwnd, GW_HWNDPREV);
                let mut steps = 0;
                while !h.is_null() && steps < 500 {
                    steps += 1;
                    let mut r: RECT = zeroed();
                    if IsWindowVisible(h) != 0
                        && GetWindowRect(h, &mut r) != 0
                        && r.left < mine.right
                        && r.right > mine.left
                        && r.top < mine.bottom
                        && r.bottom > mine.top
                    {
                        let mut pid = 0u32;
                        GetWindowThreadProcessId(h, &mut pid);
                        let top = (GetWindowLongPtrW(h, GWL_EXSTYLE) as u32 & WS_EX_TOPMOST) != 0;
                        let name = format!("{}{}", exe_name(pid), if top { " (always on top)" } else { "" });
                        if !out.contains(&name) {
                            out.push(name);
                        }
                    }
                    h = GetWindow(h, GW_HWNDPREV);
                }
            }
            out
        }
    }

    fn exe_name(pid: u32) -> String {
        use windows_sys::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION};
        unsafe {
            let hp = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if hp.is_null() {
                return format!("pid {pid}");
            }
            let mut buf = [0u16; 512];
            let mut len = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(hp, 0, buf.as_mut_ptr(), &mut len);
            CloseHandle(hp);
            if ok == 0 {
                return format!("pid {pid}");
            }
            let path = String::from_utf16_lossy(&buf[..len as usize]);
            path.rsplit('\\').next().unwrap_or(&path).to_string()
        }
    }

    /// The screen holding the window in front (the primary screen if there's none).
    pub fn screen_in_use() -> Option<Screen> {
        unsafe { screen_of(GetForegroundWindow()) }
    }

    /// The screen holding a window (a game's), if the window still exists.
    pub fn screen_of_window(handle: u64) -> Option<Screen> {
        let hwnd = handle as usize as HWND;
        unsafe {
            if IsWindow(hwnd) == 0 {
                return None;
            }
            screen_of(hwnd)
        }
    }

    unsafe fn screen_of(hwnd: HWND) -> Option<Screen> {
        unsafe {
            let mon = MonitorFromWindow(hwnd, MONITOR_DEFAULTTOPRIMARY);
            let mut info: MONITORINFO = zeroed();
            info.cbSize = size_of::<MONITORINFO>() as u32;
            if GetMonitorInfoW(mon, &mut info) == 0 {
                return None;
            }
            let r = info.rcMonitor;
            Some(Screen { x: r.left, y: r.top, width: (r.right - r.left).max(0) as u32, height: (r.bottom - r.top).max(0) as u32 })
        }
    }

    impl Drop for Overlay {
        fn drop(&mut self) {
            unsafe {
                if !self.bitmap.is_null() {
                    SelectObject(self.dc, self.old);
                    DeleteObject(self.bitmap as HGDIOBJ);
                }
                DeleteDC(self.dc);
            }
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
