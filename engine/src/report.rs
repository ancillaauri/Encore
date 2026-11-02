//! What goes with a problem report (Main Menu → Help → Report a problem), shown to the person
//! before it's sent: Encore's recent log and a few PC details. Folders under the Windows user
//! folder have the user name taken out, so a report never says who someone is.

use crate::paths;
use serde_json::{json, Value};

/// How much of the log goes with a report: enough to cover the last few minutes of use.
const LOG_LINES: usize = 300;

pub fn details(settings: &crate::settings::Settings, encoders: &[String]) -> Value {
    let log = std::fs::read_to_string(paths::log_dir().join("engine.log")).unwrap_or_default();
    let lines: Vec<&str> = log.lines().collect();
    let tail = lines[lines.len().saturating_sub(LOG_LINES)..].join("\n");
    let s = settings;
    let name = |v: Value| v.as_str().map(String::from).unwrap_or_else(|| v.to_string());
    let summary = format!(
        "{} {} fps, {}, {}, game detection {}, Desktop Capture {}, Replay {} ({} s), microphone {} (noise reduction {}), Split Application Audio {}, Priority Capture {}",
        name(json!(s.codec)),
        s.fps,
        name(json!(s.resolution)),
        if s.bitrate_custom { format!("{} Mbps set by hand", s.bitrate_mbps) } else { format!("Quality {}", name(json!(s.quality))) },
        on(s.game_detection),
        on(s.desktop_capture),
        on(s.replay_on),
        s.replay_seconds,
        on(s.audio_mic),
        s.mic_denoise,
        on(s.audio_app_tracks),
        on(s.gpu_realtime),
    );
    json!({
        "windows": crate::platform::windows_version(),
        "administrator": crate::platform::is_elevated(),
        "encoders": encoders,
        "settings": summary,
        "log": scrub(&tail),
    })
}

fn on(b: bool) -> &'static str {
    if b { "on" } else { "off" }
}

/// Takes the user name out of paths: `C:\Users\Adam\Videos` → `C:\Users\<user>\Videos`.
pub fn scrub(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = find_users(rest) {
        let (before, after) = rest.split_at(i + "\\Users\\".len());
        out.push_str(before);
        // A user name can have spaces in it: it ends at the next folder (or quote or line end).
        let end = after.find(['\\', '/', '"', '\'', '\n', ')', ']']).unwrap_or(after.len());
        out.push_str("<user>");
        rest = &after[end..];
    }
    out.push_str(rest);
    out
}

fn find_users(s: &str) -> Option<usize> {
    s.to_ascii_lowercase().find("\\users\\")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_names_are_taken_out_of_paths() {
        assert_eq!(scrub(r"saved C:\Users\Newbi\Videos\x.mp4 and \\?\C:\users\Newbi\AppData"), r"saved C:\Users\<user>\Videos\x.mp4 and \\?\C:\users\<user>\AppData");
        assert_eq!(scrub(r"D:\Encore\Warframe\clip.mp4"), r"D:\Encore\Warframe\clip.mp4");
        assert_eq!(scrub(r"C:\Users\Jo Smith\x"), r"C:\Users\<user>\x");
    }
}
