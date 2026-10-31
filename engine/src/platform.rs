//! Windows-specific lookups: monitors, windows, cursor position, local time.
//! On other platforms these return empty results so the rest of the engine can be tested anywhere.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Monitor {
    /// Stable identifier (device path). Does not change when monitors are renumbered.
    pub id: String,
    /// Friendly name, e.g. "DELL S2721DGF".
    pub name: String,
    /// GDI device name, e.g. "\\.\DISPLAY1" (changes when monitors are plugged/unplugged).
    pub device: String,
    /// HMONITOR handle for capture (valid for this session only).
    pub handle: u64,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub primary: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct WindowInfo {
    pub handle: u64,
    pub title: String,
    pub exe: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalTime {
    pub year: u16,
    pub month: u16,
    pub day: u16,
    pub hour: u16,
    pub minute: u16,
    pub second: u16,
}

#[cfg(windows)]
pub use win::*;

#[cfg(not(windows))]
pub use other::*;

#[cfg(not(windows))]
mod other {
    use super::*;
    pub fn init_dpi() {}
    pub fn list_monitors() -> Vec<Monitor> {
        Vec::new()
    }
    pub fn list_windows() -> Vec<WindowInfo> {
        Vec::new()
    }
    pub fn monitor_under_cursor() -> Option<u64> {
        None
    }
    pub fn window_exists(_handle: u64) -> bool {
        false
    }
    pub fn window_size(_handle: u64) -> Option<(u32, u32)> {
        None
    }
    pub fn window_minimised(_handle: u64) -> bool {
        false
    }
    pub fn local_time() -> LocalTime {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        // UTC; only used in tests and non-Windows builds.
        let days = secs / 86_400;
        let rem = secs % 86_400;
        let (y, m, d) = civil_from_days(days as i64);
        LocalTime {
            year: y as u16,
            month: m as u16,
            day: d as u16,
            hour: (rem / 3600) as u16,
            minute: (rem % 3600 / 60) as u16,
            second: (rem % 60) as u16,
        }
    }
    fn civil_from_days(z: i64) -> (i64, i64, i64) {
        let z = z + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let y = yoe + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        (if m <= 2 { y + 1 } else { y }, m, d)
    }
}

#[cfg(windows)]
mod win {
    use super::*;
    use std::collections::HashMap;
    use std::ffi::c_void;
    use std::mem::{size_of, zeroed};
    use std::ptr::{null, null_mut};

    use windows_sys::Win32::Devices::Display::{
        DisplayConfigGetDeviceInfo, GetDisplayConfigBufferSizes, QueryDisplayConfig,
        DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_PATH_INFO, DISPLAYCONFIG_SOURCE_DEVICE_NAME,
        DISPLAYCONFIG_TARGET_DEVICE_NAME,
    };
    use windows_sys::Win32::Foundation::{CloseHandle, BOOL, HWND, LPARAM, POINT, RECT, SYSTEMTIME};
    use windows_sys::Win32::Graphics::Dwm::DwmGetWindowAttribute;
    use windows_sys::Win32::Graphics::Gdi::{
        EnumDisplayMonitors, GetMonitorInfoW, MonitorFromPoint, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW,
    };
    use windows_sys::Win32::System::SystemInformation::GetLocalTime;
    use windows_sys::Win32::System::Threading::{GetCurrentProcessId, OpenProcess, QueryFullProcessImageNameW};
    use windows_sys::Win32::UI::HiDpi::SetProcessDpiAwarenessContext;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetClientRect, GetCursorPos, GetWindow, GetWindowLongPtrW, GetWindowTextLengthW,
        GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindow, IsWindowVisible,
    };

    // Constants defined locally so they do not depend on where a binding crate version puts them.
    const MONITORINFOF_PRIMARY: u32 = 1;
    const MONITOR_DEFAULTTONEAREST: u32 = 2;
    const QDC_ONLY_ACTIVE_PATHS: u32 = 2;
    const GET_SOURCE_NAME: i32 = 1;
    const GET_TARGET_NAME: i32 = 2;
    const GWL_EXSTYLE: i32 = -20;
    const WS_EX_TOOLWINDOW: isize = 0x80;
    const GW_OWNER: u32 = 4;
    const DWMWA_CLOAKED: i32 = 14;
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const DPI_PER_MONITOR_AWARE_V2: isize = -4;

    fn wide_to_string(buf: &[u16]) -> String {
        let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        String::from_utf16_lossy(&buf[..end])
    }

    fn h2u(h: *mut c_void) -> u64 {
        h as usize as u64
    }

    fn u2h(v: u64) -> *mut c_void {
        v as usize as *mut c_void
    }

    /// Makes all sizes real pixels (not scaled by Windows display scaling).
    pub fn init_dpi() {
        unsafe {
            SetProcessDpiAwarenessContext(DPI_PER_MONITOR_AWARE_V2 as _);
        }
    }

    /// Map of GDI device name ("\\.\DISPLAY1") to (friendly name, device path).
    fn display_names() -> HashMap<String, (String, String)> {
        let mut map = HashMap::new();
        unsafe {
            let mut n_paths: u32 = 0;
            let mut n_modes: u32 = 0;
            if GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS as _, &mut n_paths, &mut n_modes) != 0 {
                return map;
            }
            let mut paths: Vec<DISPLAYCONFIG_PATH_INFO> = vec![zeroed(); n_paths as usize];
            let mut modes: Vec<DISPLAYCONFIG_MODE_INFO> = vec![zeroed(); n_modes as usize];
            if QueryDisplayConfig(
                QDC_ONLY_ACTIVE_PATHS as _,
                &mut n_paths,
                paths.as_mut_ptr(),
                &mut n_modes,
                modes.as_mut_ptr(),
                null_mut(),
            ) != 0
            {
                return map;
            }
            for p in paths.iter().take(n_paths as usize) {
                let mut src: DISPLAYCONFIG_SOURCE_DEVICE_NAME = zeroed();
                src.header.r#type = GET_SOURCE_NAME as _;
                src.header.size = size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32;
                src.header.adapterId = p.sourceInfo.adapterId;
                src.header.id = p.sourceInfo.id;
                if DisplayConfigGetDeviceInfo(&mut src.header) != 0 {
                    continue;
                }
                let mut tgt: DISPLAYCONFIG_TARGET_DEVICE_NAME = zeroed();
                tgt.header.r#type = GET_TARGET_NAME as _;
                tgt.header.size = size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>() as u32;
                tgt.header.adapterId = p.targetInfo.adapterId;
                tgt.header.id = p.targetInfo.id;
                if DisplayConfigGetDeviceInfo(&mut tgt.header) != 0 {
                    continue;
                }
                map.insert(
                    wide_to_string(&src.viewGdiDeviceName),
                    (wide_to_string(&tgt.monitorFriendlyDeviceName), wide_to_string(&tgt.monitorDevicePath)),
                );
            }
        }
        map
    }

    unsafe extern "system" fn collect_monitor(hm: HMONITOR, _dc: HDC, _rc: *mut RECT, data: LPARAM) -> BOOL {
        let list = &mut *(data as *mut Vec<HMONITOR>);
        list.push(hm);
        1
    }

    pub fn list_monitors() -> Vec<Monitor> {
        let names = display_names();
        let mut handles: Vec<HMONITOR> = Vec::new();
        let mut out = Vec::new();
        unsafe {
            EnumDisplayMonitors(null_mut(), null(), Some(collect_monitor), &mut handles as *mut _ as LPARAM);
            for (i, hm) in handles.into_iter().enumerate() {
                let mut info: MONITORINFOEXW = zeroed();
                info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
                if GetMonitorInfoW(hm, &mut info as *mut MONITORINFOEXW as *mut MONITORINFO) == 0 {
                    continue;
                }
                let r = info.monitorInfo.rcMonitor;
                let device = wide_to_string(&info.szDevice);
                let (friendly, path) = names.get(&device).cloned().unwrap_or_default();
                let name = if friendly.trim().is_empty() { format!("Monitor {}", i + 1) } else { friendly };
                let id = if path.is_empty() { device.clone() } else { path };
                out.push(Monitor {
                    id,
                    name,
                    device,
                    handle: h2u(hm as *mut c_void),
                    x: r.left,
                    y: r.top,
                    width: (r.right - r.left).max(0) as u32,
                    height: (r.bottom - r.top).max(0) as u32,
                    primary: info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
                });
            }
        }
        out
    }

    pub fn monitor_under_cursor() -> Option<u64> {
        unsafe {
            let mut pt: POINT = zeroed();
            if GetCursorPos(&mut pt) == 0 {
                return None;
            }
            let hm = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST as _);
            if hm.is_null() {
                None
            } else {
                Some(h2u(hm as *mut c_void))
            }
        }
    }

    unsafe extern "system" fn collect_window(h: HWND, data: LPARAM) -> BOOL {
        let list = &mut *(data as *mut Vec<HWND>);
        list.push(h);
        1
    }

    fn window_title(h: HWND) -> String {
        unsafe {
            let len = GetWindowTextLengthW(h);
            if len <= 0 {
                return String::new();
            }
            let mut buf = vec![0u16; len as usize + 1];
            let got = GetWindowTextW(h, buf.as_mut_ptr(), buf.len() as i32);
            String::from_utf16_lossy(&buf[..got.max(0) as usize])
        }
    }

    fn process_exe(pid: u32) -> String {
        unsafe {
            let hp = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION as _, 0, pid);
            if hp.is_null() {
                return String::new();
            }
            let mut buf = vec![0u16; 1024];
            let mut size = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(hp, 0, buf.as_mut_ptr(), &mut size);
            CloseHandle(hp);
            if ok == 0 {
                return String::new();
            }
            let full = String::from_utf16_lossy(&buf[..size as usize]);
            full.rsplit(['\\', '/']).next().unwrap_or("").to_string()
        }
    }

    fn client_size(h: HWND) -> Option<(u32, u32)> {
        unsafe {
            let mut r: RECT = zeroed();
            if GetClientRect(h, &mut r) == 0 {
                return None;
            }
            Some(((r.right - r.left).max(0) as u32, (r.bottom - r.top).max(0) as u32))
        }
    }

    /// Top-level windows a person would recognise as an app or game.
    pub fn list_windows() -> Vec<WindowInfo> {
        let mut handles: Vec<HWND> = Vec::new();
        let mut out = Vec::new();
        unsafe {
            EnumWindows(Some(collect_window), &mut handles as *mut _ as LPARAM);
            let me = GetCurrentProcessId();
            for h in handles {
                if IsWindowVisible(h) == 0 || !GetWindow(h, GW_OWNER as _).is_null() {
                    continue;
                }
                if GetWindowLongPtrW(h, GWL_EXSTYLE as _) & WS_EX_TOOLWINDOW != 0 {
                    continue;
                }
                let mut cloaked: u32 = 0;
                DwmGetWindowAttribute(h, DWMWA_CLOAKED as _, &mut cloaked as *mut u32 as *mut c_void, 4);
                if cloaked != 0 {
                    continue;
                }
                let title = window_title(h);
                if title.trim().is_empty() || title == "Program Manager" {
                    continue;
                }
                let mut pid: u32 = 0;
                GetWindowThreadProcessId(h, &mut pid);
                if pid == me {
                    continue;
                }
                let (w, ht) = client_size(h).unwrap_or((0, 0));
                let minimised = IsIconic(h) != 0;
                if !minimised && (w < 100 || ht < 100) {
                    continue;
                }
                out.push(WindowInfo { handle: h2u(h), title, exe: process_exe(pid), width: w, height: ht });
            }
        }
        out
    }

    pub fn window_exists(handle: u64) -> bool {
        unsafe { IsWindow(u2h(handle) as HWND) != 0 }
    }

    pub fn window_size(handle: u64) -> Option<(u32, u32)> {
        client_size(u2h(handle) as HWND)
    }

    pub fn window_minimised(handle: u64) -> bool {
        unsafe { IsIconic(u2h(handle) as HWND) != 0 }
    }

    pub fn local_time() -> LocalTime {
        unsafe {
            let mut t: SYSTEMTIME = zeroed();
            GetLocalTime(&mut t);
            LocalTime {
                year: t.wYear,
                month: t.wMonth,
                day: t.wDay,
                hour: t.wHour,
                minute: t.wMinute,
                second: t.wSecond,
            }
        }
    }
}
