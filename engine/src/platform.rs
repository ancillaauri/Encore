//! Windows-specific lookups: monitors, windows, cursor position, local time.
//! On other platforms these return empty results so the rest of the engine can be tested anywhere.

#![cfg_attr(not(windows), allow(dead_code))]

use serde::Serialize;

/// Realtime graphics priority for the capture: Priority Capture (sidebar, Settings → Advanced).
static GPU_TOP: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_gpu_top(on: bool) {
    GPU_TOP.store(on, std::sync::atomic::Ordering::Relaxed);
}

pub fn gpu_top() -> bool {
    GPU_TOP.load(std::sync::atomic::Ordering::Relaxed)
}

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

/// A visible top-level window, with what game detection needs to know about it.
#[derive(Debug, Clone, Default)]
pub struct TopWindow {
    pub handle: u64,
    pub pid: u32,
    pub title: String,
    /// Program file name, e.g. "League of Legends.exe".
    pub exe: String,
    /// Full path of the program, when Windows lets us read it (empty otherwise).
    pub path: String,
    pub width: u32,
    pub height: u32,
    /// The window fills (or overflows) its whole monitor.
    pub covers_monitor: bool,
    /// The window has a normal title bar (most games in full screen or borderless don't).
    pub captioned: bool,
    pub minimised: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct WindowInfo {
    pub handle: u64,
    pub title: String,
    pub exe: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone)]
pub struct ProcInfo {
    pub parent: u32,
    pub exe: String,
}

/// The top-most ancestor of `pid` that is the same program (e.g. Chrome's audio helper -> Chrome).
/// Capturing that process "and its children" gets all of the app's sound.
pub fn app_root(pid: u32, procs: &std::collections::HashMap<u32, ProcInfo>) -> u32 {
    let Some(me) = procs.get(&pid) else { return pid };
    let exe = me.exe.to_ascii_lowercase();
    let mut current = pid;
    for _ in 0..32 {
        match procs.get(&current) {
            Some(p) if p.parent != 0 && p.parent != current => match procs.get(&p.parent) {
                Some(parent) if parent.exe.to_ascii_lowercase() == exe => current = p.parent,
                _ => break,
            },
            _ => break,
        }
    }
    current
}

/// "VALORANT-Win64-Shipping.exe" -> "VALORANT-Win64-Shipping"; used when an app has no description.
pub fn exe_stem(exe: &str) -> String {
    let base = exe.rsplit(['\\', '/']).next().unwrap_or(exe);
    match base.rfind('.') {
        Some(i) if i > 0 => base[..i].to_string(),
        _ => base.to_string(),
    }
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

/// What the focused window says about which monitor the person is using.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusRule {
    /// Not a signal (Encore itself, Alt+Tab, Start menu): keep whatever was being recorded.
    Ignore,
    /// The desktop or a taskbar: they clicked there, so use the monitor under the mouse.
    UseCursor,
    /// An ordinary window: use the monitor it's on.
    UseWindow,
}

/// Decides from the focused window's class name and its program's file name (e.g. "Discord.exe").
pub fn focus_rule(class: &str, exe: &str) -> FocusRule {
    let stem = exe_stem(exe).to_ascii_lowercase();
    if stem == "encore" || stem == "encore-engine" {
        return FocusRule::Ignore;
    }
    match class {
        "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd" => FocusRule::UseCursor,
        "" | "MultitaskingViewFrame" | "XamlExplorerHostIslandWindow" | "ForegroundStaging"
        | "Windows.UI.Core.CoreWindow" | "TopLevelWindowForOverflowXamlIsland" | "NotifyIconOverflowWindow" => {
            FocusRule::Ignore
        }
        _ => FocusRule::UseWindow,
    }
}

#[cfg(not(windows))]
pub use other::*;

#[cfg(not(windows))]
mod other {
    /// Hidden files are a Windows idea: elsewhere, the folder's leading dot hides it.
    pub fn hide(_path: &std::path::Path) {}

    use super::*;
    pub fn init_dpi() {}
    pub fn list_monitors() -> Vec<Monitor> {
        Vec::new()
    }
    pub fn list_windows() -> Vec<WindowInfo> {
        Vec::new()
    }
    pub fn top_windows() -> Vec<TopWindow> {
        Vec::new()
    }
    pub fn foreground_window() -> u64 {
        0
    }
    pub fn monitor_under_cursor() -> Option<u64> {
        None
    }
    pub fn focused_monitor() -> Option<u64> {
        None
    }
    pub fn raise_cpu_priority(_pid: u32) {}
    pub fn try_gpu_priority(_pid: u32) -> Result<String, String> {
        Err("not on Windows".into())
    }
    pub fn is_elevated() -> bool {
        false
    }
    pub fn videos_folder() -> Option<std::path::PathBuf> {
        None
    }
    pub fn window_exists(_handle: u64) -> bool {
        false
    }
    pub fn exclusive_fullscreen() -> bool {
        false
    }
    pub fn windows_version() -> String {
        "not Windows".into()
    }
    pub fn window_size(_handle: u64) -> Option<(u32, u32)> {
        None
    }
    pub fn window_minimised(_handle: u64) -> bool {
        false
    }
    /// Monotonic clock in 100 ns units (Windows uses the same clock as audio and video timestamps).
    pub fn now_hns() -> u64 {
        use std::sync::OnceLock;
        static START: OnceLock<std::time::Instant> = OnceLock::new();
        let start = START.get_or_init(std::time::Instant::now);
        1_000_000_000 + (start.elapsed().as_nanos() / 100) as u64
    }
    pub fn processes() -> std::collections::HashMap<u32, ProcInfo> {
        std::collections::HashMap::new()
    }
    pub fn app_display_name(pid: u32) -> String {
        format!("App {pid}")
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
        EnumDisplayMonitors, GetMonitorInfoW, MonitorFromPoint, MonitorFromWindow, HDC, HMONITOR, MONITORINFO,
        MONITORINFOEXW,
    };
    use windows_sys::Win32::Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
    };
    use windows_sys::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
    use windows_sys::Win32::System::SystemInformation::GetLocalTime;
    use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
    use windows_sys::Win32::Foundation::LUID;
    use windows_sys::Win32::Security::{
        AdjustTokenPrivileges, GetTokenInformation, LookupPrivilegeValueW, TokenElevation, LUID_AND_ATTRIBUTES,
        SE_PRIVILEGE_ENABLED, TOKEN_ADJUST_PRIVILEGES, TOKEN_ELEVATION, TOKEN_PRIVILEGES, TOKEN_QUERY,
    };
    use windows_sys::Win32::System::Threading::{
        GetCurrentProcess, GetCurrentProcessId, OpenProcess, OpenProcessToken, QueryFullProcessImageNameW,
        SetPriorityClass,
    };
    use windows_sys::Win32::UI::HiDpi::SetProcessDpiAwarenessContext;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetClassNameW, GetClientRect, GetCursorPos, GetForegroundWindow, GetWindow, GetWindowLongPtrW,
        GetWindowRect,
        GetWindowTextLengthW,
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
    const TH32CS_SNAPPROCESS: u32 = 2;

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

    /// The monitor the person is using: the one holding the window they last clicked on (the
    /// focused window). None when the focused window says nothing about it (see `focus_rule`).
    pub fn focused_monitor() -> Option<u64> {
        unsafe {
            let h = GetForegroundWindow();
            if h.is_null() {
                return None;
            }
            let mut buf = [0u16; 128];
            let n = GetClassNameW(h, buf.as_mut_ptr(), buf.len() as i32);
            let class = String::from_utf16_lossy(&buf[..n.max(0) as usize]);
            let mut pid: u32 = 0;
            GetWindowThreadProcessId(h, &mut pid);
            match focus_rule(&class, &process_exe(pid)) {
                FocusRule::Ignore => None,
                FocusRule::UseCursor => monitor_under_cursor(),
                FocusRule::UseWindow => {
                    let hm = MonitorFromWindow(h, MONITOR_DEFAULTTONEAREST as _);
                    if hm.is_null() { None } else { Some(h2u(hm as *mut c_void)) }
                }
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

    /// Windows' high-resolution clock in 100 ns units: the same clock audio packets and
    /// captured video frames are stamped with.
    pub fn now_hns() -> u64 {
        unsafe {
            let mut freq: i64 = 0;
            let mut count: i64 = 0;
            QueryPerformanceFrequency(&mut freq);
            QueryPerformanceCounter(&mut count);
            if freq <= 0 {
                return 0;
            }
            (count as i128 * 10_000_000 / freq as i128) as u64
        }
    }

    /// Every running process: parent and executable name.
    pub fn processes() -> HashMap<u32, ProcInfo> {
        let mut map = HashMap::new();
        unsafe {
            let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS as _, 0);
            if snap.is_null() || snap as isize == -1 {
                return map;
            }
            let mut e: PROCESSENTRY32W = zeroed();
            e.dwSize = size_of::<PROCESSENTRY32W>() as u32;
            if Process32FirstW(snap, &mut e) != 0 {
                loop {
                    map.insert(
                        e.th32ProcessID,
                        ProcInfo { parent: e.th32ParentProcessID, exe: wide_to_string(&e.szExeFile) },
                    );
                    if Process32NextW(snap, &mut e) == 0 {
                        break;
                    }
                }
            }
            CloseHandle(snap);
        }
        map
    }

    fn process_path(pid: u32) -> String {
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
            String::from_utf16_lossy(&buf[..size as usize])
        }
    }

    fn to_wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// The "File description" of a program, e.g. "Google Chrome", "Spotify".
    fn file_description(path: &str) -> Option<String> {
        unsafe {
            let wpath = to_wide(path);
            let mut handle: u32 = 0;
            let size = GetFileVersionInfoSizeW(wpath.as_ptr(), &mut handle);
            if size == 0 {
                return None;
            }
            let mut data = vec![0u8; size as usize];
            if GetFileVersionInfoW(wpath.as_ptr(), 0, size, data.as_mut_ptr() as *mut c_void) == 0 {
                return None;
            }
            let mut ptr: *mut c_void = null_mut();
            let mut len: u32 = 0;
            let trans = to_wide("\\VarFileInfo\\Translation");
            let mut lang_cp = (0x0409u16, 0x04b0u16);
            if VerQueryValueW(data.as_ptr() as *const c_void, trans.as_ptr(), &mut ptr, &mut len) != 0 && len >= 4 {
                let pair = std::slice::from_raw_parts(ptr as *const u16, 2);
                lang_cp = (pair[0], pair[1]);
            }
            let key = to_wide(&format!("\\StringFileInfo\\{:04x}{:04x}\\FileDescription", lang_cp.0, lang_cp.1));
            if VerQueryValueW(data.as_ptr() as *const c_void, key.as_ptr(), &mut ptr, &mut len) == 0 || len == 0 {
                return None;
            }
            let text = std::slice::from_raw_parts(ptr as *const u16, len as usize);
            let s = wide_to_string(text).trim().to_string();
            if s.is_empty() { None } else { Some(s) }
        }
    }

    /// Marks a file or folder hidden, as Explorer shows it (a no-op if it's gone).
    pub fn hide(path: &std::path::Path) {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::{GetFileAttributesW, SetFileAttributesW, FILE_ATTRIBUTE_HIDDEN, INVALID_FILE_ATTRIBUTES};
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        unsafe {
            let now = GetFileAttributesW(wide.as_ptr());
            if now != INVALID_FILE_ATTRIBUTES && now & FILE_ATTRIBUTE_HIDDEN == 0 {
                SetFileAttributesW(wide.as_ptr(), now | FILE_ATTRIBUTE_HIDDEN);
            }
        }
    }

    /// Friendly name for an app's audio track: its file description, else the program name.
    pub fn app_display_name(pid: u32) -> String {
        let path = process_path(pid);
        if let Some(d) = file_description(&path).filter(|d| d.chars().count() <= 48) {
            return d;
        }
        let mut stem = exe_stem(&path);
        if stem.is_empty() {
            // Protected processes (some games, anti-cheat) don't let us read their details, but
            // their program file name is always listed.
            stem = processes().get(&pid).map(|p| exe_stem(&p.exe)).unwrap_or_default();
        }
        if stem.is_empty() { format!("App {pid}") } else { stem }
    }

    /// The person's real Videos folder, wherever Windows keeps it (it can be moved to another
    /// drive or into OneDrive, so it isn't always %USERPROFILE%\Videos).
    pub fn videos_folder() -> Option<std::path::PathBuf> {
        use windows_sys::core::GUID;
        use windows_sys::Win32::System::Com::CoTaskMemFree;
        use windows_sys::Win32::UI::Shell::SHGetKnownFolderPath;
        const FOLDERID_VIDEOS: GUID = GUID::from_u128(0x18989b1d_99b5_455b_841c_ab7c74e4ddfc);
        unsafe {
            let mut p: *mut u16 = null_mut();
            let hr = SHGetKnownFolderPath(&FOLDERID_VIDEOS, 0, null_mut(), &mut p);
            if hr < 0 || p.is_null() {
                if !p.is_null() {
                    CoTaskMemFree(p as *const c_void);
                }
                return None;
            }
            let mut len = 0;
            while *p.add(len) != 0 {
                len += 1;
            }
            let s = String::from_utf16_lossy(std::slice::from_raw_parts(p, len));
            CoTaskMemFree(p as *const c_void);
            if s.is_empty() { None } else { Some(std::path::PathBuf::from(s)) }
        }
    }

    /// Gives a capture process (FFmpeg) priority on the graphics card and the processor, so a
    /// game using the whole GPU can't starve the capture (which shows up as a choppy recording).
    /// The highest GPU level needs administrator rights; otherwise "high" is used.
    /// True when Encore is running as administrator (an elevated token).
    pub fn is_elevated() -> bool {
        unsafe {
            let mut token = null_mut();
            if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
                return false;
            }
            let mut elev: TOKEN_ELEVATION = zeroed();
            let mut len = 0u32;
            let ok = GetTokenInformation(
                token,
                TokenElevation,
                &mut elev as *mut _ as *mut c_void,
                size_of::<TOKEN_ELEVATION>() as u32,
                &mut len,
            );
            CloseHandle(token);
            ok != 0 && elev.TokenIsElevated != 0
        }
    }

    /// Switches on the "increase scheduling priority" right. Administrators hold it, but it starts
    /// switched off, and Windows checks it before raising a process's graphics priority.
    fn enable_priority_privilege() -> &'static str {
        use std::sync::OnceLock;
        static RESULT: OnceLock<&'static str> = OnceLock::new();
        RESULT.get_or_init(|| unsafe {
            let mut token = null_mut();
            if OpenProcessToken(GetCurrentProcess(), TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY, &mut token) == 0 {
                return "token not opened";
            }
            let mut luid: LUID = zeroed();
            let name = to_wide("SeIncreaseBasePriorityPrivilege");
            let mut outcome = "privilege not found";
            if LookupPrivilegeValueW(null(), name.as_ptr(), &mut luid) != 0 {
                let tp = TOKEN_PRIVILEGES {
                    PrivilegeCount: 1,
                    Privileges: [LUID_AND_ATTRIBUTES { Luid: luid, Attributes: SE_PRIVILEGE_ENABLED }],
                };
                const ERROR_NOT_ALL_ASSIGNED: u32 = 1300;
                outcome = if AdjustTokenPrivileges(token, 0, &tp, 0, null_mut(), null_mut()) == 0 {
                    "privilege change failed"
                } else if windows_sys::Win32::Foundation::GetLastError() == ERROR_NOT_ALL_ASSIGNED {
                    "privilege not held"
                } else {
                    "privilege on"
                };
            }
            CloseHandle(token);
            outcome
        })
    }

    /// True when Windows' hardware-accelerated GPU scheduling is on. With it on, realtime graphics
    /// priority can make the whole system stutter, so high is used instead (as other recorders do).
    fn hags_enabled() -> bool {
        use windows_sys::Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_DWORD};
        unsafe {
            let key = to_wide("SYSTEM\\CurrentControlSet\\Control\\GraphicsDrivers");
            let name = to_wide("HwSchMode");
            let mut value: u32 = 0;
            let mut size = size_of::<u32>() as u32;
            let r = RegGetValueW(
                HKEY_LOCAL_MACHINE,
                key.as_ptr(),
                name.as_ptr(),
                RRF_RT_REG_DWORD,
                null_mut(),
                &mut value as *mut u32 as *mut c_void,
                &mut size,
            );
            r == 0 && value == 2
        }
    }

    /// Above-normal CPU priority for the capture process. Works without admin rights.
    pub fn raise_cpu_priority(pid: u32) {
        const ABOVE_NORMAL_PRIORITY_CLASS: u32 = 0x8000;
        const PROCESS_SET_INFORMATION: u32 = 0x0200;
        unsafe {
            let hp = OpenProcess(PROCESS_SET_INFORMATION, 0, pid);
            if !hp.is_null() {
                SetPriorityClass(hp, ABOVE_NORMAL_PRIORITY_CLASS);
                CloseHandle(hp);
            }
        }
    }

    /// One attempt at raising the capture process's graphics priority, so a game using the whole
    /// graphics card doesn't starve the capture. Windows only accepts it once the process has
    /// opened the graphics card, so the caller retries for a few seconds after FFmpeg starts.
    /// Ok and Err both carry a line for the log, error codes included.
    pub fn try_gpu_priority(pid: u32) -> Result<String, String> {
        let top = super::gpu_top();
        type SetGpuPriority = unsafe extern "system" fn(*mut c_void, i32) -> i32;
        const PROCESS_ALL_ACCESS: u32 = 0x001F_FFFF;
        const GPU_HIGH: i32 = 4;
        const GPU_REALTIME: i32 = 5;
        let privilege = enable_priority_privilege();
        let admin = if is_elevated() { "admin" } else { "not admin" };
        let hags = hags_enabled();
        let sched = if hags { "GPU scheduling on" } else { "GPU scheduling off" };
        unsafe {
            let hp = OpenProcess(PROCESS_ALL_ACCESS, 0, pid);
            if hp.is_null() {
                return Err(format!("GPU priority refused (could not open process; {admin}, {privilege})"));
            }
            let gdi = LoadLibraryW(to_wide("gdi32.dll").as_ptr());
            let f = if gdi.is_null() {
                None
            } else {
                GetProcAddress(gdi, b"D3DKMTSetProcessSchedulingPriorityClass\0".as_ptr())
            };
            let result = match f {
                None => Err(format!("GPU priority unavailable ({admin}, {privilege})")),
                Some(f) => {
                    let set: SetGpuPriority = std::mem::transmute(f);
                    // Realtime only with Priority Capture on: at that level the capture
                    // can starve the rest of the system (a full PC freeze was reported with it
                    // on), but high isn't always enough for games that use the whole card.
                    let rt = if top { set(hp as *mut c_void, GPU_REALTIME) } else { -1 };
                    if rt >= 0 {
                        Ok(format!("GPU priority realtime ({admin}, {privilege}, {sched})"))
                    } else {
                        let hi = set(hp as *mut c_void, GPU_HIGH);
                        if hi >= 0 && top {
                            Ok(format!("GPU priority high, realtime refused 0x{rt:08x} ({admin}, {privilege}, {sched})"))
                        } else if hi >= 0 {
                            Ok(format!("GPU priority high ({admin}, {privilege}, {sched})"))
                        } else if hags || !top {
                            Err(format!("GPU priority refused ({admin}, {privilege}, {sched}; high 0x{hi:08x})"))
                        } else {
                            Err(format!(
                                "GPU priority refused ({admin}, {privilege}, {sched}; realtime 0x{rt:08x}, high 0x{hi:08x})"
                            ))
                        }
                    }
                }
            };
            CloseHandle(hp);
            result
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

    /// The window that has the keyboard focus (0 if none).
    pub fn foreground_window() -> u64 {
        unsafe { h2u(GetForegroundWindow()) }
    }

    /// Visible top-level windows of other programs, for game detection. Same filters as
    /// `list_windows`, plus where each one sits and whether it fills its monitor.
    pub fn top_windows() -> Vec<TopWindow> {
        const GWL_STYLE: i32 = -16;
        const WS_CAPTION: isize = 0x00C0_0000;
        let mut handles: Vec<HWND> = Vec::new();
        let mut out = Vec::new();
        let procs = processes();
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
                let mut pid: u32 = 0;
                GetWindowThreadProcessId(h, &mut pid);
                if pid == me || pid == 0 {
                    continue;
                }
                let (w, ht) = client_size(h).unwrap_or((0, 0));
                let minimised = IsIconic(h) != 0;
                if !minimised && (w < 200 || ht < 150) {
                    continue;
                }
                let mut r: RECT = zeroed();
                let mut covers = false;
                if GetWindowRect(h, &mut r) != 0 {
                    let hm = MonitorFromWindow(h, MONITOR_DEFAULTTONEAREST as _);
                    let mut mi: MONITORINFO = zeroed();
                    mi.cbSize = size_of::<MONITORINFO>() as u32;
                    if !hm.is_null() && GetMonitorInfoW(hm, &mut mi) != 0 {
                        let m = mi.rcMonitor;
                        covers = r.left <= m.left && r.top <= m.top && r.right >= m.right && r.bottom >= m.bottom;
                    }
                }
                let captioned = GetWindowLongPtrW(h, GWL_STYLE as _) & WS_CAPTION == WS_CAPTION;
                let exe = procs.get(&pid).map(|p| p.exe.clone()).filter(|e| !e.is_empty()).unwrap_or_else(|| process_exe(pid));
                out.push(TopWindow {
                    handle: h2u(h),
                    pid,
                    title: window_title(h),
                    exe,
                    path: process_path(pid),
                    width: w,
                    height: ht,
                    covers_monitor: covers,
                    captioned,
                    minimised,
                });
            }
        }
        out
    }

    pub fn window_exists(handle: u64) -> bool {
        unsafe { IsWindow(u2h(handle) as HWND) != 0 }
    }

    /// "Windows 11 (10.0.26100.4652)": from `ver`, which every Windows has. Build 22000 and later
    /// is Windows 11.
    pub fn windows_version() -> String {
        let out = crate::ffmpeg::hidden_command(std::path::Path::new("cmd")).args(["/c", "ver"]).output();
        let text = out.map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
        let version = text.rsplit("Version ").next().unwrap_or("").trim_end_matches(']').trim().to_string();
        let build: u32 = version.split('.').nth(2).and_then(|b| b.parse().ok()).unwrap_or(0);
        let name = if build >= 22_000 { "Windows 11" } else if build > 0 { "Windows 10" } else { "Windows" };
        if version.is_empty() { name.into() } else { format!("{name} ({version})") }
    }

    /// Windows says a program has the screen in exclusive fullscreen (Direct3D), the mode that
    /// window capture often can't see into.
    pub fn exclusive_fullscreen() -> bool {
        use windows_sys::Win32::UI::Shell::{SHQueryUserNotificationState, QUNS_RUNNING_D3D_FULL_SCREEN};
        let mut state = 0;
        unsafe { SHQueryUserNotificationState(&mut state) == 0 && state == QUNS_RUNNING_D3D_FULL_SCREEN }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn p(parent: u32, exe: &str) -> ProcInfo {
        ProcInfo { parent, exe: exe.into() }
    }

    #[test]
    fn app_root_climbs_to_the_top_process_of_the_same_app() {
        let procs: HashMap<u32, ProcInfo> = HashMap::from([
            (1, p(0, "explorer.exe")),
            (10, p(1, "chrome.exe")),
            (11, p(10, "chrome.exe")),
            (12, p(11, "Chrome.exe")),
            (20, p(1, "Spotify.exe")),
            (30, p(1, "steam.exe")),
            (31, p(30, "game.exe")),
        ]);
        assert_eq!(app_root(12, &procs), 10); // Chrome's audio helper -> Chrome
        assert_eq!(app_root(20, &procs), 20);
        assert_eq!(app_root(31, &procs), 31); // a game launched by Steam is its own app
        assert_eq!(app_root(999, &procs), 999); // unknown process
        let cyc: HashMap<u32, ProcInfo> = HashMap::from([(5, p(6, "a.exe")), (6, p(5, "a.exe"))]);
        let r = app_root(5, &cyc);
        assert!(r == 5 || r == 6); // parent loops don't hang
    }

    #[test]
    fn focus_rules() {
        assert_eq!(focus_rule("Chrome_WidgetWin_1", "Discord.exe"), FocusRule::UseWindow);
        assert_eq!(focus_rule("UnrealWindow", "Game-Win64-Shipping.exe"), FocusRule::UseWindow);
        assert_eq!(focus_rule("Chrome_WidgetWin_1", "Encore.exe"), FocusRule::Ignore);
        assert_eq!(focus_rule("Progman", "explorer.exe"), FocusRule::UseCursor);
        assert_eq!(focus_rule("Shell_SecondaryTrayWnd", "explorer.exe"), FocusRule::UseCursor);
        assert_eq!(focus_rule("XamlExplorerHostIslandWindow", "explorer.exe"), FocusRule::Ignore);
        assert_eq!(focus_rule("CabinetWClass", "explorer.exe"), FocusRule::UseWindow);
    }

    #[test]
    fn exe_stems() {
        assert_eq!(exe_stem(r"C:\Games\VALORANT-Win64-Shipping.exe"), "VALORANT-Win64-Shipping");
        assert_eq!(exe_stem("Spotify.exe"), "Spotify");
        assert_eq!(exe_stem("noext"), "noext");
        assert_eq!(exe_stem(".hidden"), ".hidden");
    }
}
