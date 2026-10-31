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

pub fn default_output_dir() -> PathBuf {
    home().join("Videos").join("Encore")
}
