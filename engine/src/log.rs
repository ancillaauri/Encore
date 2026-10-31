//! Engine log: %LOCALAPPDATA%\Encore\logs\engine.log, plus stderr. Never stdout (that is the protocol).

use crate::paths;
use std::io::Write;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

static LOCK: Mutex<()> = Mutex::new(());
const MAX_LOG_BYTES: u64 = 2 * 1024 * 1024;

pub fn init() {
    let dir = paths::log_dir();
    let _ = std::fs::create_dir_all(&dir);
    let file = dir.join("engine.log");
    // Keep one previous log; start fresh when the current one gets large.
    if std::fs::metadata(&file).map(|m| m.len() > MAX_LOG_BYTES).unwrap_or(false) {
        let _ = std::fs::rename(&file, dir.join("engine.previous.log"));
    }
}

pub fn log(msg: &str) {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let line = format!("[{secs}] {msg}\n");
    let _guard = LOCK.lock();
    eprint!("{line}");
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(paths::log_dir().join("engine.log"))
    {
        let _ = f.write_all(line.as_bytes());
    }
}

/// Last `max_lines` non-empty lines of a text file (for error messages).
pub fn tail(path: &std::path::Path, max_lines: usize) -> String {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    let start = lines.len().saturating_sub(max_lines);
    lines[start..].join("\n")
}
