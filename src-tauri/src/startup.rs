//! "Start Encore when Windows starts": a value in the current user's Run key that opens Encore in
//! the tray (`--autostart`), so hotkeys and instant replay are ready without the window appearing.
//! With "Start Encore as administrator" on, that copy hands over to the elevated one as usual, and
//! a marker file tells the elevated copy to stay in the tray too.

#![cfg_attr(not(windows), allow(dead_code))]

use std::path::PathBuf;

pub const AUTOSTART_ARG: &str = "--autostart";
const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
const VALUE: &str = "Encore";

fn hidden_marker() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join("Encore").join("start-hidden"))
}

fn started_by_windows() -> bool {
    std::env::args().any(|a| a == AUTOSTART_ARG)
}

/// Called before handing over to the elevated copy: pass on "stay in the tray".
pub fn remember_hidden_for_handover() {
    if started_by_windows() {
        if let Some(m) = hidden_marker() {
            let _ = std::fs::write(m, b"1");
        }
    }
}

/// True if this copy should start in the tray (started with Windows, directly or via hand-over).
/// Reading the marker removes it.
pub fn start_hidden() -> bool {
    let handed = hidden_marker()
        .map(|m| {
            let fresh = std::fs::metadata(&m)
                .and_then(|md| md.modified())
                .ok()
                .and_then(|t| t.elapsed().ok())
                .map(|age| age.as_secs() < 60)
                .unwrap_or(false);
            let existed = m.exists();
            let _ = std::fs::remove_file(&m);
            existed && fresh
        })
        .unwrap_or(false);
    started_by_windows() || handed
}

#[cfg(windows)]
fn reg(args: &[&str]) -> bool {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    std::process::Command::new("reg.exe")
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn enabled() -> bool {
    #[cfg(windows)]
    {
        reg(&["query", RUN_KEY, "/v", VALUE])
    }
    #[cfg(not(windows))]
    {
        false
    }
}

#[tauri::command]
pub async fn autostart_status() -> bool {
    tauri::async_runtime::spawn_blocking(enabled).await.unwrap_or(false)
}

#[tauri::command]
pub async fn set_autostart(on: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        tauri::async_runtime::spawn_blocking(move || {
            let ok = if on {
                let exe = std::env::current_exe().map_err(|e| e.to_string())?;
                let exe = exe.to_string_lossy().trim_start_matches(r"\\?\").to_string();
                let data = format!("\"{exe}\" {AUTOSTART_ARG}");
                reg(&["add", RUN_KEY, "/v", VALUE, "/t", "REG_SZ", "/d", &data, "/f"])
            } else {
                !enabled() || reg(&["delete", RUN_KEY, "/v", VALUE, "/f"])
            };
            if ok { Ok(()) } else { Err("Windows couldn't change the start-up setting.".to_string()) }
        })
        .await
        .map_err(|e| e.to_string())?
    }
    #[cfg(not(windows))]
    {
        let _ = on;
        Err("Only available on Windows.".into())
    }
}
