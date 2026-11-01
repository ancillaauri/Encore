//! Engine state and command handling. One command at a time, on the main thread.

use crate::audio::{self, AudioOptions};
use crate::ffmpeg::{self, CaptureSpec, Encoder, Pipeline, Target, ALL_CODECS, VENDOR_ORDER};
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
}

/// A recording taken from the instant-replay capture: from `from` until it is stopped.
struct Clip {
    from: u64,
    out_path: PathBuf,
    label: String,
}

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
        Engine {
            ffmpeg,
            settings: Settings::load(),
            caps,
            active: None,
            known_pipeline: HashMap::new(),
            focus: None,
            focus_candidate: None,
            following: false,
            clip: None,
            pins: Arc::new(Mutex::new(Vec::new())),
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
            "list_audio_apps" => Ok(serde_json::to_value(audio::list_apps()).unwrap_or(Value::Null)),
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
            "save_replay" => self.save_replay(),
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
            }),
            _ => Value::Null,
        };
        json!({
            "state": if self.is_recording() { "recording" } else { "idle" },
            "recording": recording,
            "replay": replay,
            "focus_monitor": focus,
        })
    }

    fn set_settings(&mut self, patch: &Value) -> Result<Value, String> {
        let updated = self.settings.apply_patch(patch)?;
        if updated.output_dir != self.settings.output_dir {
            check_writable(&paths::encore_root(&PathBuf::from(&updated.output_dir)))?;
        }
        // Setup finished: now make the Encore folder where they chose.
        if updated.setup_done && !self.settings.setup_done {
            let root = paths::encore_root(&PathBuf::from(&updated.output_dir));
            if let Err(e) = std::fs::create_dir_all(&root) {
                log(&format!("could not create {}: {e}", root.display()));
            }
        }
        updated.save()?;
        self.settings = updated;
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
            self.clip = Some(Clip { from: now, out_path, label: a.label.clone() });
            send_event("recording_started", info.clone());
            return Ok(info);
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
        let (target, label, src_w, src_h, describe) = match source {
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
        let spec = CaptureSpec { target, fps: self.settings.fps, cursor: self.settings.capture_cursor, width, height };
        let kbps = ffmpeg::bitrate_kbps(width, height, spec.fps, self.settings.quality, codec);
        let out_dir = PathBuf::from(&self.settings.output_dir);
        let out_path = recorder::output_path(&out_dir, &label, platform::local_time());
        let preferred = self.known_pipeline.get(encoder.name()).copied();
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
            &paths::temp_dir(),
            preferred,
            &audio_opts,
            self.settings.audio_offset_ms,
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
            Some(_) => return Err("Stop the recording first, then turn on instant replay.".into()),
            None => {}
        }
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

    /// Saves the last `replay_seconds` without stopping the buffer.
    fn save_replay(&mut self) -> Result<Value, String> {
        let Some(a) = self.active.as_ref().filter(|a| a.is_replay()) else {
            return Err("Instant replay is off. Turn it on first.".into());
        };
        let to = platform::now_hns();
        let from = to.saturating_sub(self.settings.replay_seconds as u64 * 10_000_000);
        let out_path = recorder::output_path(&PathBuf::from(&self.settings.output_dir), &a.label, platform::local_time());
        self.save_in_background(from, to, out_path, a.label.clone(), true);
        Ok(json!({ "saving": true }))
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
            send_event("replay_saving", json!({}));
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
            send_event("status", json!({ "state": "saving" }));
            self.save_in_background(clip.from, platform::now_hns(), clip.out_path, clip.label, false);
            return;
        }
        match self.active.take() {
            Some(active) if active.is_replay() => self.active = Some(active),
            Some(active) => {
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
            send_event("error", json!({ "message": format!("Couldn't save the replay: {e}") }));
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
                    active.switch_to(&self.ffmpeg, Target::Monitor { handle: want });
                }
            }
            _ => self.focus_candidate = Some((want, Instant::now())),
        }
    }

    /// Called regularly from the main loop: follows the focus, notices if FFmpeg stopped on its own.
    pub fn tick(&mut self) {
        self.follow_focus();
        if let Some(event) = self.active.as_mut().and_then(|a| a.poll()) {
            match event {
                recorder::SwitchEvent::Switched(Target::Monitor { handle }) => {
                    send_event("monitor_switched", json!({ "monitor": monitor_id(handle) }))
                }
                recorder::SwitchEvent::Switched(_) => {}
                recorder::SwitchEvent::Failed(detail) => send_event(
                    "warning",
                    json!({ "message": format!("Couldn't move the recording to the other monitor, so it carries on where it was. FFmpeg said: {detail}") }),
                ),
            }
        }
        self.prune_replay();
        let stopped = matches!(self.active.as_mut().map(|a| a.has_exited()), Some(true));
        if stopped && self.active.as_ref().map(|a| a.is_replay()).unwrap_or(false) {
            // The replay capture stopped (e.g. the recorded window closed): keep anything being
            // recorded from it, then turn replay off.
            let detail = self.active.as_ref().map(|a| tail(&a.log_path(), 4)).unwrap_or_default();
            log(&format!("replay capture ended unexpectedly: {detail}"));
            send_event("warning", json!({ "message": "Instant replay stopped because what it was capturing went away (for example the window closed). Turn it on again to carry on." }));
            self.stop_replay();
        } else if stopped {
            let active = self.active.take().unwrap();
            let detail = tail(&active.log_path(), 4);
            log(&format!("capture ended unexpectedly: {detail}"));
            let reason = if detail.is_empty() { String::new() } else { format!(" FFmpeg said: {detail}") };
            send_event("warning", json!({ "message": format!("Recording stopped unexpectedly (for example the window closed). Saving what was captured.{reason}") }));
            let label = active.label.clone();
            report_saved(active.stop_and_save(&self.ffmpeg), &label, false);
        }
    }

    /// Saves recordings left in the temp folder by a crash or power cut.
    pub fn recover_leftovers(&self) {
        for dir in recorder::leftover_sessions(&paths::temp_dir()) {
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
