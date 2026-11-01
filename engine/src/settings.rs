//! User settings, stored as JSON in %APPDATA%\Encore\settings.json.

use crate::paths;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Codec {
    H264,
    Hevc,
    Av1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Resolution {
    Native,
    #[serde(rename = "1440p")]
    P1440,
    #[serde(rename = "1080p")]
    P1080,
    #[serde(rename = "720p")]
    P720,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Quality {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Settings {
    /// Where finished recordings are saved (one subfolder per game/source).
    pub output_dir: String,
    pub codec: Codec,
    pub fps: u32,
    pub resolution: Resolution,
    pub quality: Quality,
    pub capture_cursor: bool,
    /// Desktop mode monitor: "follow" (the monitor of the window you click on, switching during a
    /// recording) or "id:<stable monitor id>" (always that monitor).
    pub monitor: String,
    /// Record everything you hear as the "Full mix" track.
    pub audio_full_mix: bool,
    /// Record the microphone as its own track.
    pub audio_mic: bool,
    /// "default" or a microphone id from `list_audio_inputs`.
    pub mic_device: String,
    /// Give each app that plays sound its own track.
    pub audio_app_tracks: bool,
    /// Fine-tunes audio/video sync in milliseconds (positive = audio later). Normally 0.
    pub audio_offset_ms: i32,
    /// App updates: "manual" (check and tell me; I download and install), "auto_download"
    /// (download in the background; I choose when to install), or "never" (no checks).
    /// Read by the app shell, which owns updating.
    pub update_mode: String,
    /// The first-run setup has been completed (or skipped).
    pub setup_done: bool,
    /// Closing the window keeps Encore running in the tray (so hotkeys work) instead of quitting.
    pub close_to_tray: bool,
    /// Global hotkey to start/stop recording, e.g. "Ctrl+Alt+R"; empty = none. Read by the shell.
    pub hotkey_record: String,
    /// Play a short sound when recording starts, stops and saves.
    pub sounds: bool,
    /// Instant replay: how many seconds a save keeps (the buffer holds a little more).
    pub replay_seconds: u32,
    /// Instant replay was on when Encore last closed (the interface turns it back on).
    pub replay_on: bool,
    /// Global hotkey to save the replay; empty = none. Read by the shell.
    pub hotkey_save_replay: String,
}

/// Replay lengths offered: 30 s to 20 minutes.
pub const REPLAY_SECONDS: [u32; 7] = [30, 60, 120, 300, 600, 900, 1200];

impl Default for Settings {
    fn default() -> Self {
        Settings {
            output_dir: paths::default_output_dir().to_string_lossy().into_owned(),
            codec: Codec::H264,
            fps: 60,
            resolution: Resolution::Native,
            quality: Quality::High,
            capture_cursor: true,
            monitor: "follow".into(),
            audio_full_mix: true,
            audio_mic: true,
            mic_device: "default".into(),
            audio_app_tracks: true,
            audio_offset_ms: 0,
            update_mode: "manual".into(),
            setup_done: false,
            close_to_tray: true,
            // Not Alt+F9/F10 (NVIDIA's recorder) or Alt+Z/Alt+R (its overlay).
            hotkey_record: "Ctrl+Alt+R".into(),
            sounds: true,
            replay_seconds: 120,
            replay_on: false,
            hotkey_save_replay: "Ctrl+Alt+S".into(),
        }
    }
}

pub const ALLOWED_FPS: [u32; 5] = [30, 60, 120, 144, 240];

impl Settings {
    pub fn file() -> PathBuf {
        paths::config_dir().join("settings.json")
    }

    pub fn load() -> Settings {
        match std::fs::read_to_string(Self::file()) {
            Ok(text) => serde_json::from_str::<Settings>(&text)
                .map(|s| s.normalised())
                .unwrap_or_default(),
            Err(_) => Settings::default(),
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::file();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("Could not create settings folder: {e}"))?;
        }
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&path, text).map_err(|e| format!("Could not save settings: {e}"))
    }

    /// Applies a partial update (only the keys present in `patch`). Rejects invalid values.
    pub fn apply_patch(&self, patch: &Value) -> Result<Settings, String> {
        let obj = patch.as_object().ok_or("Settings update must be an object")?;
        let mut current = serde_json::to_value(self).map_err(|e| e.to_string())?;
        let cur = current.as_object_mut().expect("settings serialise to an object");
        for (k, v) in obj {
            if !cur.contains_key(k) {
                return Err(format!("Unknown setting: {k}"));
            }
            cur.insert(k.clone(), v.clone());
        }
        let updated: Settings =
            serde_json::from_value(current).map_err(|e| format!("Invalid setting value: {e}"))?;
        if !ALLOWED_FPS.contains(&updated.fps) {
            return Err(format!("Frame rate must be one of {:?}", ALLOWED_FPS));
        }
        if updated.output_dir.trim().is_empty() {
            return Err("Output folder cannot be empty".into());
        }
        if !(-1000..=1000).contains(&updated.audio_offset_ms) {
            return Err("Audio offset must be between -1000 and 1000 ms".into());
        }
        if !["manual", "auto_download", "never"].contains(&updated.update_mode.as_str()) {
            return Err("Update mode must be manual, auto_download or never".into());
        }
        if !REPLAY_SECONDS.contains(&updated.replay_seconds) {
            return Err(format!("Replay length must be one of {:?} seconds", REPLAY_SECONDS));
        }
        if updated.hotkey_save_replay.len() > 64 {
            return Err("Hotkey is too long".into());
        }
        if updated.hotkey_record.len() > 64 {
            return Err("Hotkey is too long".into());
        }
        if updated.mic_device.trim().is_empty() {
            return Err("Microphone cannot be empty".into());
        }
        let m = updated.monitor.as_str();
        if !(m == "follow" || (m.starts_with("id:") && m.len() > 3)) {
            return Err("Monitor must be follow or a monitor id".into());
        }
        Ok(updated)
    }

    fn normalised(mut self) -> Settings {
        if !ALLOWED_FPS.contains(&self.fps) {
            self.fps = 60;
        }
        if self.output_dir.trim().is_empty() {
            self.output_dir = Settings::default().output_dir;
        }
        if !["manual", "auto_download", "never"].contains(&self.update_mode.as_str()) {
            self.update_mode = "manual".into();
        }
        if !REPLAY_SECONDS.contains(&self.replay_seconds) {
            self.replay_seconds = 120;
        }
        // Not set up yet and still on an old guessed default: use the real Videos folder (it may be
        // on another drive or in OneDrive, where the guess made a new, empty folder).
        if !self.setup_done && paths::legacy_default_dirs().iter().any(|d| d.to_string_lossy() == self.output_dir) {
            self.output_dir = paths::default_output_dir().to_string_lossy().into_owned();
        }
        // Before 0.5: "primary" (the old default) and "mouse" (chosen at start only).
        if !(self.monitor == "follow" || (self.monitor.starts_with("id:") && self.monitor.len() > 3)) {
            self.monitor = "follow".into();
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn patch_updates_only_given_keys() {
        let s = Settings::default();
        let u = s.apply_patch(&json!({"fps": 144, "codec": "hevc"})).unwrap();
        assert_eq!(u.fps, 144);
        assert_eq!(u.codec, Codec::Hevc);
        assert_eq!(u.quality, s.quality);
    }

    #[test]
    fn patch_rejects_bad_values() {
        let s = Settings::default();
        assert!(s.apply_patch(&json!({"fps": 59})).is_err());
        assert!(s.apply_patch(&json!({"codec": "vp9"})).is_err());
        assert!(s.apply_patch(&json!({"nonsense": 1})).is_err());
        assert!(s.apply_patch(&json!({"monitor": "left"})).is_err());
        assert!(s.apply_patch(&json!({"monitor": "mouse"})).is_err());
        assert!(s.apply_patch(&json!({"monitor": "follow"})).is_ok());
        assert!(s.apply_patch(&json!({"output_dir": "  "})).is_err());
        assert!(s.apply_patch(&json!([1, 2])).is_err());
        assert!(s.apply_patch(&json!({"audio_offset_ms": 5000})).is_err());
        assert!(s.apply_patch(&json!({"audio_mic": "yes"})).is_err());
        assert!(s.apply_patch(&json!({"audio_mic": false, "audio_offset_ms": -40})).is_ok());
        assert!(s.apply_patch(&json!({"update_mode": "sometimes"})).is_err());
        assert_eq!(s.apply_patch(&json!({"update_mode": "never"})).unwrap().update_mode, "never");
    }

    #[test]
    fn resolution_names_round_trip() {
        let s = Settings::default().apply_patch(&json!({"resolution": "1080p"})).unwrap();
        assert_eq!(s.resolution, Resolution::P1080);
        assert_eq!(serde_json::to_value(&s).unwrap()["resolution"], "1080p");
    }

    #[test]
    fn missing_keys_fall_back_to_defaults() {
        let s: Settings = serde_json::from_str(r#"{"fps":30}"#).unwrap();
        assert_eq!(s.fps, 30);
        assert_eq!(s.codec, Codec::H264);
        assert_eq!(s.monitor, "follow");
        for old in ["primary", "mouse", ""] {
            let o: Settings = serde_json::from_value(serde_json::json!({ "monitor": old })).unwrap();
            assert_eq!(o.normalised().monitor, "follow");
        }
        let o: Settings = serde_json::from_value(serde_json::json!({ "monitor": "id:abc" })).unwrap();
        assert_eq!(o.normalised().monitor, "id:abc");
        assert!(s.audio_full_mix && s.audio_mic && s.audio_app_tracks);
        assert_eq!(s.mic_device, "default");
        assert_eq!(s.update_mode, "manual"); // no background downloads unless chosen
        assert!(!s.setup_done); // settings saved before the setup existed still show it once
        assert!(s.close_to_tray && s.sounds);
        assert_eq!(s.hotkey_record, "Ctrl+Alt+R");
        assert_eq!((s.replay_seconds, s.replay_on), (120, false));
        assert_eq!(s.hotkey_save_replay, "Ctrl+Alt+S");
    }
}
