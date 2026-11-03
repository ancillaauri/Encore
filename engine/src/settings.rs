//! User settings, stored as JSON in %APPDATA%\Encore\settings.json.

use crate::paths;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;

/// The range for "Set the bitrate myself", in Mbps: from what still looks right at 720p 30 fps
/// to what no game recording needs more than (and what every hardware encoder accepts).
pub const BITRATE_MBPS_MIN: u32 = 5;
pub const BITRATE_MBPS_MAX: u32 = 150;

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
    Ultra,
    High,
    Medium,
    Low,
}

/// A problem report this PC sent: its number, the key that lets this PC (and nobody else) read
/// the replies, its title, when it was sent (Unix seconds), and how many replies have been seen.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct SentReport {
    pub number: u64,
    pub key: String,
    pub title: String,
    pub sent: u64,
    pub seen: u32,
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
    /// Desktop mode monitor: "primary" (the default: whichever monitor is the primary one),
    /// "id:<stable monitor id>" (always that monitor) or "follow" (the monitor of the window you
    /// click on, switching during a recording).
    pub monitor: String,
    /// Record everything you hear as the "Full mix" track.
    pub audio_full_mix: bool,
    /// Record the microphone as its own track.
    pub audio_mic: bool,
    /// "default" or a microphone id from `list_audio_inputs`.
    pub mic_device: String,
    /// The chosen microphone's name, so it can be found again when Windows gives it a new id
    /// (a driver update, another USB port). Empty for "default".
    pub mic_name: String,
    /// Background noise reduction on the microphone when a clip is saved: "off", "light" or
    /// "strong".
    pub mic_denoise: String,
    /// "Set the bitrate myself": use `bitrate_mbps` instead of the Quality preset.
    pub bitrate_custom: bool,
    /// The bitrate chosen by hand, in Mbps (BITRATE_MBPS_MIN..=BITRATE_MBPS_MAX).
    pub bitrate_mbps: u32,
    /// Problems reported from Encore (Help → Report a problem), so replies can be shown.
    pub reports: Vec<SentReport>,
    /// Give each app that plays sound its own track.
    pub audio_app_tracks: bool,
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
    /// Global hotkeys to save just the last 30 seconds / 1 minute / the custom length.
    pub hotkey_save_30s: String,
    pub hotkey_save_60s: String,
    pub hotkey_save_custom: String,
    /// Do not disturb: Encore's own notices stay hidden (the side panel's moon button).
    pub do_not_disturb: bool,
    /// Game detection: games are found and recorded on their own; otherwise the chosen screen.
    pub game_detection: bool,
    /// With several games open: "newest" (the last one opened) or "focused" (the one last used).
    pub game_pick: String,
    /// A manual recording when its game closes: "continue" (on the chosen screen) or "stop" (save).
    pub game_end: String,
    /// Program file names always / never treated as games (e.g. "MyGame.exe").
    pub games_always: Vec<String>,
    pub games_never: Vec<String>,
    /// Hotkeys: lock onto the game in front; move to the next open game.
    pub hotkey_lock_game: String,
    pub hotkey_next_game: String,
    /// Desktop Capture: Encore may record and keep Replay running outside games (your screen, an app).
    /// Off (games only, needs game detection): Replay waits for a game, a recording ends with its
    /// game, and recording with no game open is refused. Older settings called this
    /// `replay_on_desktop`.
    #[serde(alias = "replay_on_desktop")]
    pub desktop_capture: bool,
    /// Folder for work in progress (recordings being made, the instant replay buffer); empty =
    /// Encore's own folder in %LOCALAPPDATA%. A folder "Encore temp" is made inside the chosen one.
    pub work_dir: String,
    /// Bumped when a stored value changes meaning. 1: the old "high" quality became "ultra".
    /// Missing in older settings files, so it reads as 0 there.
    #[serde(default)]
    pub settings_version: u32,
    /// Length saved by the custom hotkey, in seconds (capped at the replay length when saving).
    pub replay_custom_seconds: u32,
    /// Sound for each moment (one of SOUND_NAMES; "none" = silent) and its volume, 0–100.
    /// Played by the interface when `sounds` is on.
    pub sound_start: String,
    pub sound_stop: String,
    pub sound_save: String,
    pub volume_start: u32,
    pub volume_stop: u32,
    pub volume_save: u32,
    /// Notifications: small pop-ups over the game, shown by the app shell in a window that screen
    /// capture leaves out. Do not disturb hides them all.
    pub notifications: bool,
    /// Where they appear (one of NOTIFY_SPOTS): corners and top/bottom centre use the card; the
    /// middle of the left or right edge uses the edge tab.
    pub notify_pos: String,
    /// How long each one stays on screen, in seconds.
    pub notify_seconds: u32,
    /// Which screen: "game" (the screen in use) or a monitor id ("id:…").
    pub notify_screen: String,
    /// Which events show one.
    /// Show a notification when a replay is saved (was called `notify_echo`).
    #[serde(alias = "notify_echo")]
    pub notify_replay: bool,
    pub notify_record: bool,
    pub notify_saved: bool,
    pub notify_game: bool,
    pub notify_problems: bool,
    /// Priority Capture: realtime graphics priority for the capture (ahead of the game and
    /// Windows). Off: high. For games that use all of the graphics card; can freeze a PC with an
    /// unstable graphics driver, so it's opt-in.
    pub gpu_realtime: bool,
    /// Priority Capture's warning has been read and "I understand" ticked (asked once).
    pub priority_capture_understood: bool,
    /// Priority Capture's switch is shown at the bottom of the sidebar.
    pub pcap_in_sidebar: bool,
    /// What clicking a clip in Saved this session does: "folder" (show it in Explorer),
    /// "player" (Windows' video player) or "editor" (open it in Clip Editor).
    pub clip_click: String,
    /// Where Clip Editor is installed (its .exe), found by Locate or chosen with Browse.
    pub clip_editor_path: String,
    /// Notification size, in percent of normal (one of NOTIFY_SIZES), on top of the screen's own
    /// scale.
    pub notify_size: u32,
}

pub const NOTIFY_SIZES: [u32; 5] = [100, 125, 150, 175, 200];

pub const CLIP_CLICKS: [&str; 3] = ["folder", "player", "editor"];
pub const NOTIFY_SPOTS: [&str; 8] = ["tl", "tc", "tr", "ml", "mr", "bl", "bc", "br"];
pub const NOTIFY_SECONDS: std::ops::RangeInclusive<u32> = 2..=10;

/// Sounds the interface can play (it synthesises them; no sound files).
pub const SOUND_NAMES: [&str; 9] = ["rise", "fall", "double", "pop", "bell", "blip", "click", "shutter", "none"];

pub const SETTINGS_VERSION: u32 = 1;

/// Shortest and longest custom save length.
pub const CUSTOM_SECONDS: std::ops::RangeInclusive<u32> = 5..=1200;

/// Replay lengths offered: 30 s to 20 minutes.
pub const REPLAY_SECONDS: [u32; 7] = [30, 60, 120, 300, 600, 900, 1200];

fn monitor_ok(m: &str) -> bool {
    m == "primary" || m == "follow" || (m.starts_with("id:") && m.len() > 3)
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            output_dir: paths::default_output_dir().to_string_lossy().into_owned(),
            codec: Codec::H264,
            fps: 60,
            resolution: Resolution::Native,
            quality: Quality::Ultra,
            capture_cursor: true,
            monitor: "primary".into(),
            audio_full_mix: true,
            audio_mic: true,
            mic_device: "default".into(),
            mic_name: String::new(),
            mic_denoise: "off".into(),
            bitrate_custom: false,
            bitrate_mbps: 30,
            reports: Vec::new(),
            audio_app_tracks: true,
            update_mode: "manual".into(),
            setup_done: false,
            close_to_tray: true,
            // Not Alt+F9/F10 (NVIDIA's recorder) or Alt+Z/Alt+R (its overlay).
            hotkey_record: "Ctrl+Alt+R".into(),
            sounds: true,
            replay_seconds: 120,
            replay_on: false,
            hotkey_save_replay: "Ctrl+Alt+S".into(),
            hotkey_save_30s: "Ctrl+Alt+1".into(),
            hotkey_save_60s: "Ctrl+Alt+2".into(),
            hotkey_save_custom: "Ctrl+Alt+3".into(),
            replay_custom_seconds: 90,
            do_not_disturb: false,
            game_detection: false,
            game_pick: "newest".into(),
            game_end: "continue".into(),
            games_always: Vec::new(),
            games_never: Vec::new(),
            hotkey_lock_game: "Ctrl+Alt+G".into(),
            hotkey_next_game: "Ctrl+Alt+N".into(),
            desktop_capture: true,
            work_dir: String::new(),
            settings_version: SETTINGS_VERSION,
            sound_start: "rise".into(),
            sound_stop: "fall".into(),
            sound_save: "double".into(),
            volume_start: 50,
            volume_stop: 50,
            volume_save: 50,
            notifications: true,
            notify_pos: "tr".into(),
            notify_seconds: 5,
            notify_screen: "game".into(),
            notify_replay: true,
            notify_record: true,
            notify_saved: true,
            notify_game: true,
            notify_problems: true,
            gpu_realtime: false,
            priority_capture_understood: false,
            pcap_in_sidebar: true,
            clip_click: "folder".into(),
            clip_editor_path: String::new(),
            notify_size: 100,
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
        let mut updated: Settings =
            serde_json::from_value(current).map_err(|e| format!("Invalid setting value: {e}"))?;
        if !updated.game_detection {
            updated.desktop_capture = true; // without game detection, only the desktop is left
        }
        if !ALLOWED_FPS.contains(&updated.fps) {
            return Err(format!("Frame rate must be one of {:?}", ALLOWED_FPS));
        }
        if updated.output_dir.trim().is_empty() {
            return Err("Output folder cannot be empty".into());
        }
        if !(BITRATE_MBPS_MIN..=BITRATE_MBPS_MAX).contains(&updated.bitrate_mbps) {
            return Err(format!("The bitrate must be {BITRATE_MBPS_MIN} to {BITRATE_MBPS_MAX} Mbps"));
        }
        if !crate::denoise::LEVELS.contains(&updated.mic_denoise.as_str()) {
            return Err("Noise reduction must be off, light or strong".into());
        }
        if !["manual", "auto_download", "never"].contains(&updated.update_mode.as_str()) {
            return Err("Update mode must be manual, auto_download or never".into());
        }
        if !REPLAY_SECONDS.contains(&updated.replay_seconds) {
            return Err(format!("Replay length must be one of {:?} seconds", REPLAY_SECONDS));
        }
        if !["newest", "focused"].contains(&updated.game_pick.as_str()) {
            return Err("Game choice must be newest or focused".into());
        }
        if !["continue", "stop"].contains(&updated.game_end.as_str()) {
            return Err("When a game closes must be continue or stop".into());
        }
        if updated.games_always.len() > 200 || updated.games_never.len() > 200 {
            return Err("That list is too long".into());
        }
        for k in [&updated.hotkey_save_replay, &updated.hotkey_save_30s, &updated.hotkey_save_60s, &updated.hotkey_save_custom, &updated.hotkey_lock_game, &updated.hotkey_next_game] {
            if k.len() > 64 {
                return Err("Hotkey is too long".into());
            }
        }
        if !CUSTOM_SECONDS.contains(&updated.replay_custom_seconds) {
            return Err("Custom length must be between 5 seconds and 20 minutes".into());
        }
        for snd in [&updated.sound_start, &updated.sound_stop, &updated.sound_save] {
            if !SOUND_NAMES.contains(&snd.as_str()) {
                return Err(format!("Unknown sound: {snd}"));
            }
        }
        if [updated.volume_start, updated.volume_stop, updated.volume_save].iter().any(|v| *v > 100) {
            return Err("Volume must be between 0 and 100".into());
        }
        if !NOTIFY_SPOTS.contains(&updated.notify_pos.as_str()) {
            return Err("Unknown notification position".into());
        }
        if !CLIP_CLICKS.contains(&updated.clip_click.as_str()) {
            return Err("Unknown choice for clicking a clip".into());
        }
        if !NOTIFY_SIZES.contains(&updated.notify_size) {
            return Err(format!("Notification size must be one of {:?} %", NOTIFY_SIZES));
        }
        if !NOTIFY_SECONDS.contains(&updated.notify_seconds) {
            return Err("Notifications can stay for 2 to 10 seconds".into());
        }
        let ns = updated.notify_screen.as_str();
        if !(ns == "game" || (ns.starts_with("id:") && ns.len() > 3)) {
            return Err("Notification screen must be game or a monitor id".into());
        }
        if updated.hotkey_record.len() > 64 {
            return Err("Hotkey is too long".into());
        }
        if updated.mic_device.trim().is_empty() {
            return Err("Microphone cannot be empty".into());
        }
        if !monitor_ok(&updated.monitor) {
            return Err("Monitor must be primary, follow or a monitor id".into());
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
        if !CUSTOM_SECONDS.contains(&self.replay_custom_seconds) {
            self.replay_custom_seconds = 90;
        }
        // Before 1: there were three qualities and "high" was the top one. It's now called Ultra
        // (same bitrate), with a new High below it.
        if self.settings_version < 1 {
            if self.quality == Quality::High {
                self.quality = Quality::Ultra;
            }
            self.settings_version = 1;
        }
        if !["newest", "focused"].contains(&self.game_pick.as_str()) {
            self.game_pick = "newest".into();
        }
        if !self.game_detection {
            self.desktop_capture = true; // otherwise nothing could ever be recorded
        }
        if !["continue", "stop"].contains(&self.game_end.as_str()) {
            self.game_end = "continue".into();
        }
        let d = Settings::default();
        for (snd, def) in [(&mut self.sound_start, d.sound_start), (&mut self.sound_stop, d.sound_stop), (&mut self.sound_save, d.sound_save)] {
            if !SOUND_NAMES.contains(&snd.as_str()) {
                *snd = def;
            }
        }
        for v in [&mut self.volume_start, &mut self.volume_stop, &mut self.volume_save] {
            *v = (*v).min(100);
        }
        if !NOTIFY_SPOTS.contains(&self.notify_pos.as_str()) {
            self.notify_pos = d.notify_pos.clone();
        }
        if !CLIP_CLICKS.contains(&self.clip_click.as_str()) {
            self.clip_click = d.clip_click.clone();
        }
        if !NOTIFY_SIZES.contains(&self.notify_size) {
            self.notify_size = 100;
        }
        self.notify_seconds = self.notify_seconds.clamp(*NOTIFY_SECONDS.start(), *NOTIFY_SECONDS.end());
        if !(self.notify_screen == "game" || (self.notify_screen.starts_with("id:") && self.notify_screen.len() > 3)) {
            self.notify_screen = "game".into();
        }
        // Not set up yet and still on an old guessed default: use the real Videos folder (it may be
        // on another drive or in OneDrive, where the guess made a new, empty folder).
        if !self.setup_done && paths::legacy_default_dirs().iter().any(|d| d.to_string_lossy() == self.output_dir) {
            self.output_dir = paths::default_output_dir().to_string_lossy().into_owned();
        }
        // Before 0.5 there was also "mouse" (chosen at start only).
        if !monitor_ok(&self.monitor) {
            self.monitor = "primary".into();
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_old_echo_notification_setting_carries_over() {
        let s: Settings = serde_json::from_str(r#"{"notify_echo": false}"#).unwrap();
        assert!(!s.notify_replay);
    }

    #[test]
    fn old_settings_with_the_removed_audio_offset_still_load() {
        let s: Settings = serde_json::from_str(r#"{"audio_offset_ms": 0, "fps": 60}"#).unwrap();
        assert_eq!(s.fps, 60);
    }

    #[test]
    fn desktop_capture_reads_the_old_name_and_needs_game_detection_to_be_off() {
        let old: Settings = serde_json::from_value(json!({"game_detection": true, "replay_on_desktop": false})).unwrap();
        assert!(!old.desktop_capture);
        let s = Settings::default();
        assert!(s.desktop_capture);
        let u = s.apply_patch(&json!({"desktop_capture": false})).unwrap();
        assert!(u.desktop_capture, "game detection is off, so desktop capture stays on");
        let u = s.apply_patch(&json!({"game_detection": true, "desktop_capture": false})).unwrap();
        assert!(!u.desktop_capture);
        let u = u.apply_patch(&json!({"game_detection": false})).unwrap();
        assert!(u.desktop_capture, "turning game detection off turns desktop capture back on");
    }

    #[test]
    fn old_high_becomes_ultra_once() {
        let old: Settings = serde_json::from_value(json!({"quality": "high", "fps": 60})).unwrap();
        assert_eq!(old.settings_version, 0);
        let n = old.normalised();
        assert_eq!((n.quality, n.settings_version), (Quality::Ultra, 1));
        // Chosen after the change: stays High.
        let new: Settings = serde_json::from_value(json!({"quality": "high", "settings_version": 1})).unwrap();
        assert_eq!(new.normalised().quality, Quality::High);
        // Medium and Low are unchanged.
        let med: Settings = serde_json::from_value(json!({"quality": "medium"})).unwrap();
        assert_eq!(med.normalised().quality, Quality::Medium);
        assert_eq!(Settings::default().quality, Quality::Ultra);
        assert!(Settings::default().apply_patch(&json!({"quality": "ultra"})).is_ok());
    }

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
        assert!(s.apply_patch(&json!({"monitor": "primary"})).is_ok());
        assert!(s.apply_patch(&json!({"output_dir": "  "})).is_err());
        assert!(s.apply_patch(&json!([1, 2])).is_err());
        assert!(s.apply_patch(&json!({"audio_mic": "yes"})).is_err());
        assert!(s.apply_patch(&json!({"bitrate_mbps": 2})).is_err());
        assert!(s.apply_patch(&json!({"bitrate_mbps": 151})).is_err());
        assert_eq!(s.apply_patch(&json!({"bitrate_custom": true, "bitrate_mbps": 45})).unwrap().bitrate_mbps, 45);
        assert!(s.apply_patch(&json!({"audio_mic": false})).is_ok());
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
        for old in ["mouse", ""] {
            let o: Settings = serde_json::from_value(serde_json::json!({ "monitor": old })).unwrap();
            assert_eq!(o.normalised().monitor, "primary");
        }
        // A choice already saved stays as it was.
        for kept in ["follow", "primary"] {
            let o: Settings = serde_json::from_value(serde_json::json!({ "monitor": kept })).unwrap();
            assert_eq!(o.normalised().monitor, kept);
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
        assert_eq!((s.hotkey_save_30s.as_str(), s.hotkey_save_60s.as_str(), s.hotkey_save_custom.as_str()), ("Ctrl+Alt+1", "Ctrl+Alt+2", "Ctrl+Alt+3"));
        assert_eq!(s.replay_custom_seconds, 90);
        assert_eq!((s.sound_start.as_str(), s.sound_stop.as_str(), s.sound_save.as_str()), ("rise", "fall", "double"));
        assert_eq!((s.volume_start, s.volume_stop, s.volume_save), (50, 50, 50));
        assert!(s.notifications && s.notify_replay && s.notify_record && s.notify_saved && s.notify_game && s.notify_problems);
        assert_eq!((s.notify_pos.as_str(), s.notify_seconds, s.notify_screen.as_str()), ("tr", 5, "game"));
        assert!(!s.gpu_realtime); // opt-in only
    }

    #[test]
    fn notification_settings_are_checked() {
        let s = Settings::default();
        assert!(s.apply_patch(&serde_json::json!({ "notify_pos": "mr", "notify_seconds": 10 })).is_ok());
        assert!(s.apply_patch(&serde_json::json!({ "notify_pos": "middle" })).is_err());
        assert!(s.apply_patch(&serde_json::json!({ "notify_seconds": 1 })).is_err());
        assert!(s.apply_patch(&serde_json::json!({ "notify_size": 175 })).is_ok());
        assert!(s.apply_patch(&serde_json::json!({ "notify_size": 300 })).is_err());
        assert_eq!(Settings::default().notify_size, 100);
        assert!(s.apply_patch(&serde_json::json!({ "notify_screen": "id:abc" })).is_ok());
        assert!(s.apply_patch(&serde_json::json!({ "notify_screen": "left" })).is_err());
        // Settings from before notifications existed, or with odd values, load with sensible ones.
        let o: Settings = serde_json::from_value(serde_json::json!({ "notify_pos": "x", "notify_seconds": 99 })).unwrap();
        let o = o.normalised();
        assert_eq!((o.notify_pos.as_str(), o.notify_seconds, o.notify_screen.as_str()), ("tr", 10, "game"));
    }
}
