//! Standard folders. Windows: %APPDATA%\Encore (settings), %LOCALAPPDATA%\Encore (temp, logs),
//! %USERPROFILE%\Videos\Encore (recordings). Elsewhere (tests only): under $HOME/.encore.

use std::path::PathBuf;

fn env_dir(var: &str) -> Option<PathBuf> {
    std::env::var_os(var).filter(|v| !v.is_empty()).map(PathBuf::from)
}

fn home() -> PathBuf {
    env_dir("USERPROFILE")
        .or_else(|| env_dir("HOME"))
        .unwrap_or_else(std::env::temp_dir)
}

pub fn config_dir() -> PathBuf {
    if cfg!(windows) {
        env_dir("APPDATA").unwrap_or_else(home).join("Encore")
    } else {
        home().join(".encore").join("config")
    }
}

pub fn local_dir() -> PathBuf {
    if cfg!(windows) {
        env_dir("LOCALAPPDATA").unwrap_or_else(home).join("Encore")
    } else {
        home().join(".encore").join("local")
    }
}

pub fn temp_dir() -> PathBuf {
    local_dir().join("temp")
}

pub fn log_dir() -> PathBuf {
    local_dir().join("logs")
}

/// Where recordings go by default: the Videos folder (Encore adds its own folder inside, see
/// `encore_root`).
pub fn default_output_dir() -> PathBuf {
    crate::platform::videos_folder().unwrap_or_else(|| home().join("Videos"))
}

/// Defaults used before Encore asked Windows for the real Videos folder. A setting still on one of
/// these (and never confirmed in the setup) is moved to the real Videos folder.
pub fn legacy_default_dirs() -> [PathBuf; 2] {
    [home().join("Videos"), home().join("Videos").join("Encore")]
}

/// The Encore folder inside the chosen save folder: `D:\Clips` -> `D:\Clips\Encore`. A folder
/// that is already called Encore is used as it is (no `Encore\Encore`), which also keeps the old
/// default (`Videos\Encore`) working unchanged.
pub fn encore_root(chosen: &std::path::Path) -> PathBuf {
    let named_encore = chosen
        .file_name()
        .map(|n| n.to_string_lossy().eq_ignore_ascii_case("encore"))
        .unwrap_or(false);
    if named_encore { chosen.to_path_buf() } else { chosen.join("Encore") }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn encore_folder_is_made_inside_the_chosen_folder() {
        assert_eq!(encore_root(Path::new("/d/Program Files")), Path::new("/d/Program Files/Encore"));
        assert_eq!(encore_root(Path::new("/home/a/Videos/Encore")), Path::new("/home/a/Videos/Encore"));
        assert_eq!(encore_root(Path::new("/home/a/Videos/encore")), Path::new("/home/a/Videos/encore"));
        assert_eq!(encore_root(Path::new("/clips/Encores")), Path::new("/clips/Encores/Encore"));
    }
}
