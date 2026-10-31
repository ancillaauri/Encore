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
    /// Desktop mode monitor: "primary", "mouse", or "id:<stable monitor id>".
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
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            output_dir: paths::default_output_dir().to_string_lossy().into_owned(),
            codec: Codec::H264,
            fps: 60,
            resolution: Resolution::Native,
            quality: Quality::High,
            capture_cursor: true,
            monitor: "primary".into(),
            audio_full_mix: true,
            audio_mic: true,
            mic_device: "default".into(),
            audio_app_tracks: true,
            audio_offset_ms: 0,
            update_mode: "manual".into(),
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
        if updated.mic_device.trim().is_empty() {
            return Err("Microphone cannot be empty".into());
        }
        let m = updated.monitor.as_str();
        if !(m == "primary" || m == "mouse" || (m.starts_with("id:") && m.len() > 3)) {
            return Err("Monitor must be primary, mouse, or a monitor id".into());
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
        assert_eq!(s.monitor, "primary");
        assert!(s.audio_full_mix && s.audio_mic && s.audio_app_tracks);
        assert_eq!(s.mic_device, "default");
        assert_eq!(s.update_mode, "manual"); // no background downloads unless chosen
    }
}
