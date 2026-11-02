//! Engine state and command handling. One command at a time, on the main thread.

use crate::clips;
use crate::audio::{self, AudioOptions};
use crate::ffmpeg::{self, CaptureSpec, Encoder, Pipeline, Target, ALL_CODECS, VENDOR_ORDER};
use crate::games::{self, Pick, Watch};
use crate::log::{log, tail};
use crate::paths;
use crate::platform;
use crate::protocol::{reply_err, reply_ok, send, send_event, Command};
use crate::recorder::{self, Active, Saved};
use crate::settings::{Codec, Settings};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Default, Serialize)]
pub struct Capabilities {
    pub probed: bool,
    pub ffmpeg_path: String,
    pub ffmpeg_found: bool,
    pub ffmpeg_version: String,
    /// Window/monitor capture (FFmpeg's gfxcapture source) is available.
    pub capture_supported: bool,
    pub encoders: Vec<EncoderEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EncoderEntry {
    pub codec: Codec,
    pub vendor: ffmpeg::Vendor,
    pub name: &'static str,
}

impl Capabilities {
    pub fn best_encoder(&self, codec: Codec) -> Option<Encoder> {
        VENDOR_ORDER
            .iter()
            .find(|v| self.encoders.iter().any(|e| e.codec == codec && e.vendor == **v))
            .map(|v| Encoder { codec, vendor: *v })
    }
}

/// Checks FFmpeg and which GPU encoders work on this PC. Slow (a few seconds), so run off-thread.
pub fn probe(ffmpeg_path: &std::path::Path) -> Capabilities {
    let version = ffmpeg::version(ffmpeg_path);
    let mut caps = Capabilities {
        probed: true,
        ffmpeg_path: ffmpeg_path.display().to_string(),
        ffmpeg_found: version.is_some(),
        ffmpeg_version: version.unwrap_or_default(),
        ..Default::default()
    };
    if !caps.ffmpeg_found {
        return caps;
    }
    caps.capture_supported = ffmpeg::has_gfxcapture(ffmpeg_path);
    for codec in ALL_CODECS {
        for vendor in VENDOR_ORDER {
            let enc = Encoder { codec, vendor };
            if ffmpeg::probe_encoder(ffmpeg_path, enc) {
                caps.encoders.push(EncoderEntry { codec, vendor, name: enc.name() });
            }
        }
    }
    caps
}

pub struct Engine {
    ffmpeg: PathBuf,
    settings: Settings,
    caps: Arc<Mutex<Capabilities>>,
    active: Option<Active>,
    /// Pipeline that worked for each encoder this session (skip a failing zero-copy attempt next time).
    known_pipeline: HashMap<&'static str, Pipeline>,
    /// The monitor of the window last clicked on (Encore's own window doesn't count).
    focus: Option<u64>,
    /// A monitor the focus moved to, and since when (a switch waits for it to settle).
    focus_candidate: Option<(u64, Instant)>,
    /// Whether the current recording follows the focus (desktop, "follow" setting).
    following: bool,
    /// A recording made from a running instant-replay session (it shares the capture).
    clip: Option<Clip>,
    /// Start times of saves still reading the replay session's files (they mustn't be deleted).
    pins: Arc<Mutex<Vec<u64>>>,
    /// Game detection: the games that are open, and which one is locked.
    watch: Watch,
    last_game_scan: Option<Instant>,
    /// Priority Capture is on and no game is detected: since when (it turns itself off after
    /// PRIORITY_NO_GAME).
    no_game_since: Option<Instant>,
    /// Names already worked out for each game process (reading them takes a moment).
    game_names: HashMap<u32, String>,
    /// What the interface last heard about games (to send updates only when something changes).
    games_sent: String,
    /// What the current capture was started for: "desktop" (Follow / a monitor) or "window"
    /// (Select Application, which pauses game detection).
    session_source: String,
    /// Instant replay is on but only keeps games, and no game is open: waiting for one.
    replay_waiting: bool,
    /// Captures that failed to start, by what they were for: (failures, when the last one was).
    /// A failed target waits before it's tried again (10 s, then longer), so a window that
    /// can't be captured never sets off a stream of FFmpeg starts.
    switch_fails: HashMap<String, (u32, Instant)>,
    /// Windows that only look like games because they fill the screen, and when they were first
    /// seen: they count once they've stayed a few seconds (screenshot tools and pop-ups don't).
    fullscreen_seen: HashMap<u64, Instant>,
}

/// A recording taken from the instant-replay capture: from `from` until it is stopped.
struct Clip {
    from: u64,
    out_path: PathBuf,
    label: String,
    local: platform::LocalTime,
}

/// How long a full screen window that isn't otherwise known as a game must stay before it counts.
const FULLSCREEN_SETTLE: Duration = Duration::from_secs(4);

/// How long the focus must stay on another monitor before the recording moves there.
const SWITCH_SETTLE: Duration = Duration::from_millis(300);

/// Makes sure recordings could be saved in `dir` without creating anything: tests a write in the
/// nearest folder that already exists (the folder itself, or the one it would be made in), then
/// removes the test file. The Encore folder is made later (setup finished, or first recording).
fn check_writable(dir: &std::path::Path) -> Result<(), String> {
    let existing = dir.ancestors().find(|p| p.is_dir()).map(|p| p.to_path_buf());
    let result = match &existing {
        None => Err(std::io::Error::new(std::io::ErrorKind::NotFound, "the drive or folder doesn't exist")),
        Some(p) => {
            let test = p.join(format!(".encore-write-test-{}", std::process::id()));
            std::fs::write(&test, b"ok").and_then(|_| std::fs::remove_file(&test))
        }
    };
    result.map_err(|e| {
        format!(
            "Encore can't save recordings in {} ({e}). Pick another folder: some, like C:\\Program Files, need administrator rights.",
            dir.display()
        )
    })
}

/// The stable id of the monitor with this handle (what the interface knows monitors by).
fn monitor_id(handle: u64) -> Option<String> {
    platform::list_monitors().into_iter().find(|m| m.handle == handle).map(|m| m.id)
}

/// Priority Capture turns itself off after this long with no game detected.
const PRIORITY_NO_GAME: Duration = Duration::from_secs(60);

impl Engine {
    pub fn new(ffmpeg: PathBuf) -> Engine {
        let caps = Arc::new(Mutex::new(Capabilities::default()));
        let probe_caps = caps.clone();
        let probe_path = ffmpeg.clone();
        std::thread::spawn(move || {
            let result = probe(&probe_path);
            log(&format!(
                "probe: ffmpeg={} capture={} encoders={:?}",
                result.ffmpeg_version,
                result.capture_supported,
                result.encoders.iter().map(|e| e.name).collect::<Vec<_>>()
            ));
            let value = serde_json::to_value(&result).unwrap_or(Value::Null);
            *probe_caps.lock().unwrap() = result;
            send_event("capabilities", value);
        });
        let mut settings = Settings::load();
        // Priority Capture never starts on: it's only on for the session it's turned on in (a
        // fresh graphics driver met a realtime capture straight after a restart and the screens
        // kept blacking out).
        if settings.gpu_realtime {
            settings.gpu_realtime = false;
            let _ = settings.save();
            log("Priority Capture was on last time: off at start");
        }
        platform::set_gpu_top(false);
        Engine {
            ffmpeg,
            settings,
            caps,
            active: None,
            known_pipeline: HashMap::new(),
            focus: None,
            focus_candidate: None,
            following: false,
            clip: None,
            pins: Arc::new(Mutex::new(Vec::new())),
            watch: Watch::default(),
            last_game_scan: None,
            no_game_since: None,
            game_names: HashMap::new(),
            games_sent: String::new(),
            session_source: "desktop".into(),
            replay_waiting: false,
            switch_fails: HashMap::new(),
            fullscreen_seen: HashMap::new(),
        }
    }

    fn caps(&self) -> Capabilities {
        self.caps.lock().unwrap().clone()
    }

    fn wait_for_probe(&self, max: Duration) -> Capabilities {
        let begun = Instant::now();
        loop {
            let c = self.caps();
            if c.probed || begun.elapsed() > max {
                return c;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    /// Handles one command. Returns true when the engine should exit.
    pub fn handle(&mut self, c: Command) -> bool {
        let id = c.id;
        let result: Result<Value, String> = match c.cmd.as_str() {
            "get_status" => Ok(self.status()),
            "get_settings" => Ok(serde_json::to_value(&self.settings).unwrap_or(Value::Null)),
            "set_settings" => self.set_settings(&c.args),
            "get_capabilities" => Ok(serde_json::to_value(self.caps()).unwrap_or(Value::Null)),
            "list_monitors" => Ok(serde_json::to_value(platform::list_monitors()).unwrap_or(Value::Null)),
            "list_windows" => Ok(serde_json::to_value(platform::list_windows()).unwrap_or(Value::Null)),
            "list_audio_inputs" => Ok(serde_json::to_value(audio::list_inputs()).unwrap_or(Value::Null)),
            "start_recording" => self.start_recording(&c.args),
            "stop_recording" => {
                // Reply first so the interface can show "Saving..." while the file is assembled.
                if !self.is_recording() {
                    Err("Not recording.".into())
                } else {
                    send(&reply_ok(id, json!({ "saving": true })));
                    self.stop_recording();
                    return false;
                }
            }
            "start_replay" => self.start_replay(&c.args),
            "stop_replay" => {
                self.stop_replay();
                Ok(self.status())
            }
            "save_replay" => self.save_replay(&c.args),
            // A saved clip: rename it ({"path", "name"}) or send it to the Recycle Bin ({"path"}).
            "rename_clip" => {
                let path = PathBuf::from(c.args.get("path").and_then(|v| v.as_str()).unwrap_or(""));
                let name = c.args.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let root = paths::encore_root(&PathBuf::from(&self.settings.output_dir));
                clips::rename(&root, &path, name).map(|p| {
                    log(&format!("renamed clip {} to {}", path.display(), p.display()));
                    json!({ "path": p.display().to_string() })
                })
            }
            "delete_clip" => {
                let path = PathBuf::from(c.args.get("path").and_then(|v| v.as_str()).unwrap_or(""));
                let root = paths::encore_root(&PathBuf::from(&self.settings.output_dir));
                clips::delete(&root, &path).map(|_| {
                    log(&format!("deleted clip {} (to the Recycle Bin)", path.display()));
                    json!({ "deleted": true })
                })
            }
            // Lock onto a game ({"pid": n}) or unlock ({"pid": null}).
            "lock_game" => {
                let pid = c.args.get("pid").and_then(|v| v.as_u64()).map(|p| p as u32);
                if self.watch.lock(pid) {
                    self.send_games(true);
                    Ok(self.games_json())
                } else {
                    Err("That game has closed.".into())
                }
            }
            // Hotkey: lock onto the game in front.
            "lock_focused_game" => {
                self.scan_games(true);
                match self.watch.lock_window(platform::foreground_window()) {
                    Some(name) => {
                        log(&format!("locked to {name}"));
                        self.send_games(true);
                        Ok(json!({ "locked": name }))
                    }
                    None => Err("The window in front isn't a game Encore knows. Add it in Settings → Games.".into()),
                }
            }
            // Hotkey: move to the next open game.
            "next_game" => {
                self.scan_games(true);
                match self.watch.next(self.pick()) {
                    Some(name) => {
                        log(&format!("next game: {name}"));
                        self.send_games(true);
                        Ok(json!({ "locked": name }))
                    }
                    None => Err("No games are open.".into()),
                }
            }
            // A line from the app shell for the log (e.g. hotkey presses).
            "note" => {
                log(c.args.get("message").and_then(|m| m.as_str()).unwrap_or(""));
                Ok(json!({}))
            }
            "shutdown" => {
                send(&reply_ok(id, json!({})));
                self.shutdown();
                return true;
            }
            other => Err(format!("Unknown command: {other}")),
        };
        match result {
            Ok(v) => send(&reply_ok(id, v)),
            Err(e) => send(&reply_err(id, &e)),
        }
        false
    }

    fn is_recording(&self) -> bool {
        self.clip.is_some() || self.active.as_ref().map(|a| !a.is_replay()).unwrap_or(false)
    }

    fn status(&mut self) -> Value {
        let focus = self.focus.and_then(monitor_id);
        let replay = match &self.active {
            Some(a) if a.is_replay() => json!({ "on": true, "seconds": self.settings.replay_seconds, "label": a.label }),
            _ if self.replay_waiting => json!({ "on": true, "waiting": true, "seconds": self.settings.replay_seconds }),
            _ => json!({ "on": false }),
        };
        let recording = match &self.active {
            Some(a) if self.is_recording() => json!({
                "label": a.label,
                "encoder": a.encoder.name(),
                "pipeline": a.pipeline,
                "width": a.width,
                "height": a.height,
                "fps": a.fps(),
                "started_unix_ms": a.started_unix_ms,
                "elapsed_ms": a.elapsed_ms(),
                "out_path": self.clip.as_ref().map(|c| &c.out_path).unwrap_or(&a.out_path).display().to_string(),
                "audio_tracks": a.track_titles(),
                "monitor": match a.target() { Target::Monitor { handle } => monitor_id(handle), _ => None },
                "following": self.following,
                "game": match a.target() {
                    Target::Window { handle } if self.session_source == "desktop" => self.watch.games.iter().find(|g| g.handle == handle).map(|g| g.name.clone()),
                    _ => None,
                },
            }),
            _ => Value::Null,
        };
        json!({
            "state": if self.is_recording() { "recording" } else { "idle" },
            "recording": recording,
            "replay": replay,
            "focus_monitor": focus,
            "elevated": platform::is_elevated(),
            "games": self.games_json(),
        })
    }

    fn set_settings(&mut self, patch: &Value) -> Result<Value, String> {
        let updated = self.settings.apply_patch(patch)?;
        if updated.output_dir != self.settings.output_dir {
            check_writable(&paths::encore_root(&PathBuf::from(&updated.output_dir)))?;
        }
        if updated.work_dir != self.settings.work_dir {
            if self.is_recording() {
                return Err("Stop the recording first, then change where Encore works.".into());
            }
            if !updated.work_dir.trim().is_empty() {
                check_writable(&paths::work_root(&updated.work_dir))?;
            }
        }
        // Setup finished: now make the Encore folder where they chose.
        if updated.setup_done && !self.settings.setup_done {
            let root = paths::encore_root(&PathBuf::from(&updated.output_dir));
            if let Err(e) = std::fs::create_dir_all(&root) {
                log(&format!("could not create {}: {e}", root.display()));
            }
        }
        updated.save()?;
        platform::set_gpu_top(updated.gpu_realtime);
        let changed = updated.gpu_realtime != self.settings.gpu_realtime;
        if changed {
            log(&format!("Priority Capture {}", if updated.gpu_realtime { "on" } else { "off" }));
        }
        self.settings = updated;
        if changed {
            self.no_game_since = None;
            if let Some(a) = &self.active {
                a.reprioritise();
            }
        }
        Ok(serde_json::to_value(&self.settings).unwrap_or(Value::Null))
    }

    fn start_recording(&mut self, args: &Value) -> Result<Value, String> {
        if self.is_recording() {
            return Err("Already recording.".into());
        }
        if let Some(a) = &self.active {
            // Instant replay is running: the recording comes from the same capture, from now on.
            let now = platform::now_hns();
            let out_path = recorder::output_path(&PathBuf::from(&self.settings.output_dir), &a.label, platform::local_time());
            let info = json!({
                "label": a.label,
                "source": a.label,
                "encoder": a.encoder.name(),
                "pipeline": a.pipeline,
                "width": a.width,
                "height": a.height,
                "fps": a.fps(),
                "started_unix_ms": recorder::unix_ms(),
                "audio_tracks": a.track_titles(),
                "monitor": match a.target() { Target::Monitor { handle } => monitor_id(handle), _ => None },
                "following": self.following,
                "from_replay": true,
            });
            self.clip = Some(Clip { from: now, out_path, label: a.label.clone(), local: platform::local_time() });
            send_event("recording_started", info.clone());
            return Ok(info);
        }
        let source = args.get("source").and_then(|v| v.as_str()).unwrap_or("desktop");
        if source == "desktop" && self.settings.game_detection && !self.settings.desktop_capture {
            self.scan_games(true);
            if self.current_game().is_none() {
                return Err("Start a game to record, or turn on Desktop Capture to record your screen.".into());
            }
        }
        self.start_session(args, false)
    }

    /// Starts a capture: a normal recording, or the instant-replay buffer (`replay`).
    fn start_session(&mut self, args: &Value, replay: bool) -> Result<Value, String> {
        let caps = self.wait_for_probe(Duration::from_secs(20));
        if !caps.ffmpeg_found {
            return Err(format!("FFmpeg was not found at {}. Reinstalling Encore should fix this.", caps.ffmpeg_path));
        }
        if !caps.capture_supported {
            return Err("This copy of FFmpeg cannot capture the screen (its window/monitor capture is missing).".into());
        }
        let codec = self.settings.codec;
        let encoder = caps.best_encoder(codec).ok_or_else(|| {
            let available: Vec<&str> = caps.encoders.iter().map(|e| e.name).collect();
            if available.is_empty() {
                "No graphics card video encoder was found. Encore needs an NVIDIA, AMD or Intel GPU encoder; updating your graphics driver may help.".to_string()
            } else {
                format!("Your graphics card cannot encode {codec:?}. Pick another format in Settings (available: {}).", available.join(", "))
            }
        })?;

        let source = args.get("source").and_then(|v| v.as_str()).unwrap_or("desktop");
        let following = source == "desktop" && self.settings.monitor == "follow";
        if source == "desktop" && self.settings.game_detection {
            self.scan_games(true);
        }
        let game = if source == "desktop" && self.settings.game_detection { self.current_game() } else { None };
        let (target, label, src_w, src_h, describe) = match source {
            "desktop" if game.is_some() => {
                let g = game.unwrap();
                let (w, h) = platform::window_size(g.handle).filter(|(w, h)| *w > 0 && *h > 0).map_or_else(
                    || self.pick_monitor().map(|m| (m.2, m.3)).unwrap_or((1920, 1080)),
                    |s| s,
                );
                // The output size is set once, from the screen, so switching to and from the
                // game doesn't change it (the game is scaled to fit if it's windowed).
                let (mw, mh) = self.pick_monitor().map(|m| (m.2, m.3)).unwrap_or((w, h));
                (Target::Window { handle: g.handle }, g.name.clone(), mw.max(w), mh.max(h), format!("Game: {}", g.name))
            }
            "desktop" => self.pick_monitor()?,
            "window" => {
                let handle = args
                    .get("handle")
                    .and_then(|v| v.as_u64())
                    .ok_or("Choose a window to record first.")?;
                if !platform::window_exists(handle) {
                    return Err("That window has closed. Refresh the list and pick it again.".into());
                }
                if platform::window_minimised(handle) {
                    return Err("That window is minimised. Restore it, then start recording.".into());
                }
                let (w, h) = platform::window_size(handle).ok_or("Could not read that window's size.")?;
                let title = platform::list_windows()
                    .into_iter()
                    .find(|w| w.handle == handle)
                    .map(|w| w.title)
                    .unwrap_or_else(|| "Window".into());
                (Target::Window { handle }, title.clone(), w, h, format!("Window: {title}"))
            }
            other => return Err(format!("Unknown source: {other}")),
        };

        let (width, height) = ffmpeg::output_size(src_w, src_h, self.settings.resolution);
        let spec = CaptureSpec { target, fps: self.settings.fps, cursor: self.settings.capture_cursor, width, height, quality: self.settings.quality };
        let kbps = ffmpeg::bitrate_kbps(width, height, spec.fps, self.settings.quality, codec);
        let out_dir = PathBuf::from(&self.settings.output_dir);
        let out_path = recorder::output_path(&out_dir, &label, platform::local_time());
        let preferred = self.known_pipeline.get(encoder.name()).copied();
        self.find_mic();
        let audio_opts = AudioOptions {
            full_mix: self.settings.audio_full_mix,
            mic: self.settings.audio_mic,
            mic_device: self.settings.mic_device.clone(),
            app_tracks: self.settings.audio_app_tracks,
            max_app_tracks: 8,
            roll_keep_from: None,
        };

        let active = recorder::start(
            &self.ffmpeg,
            &spec,
            encoder,
            kbps,
            &label,
            out_path,
            &self.work_root(),
            preferred,
            &audio_opts,
            replay,
        )?;
        self.known_pipeline.insert(encoder.name(), active.pipeline);
        let info = json!({
            "label": active.label,
            "source": describe,
            "encoder": encoder.name(),
            "pipeline": active.pipeline,
            "width": width,
            "height": height,
            "fps": spec.fps,
            "bitrate_kbps": kbps,
            "started_unix_ms": active.started_unix_ms,
            "audio_tracks": active.track_titles(),
            "monitor": match active.target() { Target::Monitor { handle } => monitor_id(handle), _ => None },
            "following": following,
        });
        if active.pipeline == Pipeline::Download {
            send_event("warning", json!({ "message": "Recording is using the slower copy path on this PC (zero-copy was not accepted by the driver). It works, but uses more CPU." }));
        }
        self.active = Some(active);
        self.following = following;
        self.focus_candidate = None;
        self.session_source = source.to_string();
        if replay {
            send_event("replay_status", json!({ "on": true, "seconds": self.settings.replay_seconds, "label": info["label"] }));
        } else {
            send_event("recording_started", info.clone());
        }
        Ok(info)
    }

    fn start_replay(&mut self, args: &Value) -> Result<Value, String> {
        match &self.active {
            Some(a) if a.is_replay() => return Ok(self.status()),
            Some(_) => return Err("Stop the recording first, then turn on Echoes.".into()),
            None => {}
        }
        let source = args.get("source").and_then(|v| v.as_str()).unwrap_or("desktop");
        if source == "desktop" && self.settings.game_detection && !self.settings.desktop_capture {
            self.scan_games(true);
            if self.current_game().is_none() {
                // Only games are kept: wait for one to open.
                self.replay_waiting = true;
                send_event("replay_status", json!({ "on": true, "waiting": true, "seconds": self.settings.replay_seconds }));
                return Ok(self.status());
            }
        }
        self.replay_waiting = false;
        self.start_session(args, true)?;
        Ok(self.status())
    }

    /// Waits (up to a minute) for saves still reading the replay session's files.
    fn wait_for_saves(&self) {
        let begun = Instant::now();
        while !self.pins.lock().unwrap().is_empty() && begun.elapsed() < Duration::from_secs(60) {
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    fn stop_replay(&mut self) {
        if self.replay_waiting {
            self.replay_waiting = false;
            send_event("replay_status", json!({ "on": false }));
        }
        if self.clip.is_some() {
            self.stop_recording();
        }
        if self.active.as_ref().map(|a| a.is_replay()).unwrap_or(false) {
            self.wait_for_saves();
            if let Some(a) = self.active.take() {
                a.discard();
            }
            send_event("replay_status", json!({ "on": false }));
        }
    }

    /// Saves the last `seconds` (from args; the whole replay length if not given, and never more
    /// than it) without stopping the buffer.
    fn save_replay(&mut self, args: &Value) -> Result<Value, String> {
        let Some(a) = self.active.as_ref().filter(|a| a.is_replay()) else {
            return Err("Echoes is off. Turn it on first (top left).".into());
        };
        let max = self.settings.replay_seconds;
        let seconds = args.get("seconds").and_then(|v| v.as_u64()).map(|s| (s as u32).clamp(1, max)).unwrap_or(max);
        let to = platform::now_hns();
        let from = to.saturating_sub(seconds as u64 * 10_000_000);
        let label = a.label_for(from, to);
        let out_path = recorder::output_path(&PathBuf::from(&self.settings.output_dir), &label, platform::local_time());
        self.save_in_background(from, to, out_path, label, true);
        Ok(json!({ "saving": true, "seconds": seconds }))
    }

    /// Joins `from..to` of the replay session into `out_path` on a worker thread, so the capture
    /// and the engine carry on; reports `replay_saved` (or `recording_saved`) when done.
    fn save_in_background(&self, from: u64, to: u64, out_path: PathBuf, label: String, replay: bool) {
        let Some(a) = self.active.as_ref() else { return };
        // Claim the file name now, so a second save in the same second gets "(2)".
        if let Some(dir) = out_path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::File::create(&out_path);
        self.pins.lock().unwrap().push(from);
        let (ffmpeg, dir, snapshot, pins) = (self.ffmpeg.clone(), a.seg_dir.clone(), a.snapshot(), self.pins.clone());
        if replay {
            send_event("replay_saving", json!({ "seconds": (to.saturating_sub(from) + 5_000_000) / 10_000_000 }));
        }
        std::thread::spawn(move || {
            let result = recorder::save_window(&ffmpeg, &dir, &snapshot, from, to, &out_path);
            {
                let mut p = pins.lock().unwrap();
                if let Some(i) = p.iter().position(|&x| x == from) {
                    p.remove(i);
                }
            }
            if result.is_err() {
                let _ = std::fs::remove_file(&out_path); // the placeholder
            }
            if replay {
                report_replay_saved(result, &label);
            } else {
                report_saved(result, &label, false);
            }
        });
    }

    /// Desktop mode: which monitor, per the "monitor" setting. Falls back to the primary monitor.
    fn pick_monitor(&self) -> Result<(Target, String, u32, u32, String), String> {
        let monitors = platform::list_monitors();
        if monitors.is_empty() {
            return Err("No monitors were found.".into());
        }
        let primary = monitors.iter().find(|m| m.primary).unwrap_or(&monitors[0]);
        let wanted = self.settings.monitor.as_str();
        let chosen = if wanted == "follow" {
            // The monitor last clicked on before Encore (pressing record focuses Encore itself).
            self.focus
                .or_else(platform::focused_monitor)
                .or_else(platform::monitor_under_cursor)
                .and_then(|h| monitors.iter().find(|m| m.handle == h))
                .unwrap_or(primary)
        } else if let Some(id) = wanted.strip_prefix("id:") {
            match monitors.iter().find(|m| m.id == id) {
                Some(m) => m,
                None => {
                    send_event("warning", json!({ "message": "The chosen monitor is not connected, so the primary monitor is being recorded." }));
                    primary
                }
            }
        } else {
            primary
        };
        Ok((
            Target::Monitor { handle: chosen.handle },
            "Desktop".into(),
            chosen.width,
            chosen.height,
            format!("Desktop: {}", chosen.name),
        ))
    }

    fn stop_recording(&mut self) {
        if let Some(clip) = self.clip.take() {
            // A recording from the replay capture: save its stretch; the buffer keeps running.
            // Named after what was on screen for most of it (e.g. the game).
            send_event("status", json!({ "state": "saving" }));
            let now = platform::now_hns();
            // The name is picked now, not when the recording started: an Echo saved in the
            // meantime may have taken the name chosen then, and it must never be overwritten.
            let label = self.active.as_ref().map(|a| a.label_for(clip.from, now)).unwrap_or(clip.label);
            let out_path = recorder::output_path(&PathBuf::from(&self.settings.output_dir), &label, clip.local);
            self.save_in_background(clip.from, now, out_path, label, false);
            return;
        }
        match self.active.take() {
            Some(active) if active.is_replay() => self.active = Some(active),
            Some(mut active) => {
                let label = active.label_for(0, platform::now_hns());
                if label != active.label {
                    let path = recorder::output_path(&PathBuf::from(&self.settings.output_dir), &label, active.started_local);
                    active.set_output(label, path);
                }
                let label = active.label.clone();
                send_event("status", json!({ "state": "saving" }));
                report_saved(active.stop_and_save(&self.ffmpeg), &label, false);
            }
            None => {}
        }
    }

    /// Instant replay: lets go of video and audio older than anything still needed.
    fn prune_replay(&self) {
        let Some(a) = self.active.as_ref().filter(|a| a.is_replay()) else { return };
        let now = platform::now_hns();
        // The replay length plus a margin; never past a recording or save still in progress.
        let mut keep_from = now.saturating_sub((self.settings.replay_seconds as u64 + 6) * 10_000_000);
        if let Some(c) = &self.clip {
            keep_from = keep_from.min(c.from);
        }
        if let Some(&p) = self.pins.lock().unwrap().iter().min() {
            keep_from = keep_from.min(p);
        }
        // Keep the segment that holds `keep_from` (a save starts at its keyframe).
        a.prune(keep_from.saturating_sub(3 * 10_000_000));
    }
}

fn report_replay_saved(result: Result<Saved, String>, label: &str) {
    match result {
        Ok(s) => send_event(
            "replay_saved",
            json!({
                "path": s.path.display().to_string(),
                "duration_ms": s.duration_ms,
                "size_bytes": s.size_bytes,
                "label": label,
            }),
        ),
        Err(e) => {
            log(&format!("replay save failed: {e}"));
            send_event("error", json!({ "message": format!("Couldn't save the Echo: {e}") }));
        }
    }
}

fn report_saved(result: Result<Saved, String>, label: &str, recovered: bool) {
        match result {
            Ok(s) => send_event(
                "recording_saved",
                json!({
                    "path": s.path.display().to_string(),
                    "duration_ms": s.duration_ms,
                    "size_bytes": s.size_bytes,
                    "label": label,
                    "recovered": recovered,
                }),
            ),
            Err(e) => {
                log(&format!("save failed: {e}"));
                send_event("error", json!({ "message": e }));
            }
        }
        send_event("status", json!({ "state": "idle" }));
    }

impl Engine {

    /// Keeps track of the monitor being used, and moves a following recording there once the
    /// focus has settled on it.
    fn follow_focus(&mut self) {
        if let Some(m) = platform::focused_monitor() {
            if self.focus != Some(m) {
                self.focus = Some(m);
                send_event("focus_monitor", json!({ "id": monitor_id(m) }));
            }
        }
        let Some(active) = self.active.as_mut() else { return };
        if !self.following {
            return;
        }
        let Some(want) = self.focus else { return };
        let current = match active.switching_to().unwrap_or(active.target()) {
            Target::Monitor { handle } => handle,
            Target::Window { .. } => return,
        };
        if want == current {
            self.focus_candidate = None;
            return;
        }
        match self.focus_candidate {
            Some((m, since)) if m == want => {
                if since.elapsed() >= SWITCH_SETTLE {
                    self.focus_candidate = None;
                    log(&format!("focus moved to monitor {want}: switching"));
                    self.switch_capture(Target::Monitor { handle: want }, "Desktop");
                }
            }
            _ => self.focus_candidate = Some((want, Instant::now())),
        }
    }

    fn target_key(t: Target) -> String {
        match t {
            Target::Monitor { handle } => format!("m{handle}"),
            Target::Window { handle } => format!("w{handle}"),
        }
    }

    /// False while a target that failed recently is waiting before the next try.
    fn may_switch_to(&self, t: Target) -> bool {
        match self.switch_fails.get(&Self::target_key(t)) {
            None => true,
            Some((n, at)) => at.elapsed() >= Duration::from_secs((10u64 << (n.saturating_sub(1)).min(4)).min(120)),
        }
    }

    /// Starts a switch unless that target is waiting after failing.
    fn switch_capture(&mut self, t: Target, label: &str) -> bool {
        if !self.may_switch_to(t) {
            return false;
        }
        match self.active.as_mut() {
            Some(a) => {
                a.switch_to(&self.ffmpeg, t, label);
                true
            }
            None => false,
        }
    }

    /// A switch didn't start: wait before trying that target again, and say so once, plainly.
    fn switch_failed(&mut self, target: Target, detail: &str) {
        let key = Self::target_key(target);
        let n = self.switch_fails.get(&key).map(|(n, _)| n + 1).unwrap_or(1);
        self.switch_fails.insert(key, (n, Instant::now()));
        // FFmpeg's own reason, without the start-up notices that aren't errors.
        let reason: Vec<&str> = detail
            .lines()
            .filter(|l| !l.contains("minimum update interval") && !l.trim().is_empty())
            .collect();
        log(&format!("switch to {target:?} failed (attempt {n}): {}", reason.join(" / ")));
        // A graphics card error while Priority Capture is on: turn it off.
        let lower = detail.to_ascii_lowercase();
        if ["nvenc", "amf", "qsv", "d3d", "device", "encoder"].iter().any(|w| lower.contains(w)) {
            self.priority_off("capture_error", "Priority Capture turned off: the capture stopped with an error while it was on.");
        }
        if n > 1 {
            return; // told them the first time
        }
        let what = match target {
            Target::Window { handle } => self
                .watch
                .games
                .iter()
                .find(|g| g.handle == handle)
                .map(|g| g.name.clone())
                .unwrap_or_else(|| "that window".into()),
            Target::Monitor { .. } => "the other monitor".into(),
        };
        send_event(
            "warning",
            json!({ "message": format!("Couldn't start recording {what}, so Encore carried on with what it was recording. It will try again in a little while.") }),
        );
    }

    fn pick(&self) -> Pick {
        if self.settings.game_pick == "focused" { Pick::Focused } else { Pick::Newest }
    }

    /// The game to record now, if game detection is on and one is open.
    fn current_game(&self) -> Option<games::Game> {
        if !self.settings.game_detection {
            return None;
        }
        self.watch.active(self.pick()).cloned()
    }

    /// Looks for games: once a second, or straight away when `now`.
    fn scan_games(&mut self, now: bool) {
        if !self.settings.game_detection {
            if !self.watch.games.is_empty() || self.watch.locked.is_some() {
                self.watch = Watch::default();
            }
            self.send_games(false);
            return;
        }
        if !now && self.last_game_scan.map(|t| t.elapsed() < Duration::from_secs(1)).unwrap_or(false) {
            return;
        }
        self.last_game_scan = Some(Instant::now());
        let mut found = Vec::new();
        let mut still_fullscreen = Vec::new();
        for w in platform::top_windows() {
            let Some(why) = games::classify(&w, &self.settings.games_always, &self.settings.games_never) else { continue };
            if why == games::Why::Fullscreen {
                // Only a guess: it has to stay for a few seconds first.
                still_fullscreen.push(w.handle);
                let since = *self.fullscreen_seen.entry(w.handle).or_insert_with(Instant::now);
                if since.elapsed() < FULLSCREEN_SETTLE {
                    continue;
                }
            }
            let name = match self.game_names.get(&w.pid) {
                Some(n) => n.clone(),
                None => {
                    let n = games::game_name(&w, &platform::app_display_name(w.pid));
                    log(&format!("game found: {n} ({}, {:?})", w.exe, why));
                    self.game_names.insert(w.pid, n.clone());
                    n
                }
            };
            found.push((w, name, why));
        }
        self.fullscreen_seen.retain(|h, _| still_fullscreen.contains(h));
        let alive: Vec<u32> = found.iter().map(|f| f.0.pid).collect();
        self.game_names.retain(|p, _| alive.contains(p));
        self.watch.update(found, platform::foreground_window());
        self.send_games(false);
    }

    fn games_json(&self) -> Value {
        let active = self.current_game();
        let mut list = self.watch.games.clone();
        list.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        json!({
            "on": self.settings.game_detection,
            // Select Application is being recorded: detection waits until a screen is picked again.
            "paused": self.settings.game_detection && self.active.is_some() && self.session_source == "window",
            "games": list,
            "active": active.as_ref().map(|g| g.pid),
            "active_name": active.map(|g| g.name),
            "locked": self.watch.locked,
        })
    }

    /// Tells the interface about the games, when something changed (or always, when `force`).
    fn send_games(&mut self, force: bool) {
        let v = self.games_json();
        let sig = v.to_string();
        if force || sig != self.games_sent {
            self.games_sent = sig;
            send_event("games", v);
        }
    }

    /// Moves the capture to the game being played, and back to the chosen screen when games close.
    fn follow_games(&mut self) {
        // A recorded game window that has just closed: look again now rather than within a second,
        // so the hand-over to the screen happens before the capture is taken as having failed.
        let gone = self
            .active
            .as_ref()
            .map(|a| matches!(a.target(), Target::Window { handle } if self.session_source == "desktop" && !platform::window_exists(handle)))
            .unwrap_or(false);
        self.scan_games(gone);
        // Instant replay that only keeps games: start when one opens.
        if self.replay_waiting && self.active.is_none() && self.current_game().is_some() {
            log("a game opened: starting instant replay");
            self.replay_waiting = false;
            if let Err(e) = self.start_session(&json!({ "source": "desktop" }), true) {
                self.replay_waiting = true;
                send_event("warning", json!({ "message": format!("Couldn't start Echoes for the game: {e}") }));
            }
            return;
        }
        let Some(active) = self.active.as_ref() else { return };
        if self.session_source != "desktop" {
            return;
        }
        let current = active.switching_to().unwrap_or(active.target());
        match (self.current_game(), current) {
            (Some(g), Target::Window { handle }) if handle == g.handle => {}
            (Some(g), _) => {
                if self.switch_capture(Target::Window { handle: g.handle }, &g.name) {
                    log(&format!("game: recording {}", g.name));
                    self.focus_candidate = None;
                    send_event("game_switched", json!({ "name": g.name }));
                }
            }
            (None, Target::Window { .. }) => self.leave_game(),
            (None, Target::Monitor { .. }) => {}
        }
    }

    /// The game being recorded has closed (or game detection was turned off).
    fn leave_game(&mut self) {
        let replay = self.active.as_ref().map(|a| a.is_replay()).unwrap_or(false);
        let detection = self.settings.game_detection;
        // Desktop Capture off: nothing is recorded outside games, so a recording ends with its game.
        let games_only = detection && !self.settings.desktop_capture;
        if (games_only || (detection && self.settings.game_end == "stop")) && self.is_recording() {
            log("game closed: stopping the recording");
            self.stop_recording();
            if !replay {
                return;
            }
        }
        if replay && games_only {
            log("game closed: instant replay waits for the next game");
            if self.clip.is_some() {
                self.stop_recording();
            }
            self.wait_for_saves();
            if let Some(a) = self.active.take() {
                a.discard();
            }
            self.replay_waiting = true;
            send_event("replay_status", json!({ "on": true, "waiting": true, "seconds": self.settings.replay_seconds }));
            send_event("game_switched", json!({ "name": null }));
            return;
        }
        match self.pick_monitor() {
            Ok((target, ..)) => {
                log("game closed: back to the chosen screen");
                self.focus_candidate = None;
                // Always allowed: the game's capture has ended, so the screen is the way out.
                self.switch_fails.remove(&Self::target_key(target));
                self.switch_capture(target, "Desktop");
                send_event("game_switched", json!({ "name": null }));
            }
            Err(e) => log(&format!("game closed, but no screen to go back to: {e}")),
        }
    }

    /// Priority Capture turns itself off once no game has been detected for PRIORITY_NO_GAME (the
    /// game closed, or none was found after turning it on).
    fn priority_watch(&mut self) {
        if !self.settings.gpu_realtime {
            self.no_game_since = None;
            return;
        }
        if self.current_game().is_some() {
            self.no_game_since = None;
            return;
        }
        let since = *self.no_game_since.get_or_insert_with(Instant::now);
        if since.elapsed() >= PRIORITY_NO_GAME {
            self.priority_off("no_game", "Priority Capture turned off: no game has been detected for a minute.");
        }
    }

    /// Turns Priority Capture off by itself, lowers the running capture, and tells the interface.
    /// Before a capture starts: if Windows gave the chosen microphone a new id, use the new one
    /// (found by its name) and tell the interface. Also fills in the name for older settings.
    fn find_mic(&mut self) {
        if !self.settings.audio_mic || self.settings.mic_device == "default" {
            return;
        }
        let inputs = audio::list_inputs();
        match audio::pick_mic(&inputs, &self.settings.mic_device, &self.settings.mic_name) {
            audio::MicPick::Same => {
                if self.settings.mic_name.is_empty() {
                    if let Some(d) = inputs.iter().find(|d| d.id == self.settings.mic_device) {
                        self.settings.mic_name = d.name.clone();
                        let _ = self.settings.save();
                    }
                }
            }
            audio::MicPick::Moved(id) => {
                log(&format!("microphone {} has a new id: {} (was {})", self.settings.mic_name, id, self.settings.mic_device));
                self.settings.mic_device = id;
                let _ = self.settings.save();
                send_event(
                    "mic_found",
                    json!({ "name": self.settings.mic_name, "settings": serde_json::to_value(&self.settings).unwrap_or(Value::Null) }),
                );
            }
            audio::MicPick::Missing => log(&format!("chosen microphone {} isn't connected", if self.settings.mic_name.is_empty() { &self.settings.mic_device } else { &self.settings.mic_name })),
            audio::MicPick::Default => {}
        }
    }

    fn priority_off(&mut self, reason: &str, message: &str) {
        if !self.settings.gpu_realtime {
            return;
        }
        self.settings.gpu_realtime = false;
        let _ = self.settings.save();
        platform::set_gpu_top(false);
        self.no_game_since = None;
        log(&format!("Priority Capture off by itself ({reason})"));
        if let Some(a) = &self.active {
            a.reprioritise();
        }
        send_event(
            "priority_capture_off",
            json!({ "reason": reason, "message": message, "settings": serde_json::to_value(&self.settings).unwrap_or(Value::Null) }),
        );
    }

    /// Called regularly from the main loop: follows games and the focus, notices if FFmpeg
    /// stopped on its own.
    pub fn tick(&mut self) {
        self.follow_games();
        self.priority_watch();
        self.follow_focus();
        if let Some(event) = self.active.as_mut().and_then(|a| a.poll()) {
            match event {
                recorder::SwitchEvent::Switched(t @ Target::Monitor { handle }) => {
                    self.switch_fails.remove(&Self::target_key(t));
                    send_event("monitor_switched", json!({ "monitor": monitor_id(handle) }))
                }
                recorder::SwitchEvent::Switched(t) => {
                    self.switch_fails.remove(&Self::target_key(t));
                }
                recorder::SwitchEvent::Failed(target, detail) => self.switch_failed(target, &detail),
            }
        }
        self.prune_replay();
        // A capture that ended while another is taking over (a game closed) isn't a failure.
        let stopped = matches!(self.active.as_mut().map(|a| a.has_exited() && a.switching_to().is_none()), Some(true));
        if stopped && self.active.as_ref().map(|a| a.is_replay()).unwrap_or(false) {
            // The replay capture stopped (e.g. the recorded window closed): keep anything being
            // recorded from it, then turn replay off.
            let detail = self.active.as_ref().map(|a| tail(&a.log_path(), 4)).unwrap_or_default();
            log(&format!("replay capture ended unexpectedly: {detail}"));
            self.priority_off("capture_error", "Priority Capture turned off: the capture stopped with an error while it was on.");
            send_event("warning", json!({ "message": "Echoes stopped because what it was capturing went away (for example the window closed). Turn it on again to carry on." }));
            self.stop_replay();
        } else if stopped {
            let active = self.active.take().unwrap();
            let detail = tail(&active.log_path(), 4);
            log(&format!("capture ended unexpectedly: {detail}"));
            self.priority_off("capture_error", "Priority Capture turned off: the capture stopped with an error while it was on.");
            let reason = if detail.is_empty() { String::new() } else { format!(" FFmpeg said: {detail}") };
            send_event("warning", json!({ "message": format!("Recording stopped unexpectedly (for example the window closed). Saving what was captured.{reason}") }));
            let label = active.label.clone();
            report_saved(active.stop_and_save(&self.ffmpeg), &label, false);
        }
    }

    /// Where recordings in progress and the instant replay buffer go (setting `work_dir`).
    fn work_root(&self) -> PathBuf {
        paths::work_root(&self.settings.work_dir)
    }

    /// Saves recordings left in the working folder by a crash or power cut (the default one, and
    /// the chosen one if different).
    pub fn recover_leftovers(&self) {
        let mut roots = vec![paths::temp_dir()];
        if self.work_root() != roots[0] {
            roots.push(self.work_root());
        }
        for dir in roots.iter().flat_map(|r| recorder::leftover_sessions(r)) {
            log(&format!("recovering {}", dir.display()));
            let out_dir = PathBuf::from(&self.settings.output_dir);
            let out_path = recorder::output_path(&out_dir, "Recovered", platform::local_time());
            report_saved(recorder::finalise(&self.ffmpeg, &dir, &out_path, 0), "Recovered", true);
        }
    }

    pub fn shutdown(&mut self) {
        if self.is_recording() {
            log("shutting down while recording: saving first");
            self.stop_recording();
        }
        // Finish any replay saves, then let the buffer go.
        self.stop_replay();
    }
}

#[cfg(test)]
mod folder_tests {
    use super::*;

    #[test]
    fn checking_a_save_folder_creates_nothing() {
        let base = std::env::temp_dir().join(format!("encore-wtest-{}", std::process::id()));
        std::fs::create_dir_all(&base).unwrap();
        let chosen = base.join("Clips"); // doesn't exist yet
        let root = paths::encore_root(&chosen);
        assert!(check_writable(&root).is_ok());
        assert!(!chosen.exists() && !root.exists(), "no folders made by the check");
        assert_eq!(std::fs::read_dir(&base).unwrap().count(), 0, "no test file left behind");
        let _ = std::fs::remove_dir_all(&base);
    }
}
