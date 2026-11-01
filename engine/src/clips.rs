//! Renaming and deleting saved clips (from "Saved this session"). Only video files inside
//! Encore's own folder can be touched. Deleting sends the file to the Recycle Bin, so a clip
//! deleted by mistake can still be got back after the interface's 5-second undo.

use crate::recorder::sanitise;
use std::path::{Path, PathBuf};

/// The clip's real path, if it is an .mp4 inside `root`.
fn inside(root: &Path, path: &Path) -> Result<PathBuf, String> {
    let file = path.canonicalize().map_err(|_| "That clip isn't there any more.".to_string())?;
    let root = root.canonicalize().map_err(|_| "Encore's folder couldn't be found.".to_string())?;
    let is_mp4 = file.extension().map(|e| e.eq_ignore_ascii_case("mp4")).unwrap_or(false);
    if !file.starts_with(&root) || !is_mp4 || !file.is_file() {
        return Err("Only clips in Encore's folder can be renamed or deleted here.".into());
    }
    Ok(file)
}

/// A new name for the clip (without ".mp4"), in the same folder. Returns the new path.
pub fn rename(root: &Path, path: &Path, new_name: &str) -> Result<PathBuf, String> {
    let file = inside(root, path)?;
    let wanted = new_name.trim();
    let wanted = wanted.strip_suffix(".mp4").or_else(|| wanted.strip_suffix(".MP4")).unwrap_or(wanted);
    if wanted.trim().is_empty() {
        return Err("Type a name for the clip.".into());
    }
    let clean = sanitise(wanted);
    let target = file.with_file_name(format!("{clean}.mp4"));
    if target == file {
        return Ok(file);
    }
    // Same name apart from capital letters: Windows sees that as the same file, so allow it.
    let same_file = target.to_string_lossy().eq_ignore_ascii_case(&file.to_string_lossy());
    if target.exists() && !same_file {
        return Err(format!("There's already a clip called \"{clean}\" in that folder."));
    }
    let target = plain(&target);
    std::fs::rename(&file, &target).map_err(|e| {
        if e.raw_os_error() == Some(32) {
            "The clip is open in another program. Close it there and try again.".to_string()
        } else {
            format!("Couldn't rename the clip: {e}")
        }
    })?;
    Ok(target)
}

/// Without the long-path prefix Windows adds when resolving a path (`\\?\`), as people see it.
fn plain(p: &Path) -> PathBuf {
    let s = p.to_string_lossy();
    PathBuf::from(s.strip_prefix(r"\\?\").unwrap_or(&s).to_string())
}

/// Sends the clip to the Recycle Bin.
pub fn delete(root: &Path, path: &Path) -> Result<(), String> {
    let file = inside(root, path)?;
    to_recycle_bin(&file)
}

#[cfg(windows)]
fn to_recycle_bin(file: &Path) -> Result<(), String> {
    use windows_sys::Win32::UI::Shell::{
        SHFileOperationW, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT, FO_DELETE, SHFILEOPSTRUCTW,
    };
    // The path list ends with two zeros.
    let mut from: Vec<u16> = file.as_os_str().to_string_lossy().trim_start_matches(r"\\?\").encode_utf16().collect();
    from.extend([0, 0]);
    unsafe {
        let mut op: SHFILEOPSTRUCTW = std::mem::zeroed();
        op.wFunc = FO_DELETE;
        op.pFrom = from.as_ptr();
        op.fFlags = (FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_NOERRORUI | FOF_SILENT) as u16;
        let r = SHFileOperationW(&mut op);
        if r != 0 || op.fAnyOperationsAborted != 0 {
            return Err(if r == 32 || r == 0x20 {
                "The clip is open in another program. Close it there and try again.".into()
            } else {
                format!("Couldn't delete the clip (error {r}).")
            });
        }
    }
    Ok(())
}

#[cfg(not(windows))]
fn to_recycle_bin(file: &Path) -> Result<(), String> {
    std::fs::remove_file(file).map_err(|e| format!("Couldn't delete the clip: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rename_and_delete_stay_inside_the_folder() {
        let base = std::env::temp_dir().join(format!("encore-clips-{}", std::process::id()));
        let root = base.join("Encore");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(root.join("Warframe")).unwrap();
        let clip = root.join("Warframe").join("Warframe 2026.11.01 - 21.52.58.mp4");
        std::fs::write(&clip, b"x").unwrap();
        let outside = base.join("notes.mp4");
        std::fs::write(&outside, b"x").unwrap();

        // Renamed in place; bad characters cleaned; ".mp4" typed by the person is fine.
        let renamed = rename(&root, &clip, "Big: win?.mp4").unwrap();
        assert_eq!(renamed.file_name().unwrap().to_string_lossy(), "Big win.mp4");
        assert!(!clip.exists() && renamed.exists());
        // Empty names and names already taken are refused.
        assert!(rename(&root, &renamed, "   ").is_err());
        std::fs::write(root.join("Warframe").join("Taken.mp4"), b"x").unwrap();
        assert!(rename(&root, &renamed, "Taken").is_err());
        // Nothing outside Encore's folder, and only videos.
        assert!(rename(&root, &outside, "x").is_err());
        assert!(delete(&root, &outside).is_err());
        std::fs::write(root.join("Warframe").join("a.txt"), b"x").unwrap();
        assert!(delete(&root, &root.join("Warframe").join("a.txt")).is_err());
        // Deleted.
        delete(&root, &renamed).unwrap();
        assert!(!renamed.exists());
        assert!(outside.exists());
        let _ = std::fs::remove_dir_all(&base);
    }
}
