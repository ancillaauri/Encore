//! Audio capture for a recording: the full mix of everything you hear, the microphone, and one
//! track per app that plays sound. Each source runs on its own thread and writes 48 kHz stereo
//! 16-bit PCM to its own file in the session folder, aligned to the session origin (see timeline.rs).
//!
//! The session folder also holds `audio.json` (the manifest) so a crashed recording can be
//! rebuilt with its audio tracks.

// Capture itself only exists on Windows; elsewhere (tests) parts are unused.
#![cfg_attr(not(windows), allow(dead_code, unused_imports))]

use crate::log::log;
use crate::protocol::send_event;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

pub const MANIFEST: &str = "audio.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TrackKind {
    FullMix,
    Mic,
    App,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrackInfo {
    /// File name inside the session folder.
    pub file: String,
    /// Track title written into the MP4 (the companion editor shows it).
    pub title: String,
    pub kind: TrackKind,
}

/// What a recording session needs to rebuild its file: timing and tracks.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Manifest {
    pub origin_hns: u64,
    pub video_start_hns: Option<u64>,
    pub audio_offset_ms: i32,
    pub tracks: Vec<TrackInfo>,
    /// Video parts, in order: a new part starts each time recording moves to another monitor.
    /// Empty in sessions from before 0.5 (one part, files `seg_*.mkv`).
    #[serde(default)]
    pub parts: Vec<VideoPart>,
}

/// One FFmpeg run's segment files and when its video started (same clock as `origin_hns`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct VideoPart {
    /// File name prefix of this part's segments, e.g. `seg_` or `p001_`.
    pub prefix: String,
    pub start_hns: Option<u64>,
    /// A part that started but was abandoned (it never took over): left out of the file.
    #[serde(default)]
    pub dropped: bool,
}

impl Manifest {
    pub fn save(&self, dir: &Path) {
        if let Ok(text) = serde_json::to_string_pretty(self) {
            let tmp = dir.join("audio.json.tmp");
            if std::fs::write(&tmp, text).is_ok() {
                let _ = std::fs::rename(&tmp, dir.join(MANIFEST));
            }
        }
    }

    pub fn load(dir: &Path) -> Option<Manifest> {
        serde_json::from_str(&std::fs::read_to_string(dir.join(MANIFEST)).ok()?).ok()
    }
}

#[derive(Debug, Clone)]
pub struct AudioOptions {
    pub full_mix: bool,
    pub mic: bool,
    /// "default" or a device id from `list_inputs`.
    pub mic_device: String,
    pub app_tracks: bool,
    pub max_app_tracks: usize,
    /// Instant replay: write each track in 10-second pieces, deleting pieces that end before this
    /// frame position (kept up to date by the engine). None: one plain file per track.
    pub roll_keep_from: Option<Arc<AtomicU64>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InputDevice {
    pub id: String,
    pub name: String,
    pub default: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct AudioApp {
    pub pid: u32,
    pub title: String,
    pub exe: String,
}

/// One audio track in the saved file, built from one or more recorded sources (indices into
/// `Manifest::tracks`). Several sources are mixed together.
#[derive(Debug, Clone, PartialEq)]
pub struct OutTrack {
    pub title: String,
    pub sources: Vec<usize>,
}

/// Which tracks the saved file gets, in order. Players use the first one, so it is everything:
/// - `Full mix`: system sound (or, if that is off, every app) plus the microphone, mixed
/// - `Microphone` on its own
/// - `Desktop audio`: system sound without the mic (only when there is a mic to leave out). The
///   interface calls this source "Everything You Hear"; the track title stays (file contract).
/// - one track per app
/// A track that would just repeat another (e.g. Full mix with nothing to mix in) is left out.
pub fn plan_output(tracks: &[TrackInfo]) -> Vec<OutTrack> {
    let idx = |k: TrackKind| tracks.iter().position(|t| t.kind == k);
    let apps: Vec<usize> = (0..tracks.len()).filter(|&i| tracks[i].kind == TrackKind::App).collect();
    let desktop = idx(TrackKind::FullMix);
    let mic = idx(TrackKind::Mic);
    let mut mix: Vec<usize> = match desktop {
        Some(d) => vec![d],
        None => apps.clone(),
    };
    if let Some(m) = mic {
        mix.push(m);
    }
    let mut out = Vec::new();
    if !mix.is_empty() {
        out.push(OutTrack { title: "Full mix".into(), sources: mix.clone() });
    }
    if let Some(m) = mic {
        if mix.len() > 1 {
            out.push(OutTrack { title: "Microphone".into(), sources: vec![m] });
        }
    }
    if let (Some(d), Some(_)) = (desktop, mic) {
        out.push(OutTrack { title: "Desktop audio".into(), sources: vec![d] });
    }
    // Apps are separate tracks unless one of them is already the whole Full mix on its own.
    if !(desktop.is_none() && mic.is_none() && apps.len() == 1) {
        for &a in &apps {
            out.push(OutTrack { title: tracks[a].title.clone(), sources: vec![a] });
        }
    }
    out
}

/// Gives each new app track a unique title ("Discord", "Discord 2", ...).
pub fn unique_title(existing: &[TrackInfo], wanted: &str) -> String {
    let taken = |t: &str| existing.iter().any(|x| x.title.eq_ignore_ascii_case(t));
    let reserved = |t: &str| {
        ["Full mix", "Microphone", "Desktop audio"].iter().any(|r| t.eq_ignore_ascii_case(r))
    };
    let base = if wanted.trim().is_empty() { "App" } else { wanted.trim() };
    if !taken(base) && !reserved(base) {
        return base.to_string();
    }
    (2..100).map(|n| format!("{base} {n}")).find(|t| !taken(t)).unwrap_or_else(|| base.to_string())
}

/// A running audio capture. Dropping it without `stop` leaves threads running, so always stop it.
pub struct AudioSession {
    dir: PathBuf,
    manifest: Arc<Mutex<Manifest>>,
    stop: Arc<AtomicBool>,
    stop_at: Arc<AtomicU64>,
    threads: Arc<Mutex<Vec<JoinHandle<()>>>>,
    monitor: Option<JoinHandle<()>>,
}

impl AudioSession {
    pub fn manifest(&self) -> Arc<Mutex<Manifest>> {
        self.manifest.clone()
    }

    /// The tracks the saved file will have, as things stand.
    pub fn track_titles(&self) -> Vec<String> {
        plan_output(&self.manifest.lock().unwrap().tracks).into_iter().map(|t| t.title).collect()
    }

    /// Stops every source at `end_hns`, pads all tracks to that moment, and returns the manifest.
    pub fn stop(mut self, end_hns: u64) -> Manifest {
        self.stop_at.store(end_hns, Ordering::SeqCst);
        self.stop.store(true, Ordering::SeqCst);
        if let Some(m) = self.monitor.take() {
            let _ = m.join();
        }
        let handles: Vec<JoinHandle<()>> = std::mem::take(&mut *self.threads.lock().unwrap());
        for h in handles {
            let _ = h.join();
        }
        let m = self.manifest.lock().unwrap().clone();
        m.save(&self.dir);
        m
    }
}

fn track_file(index: usize, kind: &TrackKind) -> String {
    let tag = match kind {
        TrackKind::FullMix => "full",
        TrackKind::Mic => "mic",
        TrackKind::App => "app",
    };
    format!("a{index:02}_{tag}.pcm")
}

/// Adds a track to the manifest (and saves it) and returns its file path.
fn add_track(dir: &Path, manifest: &Arc<Mutex<Manifest>>, title: String, kind: TrackKind) -> (PathBuf, String) {
    let mut m = manifest.lock().unwrap();
    let title = if kind == TrackKind::App { unique_title(&m.tracks, &title) } else { title };
    let file = track_file(m.tracks.len(), &kind);
    m.tracks.push(TrackInfo { file: file.clone(), title: title.clone(), kind });
    m.save(dir);
    (dir.join(file), title)
}

#[cfg(not(windows))]
pub fn start(dir: &Path, origin_hns: u64, audio_offset_ms: i32, _opts: &AudioOptions) -> AudioSession {
    let manifest = Manifest { origin_hns, video_start_hns: None, audio_offset_ms, tracks: Vec::new(), parts: Vec::new() };
    manifest.save(dir);
    AudioSession {
        dir: dir.to_path_buf(),
        manifest: Arc::new(Mutex::new(manifest)),
        stop: Arc::new(AtomicBool::new(false)),
        stop_at: Arc::new(AtomicU64::new(0)),
        threads: Arc::new(Mutex::new(Vec::new())),
        monitor: None,
    }
}

#[cfg(not(windows))]
pub fn list_inputs() -> Vec<InputDevice> {
    Vec::new()
}

#[cfg(not(windows))]
pub fn list_apps() -> Vec<AudioApp> {
    Vec::new()
}

#[cfg(windows)]
pub use win::{list_apps, list_inputs, start};

#[cfg(windows)]
mod win {
    use super::*;
    use crate::platform;
    use crate::timeline::{frames_to_hns, Timeline};
    use std::collections::HashSet;
    use std::io::BufWriter;
    use std::time::Duration;
    use wasapi::{
        initialize_mta, AudioClient, DeviceEnumerator, Direction, SampleType, SessionState, StreamMode, WaveFormat,
    };

    enum Source {
        FullMix,
        Mic(String),
        App(u32),
    }

    fn e<T, E: std::fmt::Display>(r: Result<T, E>) -> Result<T, String> {
        r.map_err(|err| err.to_string())
    }

    fn open_client(source: &Source) -> Result<AudioClient, String> {
        match source {
            Source::FullMix => {
                // Everything except Encore itself, so its start/stop/save sounds aren't recorded.
                // Needs a recent Windows; otherwise the whole output device is recorded as before.
                if let Some(shell) = shell_pid(&platform::processes()) {
                    match AudioClient::new_application_loopback_client(shell, false) {
                        Ok(c) => return Ok(c),
                        Err(err) => log(&format!("desktop audio without Encore's sounds not available ({err}); recording all of it")),
                    }
                }
                let en = e(DeviceEnumerator::new())?;
                let dev = e(en.get_default_device(&Direction::Render))?;
                e(dev.get_iaudioclient())
            }
            Source::Mic(id) => {
                let en = e(DeviceEnumerator::new())?;
                if id != "default" {
                    for dev in &e(en.get_device_collection(&Direction::Capture))? {
                        let dev = e(dev)?;
                        if e(dev.get_id())? == *id {
                            return e(dev.get_iaudioclient());
                        }
                    }
                    log(&format!("chosen microphone {id} not found; using the default"));
                }
                let dev = e(en.get_default_device(&Direction::Capture))?;
                e(dev.get_iaudioclient())
            }
            Source::App(pid) => e(AudioClient::new_application_loopback_client(*pid, true)),
        }
    }

    /// Captures one source until told to stop, writing it onto its timeline.
    fn capture(
        source: Source,
        file: PathBuf,
        title: String,
        origin: u64,
        stop: Arc<AtomicBool>,
        stop_at: Arc<AtomicU64>,
        roll: Option<Arc<AtomicU64>>,
    ) {
        let _ = initialize_mta();
        let out: Box<dyn std::io::Write + Send> = match roll {
            Some(keep_from) => Box::new(crate::timeline::ChunkedWriter::new(file.clone(), keep_from)),
            None => match std::fs::File::create(&file) {
                Ok(f) => Box::new(BufWriter::with_capacity(256 * 1024, f)),
                Err(err) => {
                    log(&format!("audio {title}: cannot create {}: {err}", file.display()));
                    return;
                }
            },
        };
        let mut timeline = Timeline::new(origin, out);
        let result = (|| -> Result<(), String> {
            let mut client = open_client(&source)?;
            let format = WaveFormat::new(32, 32, &SampleType::Float, 48_000, 2, None);
            let mode = StreamMode::EventsShared { autoconvert: true, buffer_duration_hns: 0 };
            e(client.initialize_client(&format, &Direction::Capture, &mode))?;
            let event = e(client.set_get_eventhandle())?;
            let capture = e(client.get_audiocaptureclient())?;
            e(client.start_stream())?;
            let mut buf: Vec<u8> = Vec::new();
            while !stop.load(Ordering::SeqCst) {
                // Times out while the source is silent; that's normal.
                let _ = event.wait_for_event(100);
                loop {
                    let frames = e(capture.get_next_packet_size())?.unwrap_or(0) as usize;
                    if frames == 0 {
                        break;
                    }
                    buf.resize(frames * 8, 0);
                    let (got, info) = e(capture.read_from_device(&mut buf))?;
                    let got = got as usize;
                    let ts = if info.timestamp == 0 || info.flags.timestamp_error {
                        platform::now_hns().saturating_sub(frames_to_hns(got as u64))
                    } else {
                        info.timestamp
                    };
                    let written = if info.flags.silent {
                        timeline.push_silence(ts, got)
                    } else {
                        timeline.push_f32_bytes(ts, &buf[..got * 8])
                    };
                    written.map_err(|err| format!("writing audio failed: {err}"))?;
                }
            }
            let _ = client.stop_stream();
            Ok(())
        })();
        if let Err(err) = result {
            log(&format!("audio {title}: {err}"));
            send_event("warning", json!({ "message": format!("Couldn't record {title}: {err}. That track will be silent.") }));
            // Keep the track silent until the end rather than dropping it.
            while !stop.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(100));
            }
        }
        let end = stop_at.load(Ordering::SeqCst).max(origin);
        if let Err(err) = timeline.finish(end) {
            log(&format!("audio {title}: finishing failed: {err}"));
        }
    }

    fn spawn(
        threads: &Arc<Mutex<Vec<JoinHandle<()>>>>,
        source: Source,
        file: PathBuf,
        title: String,
        origin: u64,
        stop: &Arc<AtomicBool>,
        stop_at: &Arc<AtomicU64>,
        roll: &Option<Arc<AtomicU64>>,
    ) {
        let (stop, stop_at, roll) = (stop.clone(), stop_at.clone(), roll.clone());
        let name = format!("audio-{title}");
        match std::thread::Builder::new().name(name).spawn(move || capture(source, file, title, origin, stop, stop_at, roll)) {
            Ok(h) => threads.lock().unwrap().push(h),
            Err(err) => log(&format!("could not start audio thread: {err}")),
        }
    }

    /// Active audio sessions on the default output, grouped by app (one entry per app root process).
    fn active_apps(own: &HashSet<u32>) -> Result<Vec<AudioApp>, String> {
        let en = e(DeviceEnumerator::new())?;
        let dev = e(en.get_default_device(&Direction::Render))?;
        let manager = e(dev.get_iaudiosessionmanager())?;
        let sessions = e(manager.get_audiosessionenumerator())?;
        let procs = platform::processes();
        let mut seen = HashSet::new();
        let mut apps = Vec::new();
        for i in 0..e(sessions.get_count())? {
            let control = e(sessions.get_session(i))?;
            if e(control.get_state())? != SessionState::Active {
                continue;
            }
            let pid = e(control.get_process_id())?;
            if pid == 0 || own.contains(&pid) {
                continue; // 0 = Windows system sounds
            }
            let root = platform::app_root(pid, &procs);
            if own.contains(&root) || !seen.insert(root) {
                continue;
            }
            let exe = procs.get(&root).map(|p| p.exe.clone()).unwrap_or_default();
            apps.push(AudioApp { pid: root, title: platform::app_display_name(root), exe });
        }
        Ok(apps)
    }

    /// The Encore app (the shell that started this engine), if that's how the engine was started.
    fn shell_pid(procs: &std::collections::HashMap<u32, platform::ProcInfo>) -> Option<u32> {
        let parent = procs.get(&std::process::id())?.parent;
        let exe = procs.get(&parent)?.exe.to_ascii_lowercase();
        (exe == "encore.exe").then_some(parent)
    }

    fn own_processes() -> HashSet<u32> {
        // Encore's own processes are never recorded as apps: the app and everything it started
        // (its window's WebView2 processes, which play Encore's sounds; this engine; FFmpeg).
        let me = std::process::id();
        let procs = platform::processes();
        let top = shell_pid(&procs).unwrap_or(me);
        let mut own: HashSet<u32> = HashSet::from([me, top]);
        // Add children until nothing new turns up (process trees are shallow).
        loop {
            let before = own.len();
            for (pid, p) in &procs {
                if own.contains(&p.parent) && p.parent != *pid {
                    own.insert(*pid);
                }
            }
            if own.len() == before {
                break;
            }
        }
        own
    }

    pub fn list_apps() -> Vec<AudioApp> {
        let _ = initialize_mta();
        active_apps(&own_processes()).unwrap_or_else(|err| {
            log(&format!("listing audio apps failed: {err}"));
            Vec::new()
        })
    }

    pub fn list_inputs() -> Vec<InputDevice> {
        let _ = initialize_mta();
        let mut out = Vec::new();
        let Ok(en) = DeviceEnumerator::new() else { return out };
        let default_id = en.get_default_device(&Direction::Capture).ok().and_then(|d| d.get_id().ok());
        if let Ok(coll) = en.get_device_collection(&Direction::Capture) {
            for dev in &coll {
                let Ok(dev) = dev else { continue };
                let (Ok(id), Ok(name)) = (dev.get_id(), dev.get_friendlyname()) else { continue };
                out.push(InputDevice { default: Some(&id) == default_id.as_ref(), id, name });
            }
        }
        out
    }

    pub fn start(dir: &Path, origin_hns: u64, audio_offset_ms: i32, opts: &AudioOptions) -> AudioSession {
        let manifest = Arc::new(Mutex::new(Manifest {
            origin_hns,
            video_start_hns: None,
            audio_offset_ms,
            tracks: Vec::new(),
            parts: Vec::new(),
        }));
        manifest.lock().unwrap().save(dir);
        let stop = Arc::new(AtomicBool::new(false));
        let stop_at = Arc::new(AtomicU64::new(0));
        let threads: Arc<Mutex<Vec<JoinHandle<()>>>> = Arc::new(Mutex::new(Vec::new()));

        if opts.full_mix {
            let (file, title) = add_track(dir, &manifest, "Desktop audio".into(), TrackKind::FullMix);
            spawn(&threads, Source::FullMix, file, title, origin_hns, &stop, &stop_at, &opts.roll_keep_from);
        }
        if opts.mic {
            let (file, title) = add_track(dir, &manifest, "Microphone".into(), TrackKind::Mic);
            spawn(&threads, Source::Mic(opts.mic_device.clone()), file, title, origin_hns, &stop, &stop_at, &opts.roll_keep_from);
        }

        // Watches for apps making sound and gives each its own track (checked every half second).
        let monitor = if opts.app_tracks {
            let (dir, manifest, threads, stop2, stop_at2) =
                (dir.to_path_buf(), manifest.clone(), threads.clone(), stop.clone(), stop_at.clone());
            let max = opts.max_app_tracks;
            let roll = opts.roll_keep_from.clone();
            std::thread::Builder::new()
                .name("audio-apps".into())
                .spawn(move || {
                    let _ = initialize_mta();
                    let own = own_processes();
                    let mut captured: HashSet<u32> = HashSet::new();
                    let mut warned_full = false;
                    while !stop2.load(Ordering::SeqCst) {
                        match active_apps(&own) {
                            Ok(apps) => {
                                for app in apps {
                                    if captured.contains(&app.pid) {
                                        continue;
                                    }
                                    if captured.len() >= max {
                                        if !warned_full {
                                            warned_full = true;
                                            send_event("warning", json!({ "message": format!("More than {max} apps are playing sound. Extra apps are only in the Full mix track.") }));
                                        }
                                        continue;
                                    }
                                    captured.insert(app.pid);
                                    let (file, title) = add_track(&dir, &manifest, app.title.clone(), TrackKind::App);
                                    log(&format!("app track: {title} (pid {}, {})", app.pid, app.exe));
                                    send_event("audio_app_added", json!({ "title": title, "exe": app.exe }));
                                    spawn(&threads, Source::App(app.pid), file, title, origin_hns, &stop2, &stop_at2, &roll);
                                }
                            }
                            Err(err) => log(&format!("audio app scan failed: {err}")),
                        }
                        for _ in 0..5 {
                            if stop2.load(Ordering::SeqCst) {
                                break;
                            }
                            std::thread::sleep(Duration::from_millis(100));
                        }
                    }
                })
                .ok()
        } else {
            None
        };

        AudioSession { dir: dir.to_path_buf(), manifest, stop, stop_at, threads, monitor }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(title: &str) -> TrackInfo {
        TrackInfo { file: "x".into(), title: title.into(), kind: TrackKind::App }
    }

    fn k(title: &str, kind: TrackKind) -> TrackInfo {
        TrackInfo { file: "x".into(), title: title.into(), kind }
    }

    fn titles(p: &[OutTrack]) -> Vec<&str> {
        p.iter().map(|t| t.title.as_str()).collect()
    }

    #[test]
    fn plan_with_everything_on() {
        let tr = vec![k("Desktop audio", TrackKind::FullMix), k("Microphone", TrackKind::Mic), k("Spotify", TrackKind::App), k("Microsoft Edge", TrackKind::App)];
        let p = plan_output(&tr);
        assert_eq!(titles(&p), ["Full mix", "Microphone", "Desktop audio", "Spotify", "Microsoft Edge"]);
        assert_eq!(p[0].sources, vec![0, 1]); // desktop + mic mixed
        assert_eq!(p[2].sources, vec![0]);
    }

    #[test]
    fn plan_without_mic_has_no_duplicate_desktop_track() {
        let tr = vec![k("Desktop audio", TrackKind::FullMix), k("Spotify", TrackKind::App)];
        let p = plan_output(&tr);
        assert_eq!(titles(&p), ["Full mix", "Spotify"]);
        assert_eq!(p[0].sources, vec![0]);
    }

    #[test]
    fn plan_without_desktop_mixes_apps_and_mic() {
        let tr = vec![k("Microphone", TrackKind::Mic), k("Spotify", TrackKind::App), k("Game", TrackKind::App)];
        let p = plan_output(&tr);
        assert_eq!(titles(&p), ["Full mix", "Microphone", "Spotify", "Game"]);
        assert_eq!(p[0].sources, vec![1, 2, 0]);
    }

    #[test]
    fn plan_edge_cases() {
        assert!(plan_output(&[]).is_empty());
        let mic_only = plan_output(&[k("Microphone", TrackKind::Mic)]);
        assert_eq!(titles(&mic_only), ["Full mix"]); // no separate mic copy
        let one_app = plan_output(&[k("Spotify", TrackKind::App)]);
        assert_eq!(titles(&one_app), ["Full mix"]);
        // older recordings named the desktop track "Full mix": plan by kind, not name
        let old = plan_output(&[k("Full mix", TrackKind::FullMix), k("Microphone", TrackKind::Mic)]);
        assert_eq!(titles(&old), ["Full mix", "Microphone", "Desktop audio"]);
    }

    #[test]
    fn unique_titles() {
        let tracks = vec![t("Full mix"), t("Microphone"), t("Discord")];
        assert_eq!(unique_title(&tracks, "Spotify"), "Spotify");
        assert_eq!(unique_title(&tracks, "discord"), "discord 2");
        assert_eq!(unique_title(&tracks, "Microphone"), "Microphone 2");
        assert_eq!(unique_title(&[], "Full mix"), "Full mix 2");
        assert_eq!(unique_title(&[], "  "), "App");
        assert_eq!(unique_title(&[], "Desktop audio"), "Desktop audio 2");
    }

    #[test]
    fn manifest_round_trip_and_track_files() {
        let dir = std::env::temp_dir().join(format!("encore-man-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let m = Arc::new(Mutex::new(Manifest { origin_hns: 42, ..Default::default() }));
        let (p0, t0) = add_track(&dir, &m, "Desktop audio".into(), TrackKind::FullMix);
        let (p1, _) = add_track(&dir, &m, "Microphone".into(), TrackKind::Mic);
        let (p2, t2) = add_track(&dir, &m, "Microphone".into(), TrackKind::App);
        assert_eq!(t0, "Desktop audio");
        assert_eq!(t2, "Microphone 2");
        assert!(p0.ends_with("a00_full.pcm") && p1.ends_with("a01_mic.pcm") && p2.ends_with("a02_app.pcm"));
        let loaded = Manifest::load(&dir).unwrap();
        assert_eq!(loaded, *m.lock().unwrap());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
