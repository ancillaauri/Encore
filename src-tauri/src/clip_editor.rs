//! Opening saved clips: in Backstage (the companion editor, called Clip Editor before 1.48) or in
//! Windows' video player. Backstage opens a video it's started with, and a second start hands the video to the
//! copy that's already open, so all Encore has to do is find it and start it with the clip.

use std::path::{Path, PathBuf};

/// The editor's program and its name in Windows' list of installed apps: Backstage first, then
/// its old name (its installer removes Clip Editor, but an older install may still be there).
const EXES: [&str; 2] = ["Backstage.exe", "Clip Editor.exe"];
#[cfg(windows)]
const NAMES: [&str; 2] = ["Backstage", "Clip Editor"];
const FOLDERS: [&str; 4] = ["backstage", "Backstage", "clip-editor", "Clip Editor"];

/// Where the editor is installed, if it can be found: Windows' list of installed apps first
/// (that's where a changed install folder is recorded), then the usual folders. Backstage is
/// preferred over an old Clip Editor.
#[tauri::command]
pub fn locate_clip_editor() -> Option<String> {
    let mut places: Vec<PathBuf> = installed_places();
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        places.extend(FOLDERS.iter().map(|f| Path::new(&local).join("Programs").join(f)));
    }
    for var in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Ok(pf) = std::env::var(var) {
            places.extend(FOLDERS.iter().map(|f| Path::new(&pf).join(f)));
        }
    }
    find_exe(&places).map(|exe| exe.display().to_string())
}

/// The first editor program in `places`, Backstage before Clip Editor.
fn find_exe(places: &[PathBuf]) -> Option<PathBuf> {
    EXES.iter().find_map(|name| places.iter().map(|dir| dir.join(name)).find(|exe| exe.is_file()))
}

/// Windows' file picker for Clip Editor's .exe. Returns the chosen file, or None if cancelled.
#[tauri::command]
pub async fn pick_clip_editor(app: tauri::AppHandle) -> Option<String> {
    use tauri::Manager;
    use tauri_plugin_dialog::DialogExt;
    let mut dialog = app.dialog().file().set_title("Find Backstage").add_filter("Backstage", &["exe"]);
    if let Some(win) = app.get_webview_window("main") {
        dialog = dialog.set_parent(&win);
    }
    dialog.blocking_pick_file().and_then(|p| p.into_path().ok()).map(|p| p.display().to_string())
}

fn check_clip(path: &str) -> Result<(), String> {
    let p = Path::new(path);
    let mp4 = p.extension().map(|e| e.eq_ignore_ascii_case("mp4")).unwrap_or(false);
    if !mp4 || !p.is_file() {
        return Err("That clip isn't there any more.".into());
    }
    Ok(())
}

/// Opens the clip in Backstage, ready to edit.
#[tauri::command]
pub fn open_in_editor(exe: String, path: String) -> Result<(), String> {
    check_clip(&path)?;
    let e = Path::new(&exe);
    let is_exe = e.extension().map(|x| x.eq_ignore_ascii_case("exe")).unwrap_or(false);
    if !is_exe || !e.is_file() {
        return Err("Backstage wasn't found. Find it in Settings → Files.".into());
    }
    std::process::Command::new(e).arg(&path).spawn().map(|_| ()).map_err(|err| format!("Couldn't open Backstage: {err}"))
}

/// Opens the clip in whatever Windows uses for videos.
#[tauri::command]
pub fn open_in_player(path: String) -> Result<(), String> {
    check_clip(&path)?;
    #[cfg(windows)]
    {
        std::process::Command::new("explorer").arg(&path).spawn().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Install folders of Backstage (or Clip Editor) in Windows' list of installed apps.
#[cfg(windows)]
fn installed_places() -> Vec<PathBuf> {
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegEnumKeyExW, RegGetValueW, RegOpenKeyExW, HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, RRF_RT_REG_SZ,
    };
    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }
    unsafe fn text(key: HKEY, name: &str) -> Option<String> {
        let mut buf = vec![0u16; 1024];
        let mut bytes = (buf.len() * 2) as u32;
        let r = RegGetValueW(key, std::ptr::null(), wide(name).as_ptr(), RRF_RT_REG_SZ, std::ptr::null_mut(), buf.as_mut_ptr().cast(), &mut bytes);
        if r != 0 {
            return None;
        }
        let len = buf.iter().position(|&c| c == 0).unwrap_or(0);
        Some(String::from_utf16_lossy(&buf[..len]))
    }
    let mut out = Vec::new();
    let roots = [
        (HKEY_CURRENT_USER, r"Software\Microsoft\Windows\CurrentVersion\Uninstall"),
        (HKEY_LOCAL_MACHINE, r"Software\Microsoft\Windows\CurrentVersion\Uninstall"),
        (HKEY_LOCAL_MACHINE, r"Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall"),
    ];
    for (root, path) in roots {
        unsafe {
            let mut list: HKEY = std::ptr::null_mut();
            if RegOpenKeyExW(root, wide(path).as_ptr(), 0, KEY_READ, &mut list) != 0 {
                continue;
            }
            let mut i = 0;
            loop {
                let mut name = [0u16; 256];
                let mut len = name.len() as u32;
                let r = RegEnumKeyExW(list, i, name.as_mut_ptr(), &mut len, std::ptr::null(), std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut());
                if r != 0 {
                    break;
                }
                i += 1;
                let mut app: HKEY = std::ptr::null_mut();
                if RegOpenKeyExW(list, name.as_ptr(), 0, KEY_READ, &mut app) != 0 {
                    continue;
                }
                if text(app, "DisplayName").map(|n| NAMES.iter().any(|name| n.starts_with(name))).unwrap_or(false) {
                    if let Some(dir) = text(app, "InstallLocation").filter(|d| !d.is_empty()) {
                        out.push(PathBuf::from(dir.trim_matches('"')));
                    }
                    // The uninstaller sits in the install folder too.
                    if let Some(u) = text(app, "UninstallString") {
                        let u = u.trim().trim_start_matches('"');
                        if let Some(end) = u.to_ascii_lowercase().find(".exe") {
                            if let Some(dir) = Path::new(&u[..end + 4]).parent() {
                                out.push(dir.to_path_buf());
                            }
                        }
                    }
                }
                RegCloseKey(app);
            }
            RegCloseKey(list);
        }
    }
    out
}

#[cfg(not(windows))]
fn installed_places() -> Vec<PathBuf> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backstage_is_found_before_an_old_clip_editor() {
        let base = std::env::temp_dir().join(format!("encore-editor-{}", std::process::id()));
        let old = base.join("Clip Editor");
        let new = base.join("backstage");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::create_dir_all(&new).unwrap();
        std::fs::write(old.join("Clip Editor.exe"), b"x").unwrap();
        assert_eq!(find_exe(&[old.clone(), new.clone()]), Some(old.join("Clip Editor.exe")));
        std::fs::write(new.join("Backstage.exe"), b"x").unwrap();
        assert_eq!(find_exe(&[old.clone(), new.clone()]), Some(new.join("Backstage.exe")));
        let _ = std::fs::remove_dir_all(&base);
    }
}
