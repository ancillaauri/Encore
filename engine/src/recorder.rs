//! One recording session: FFmpeg writing segments to a temp folder, then joined into the final MP4.
//! Segments mean a crash or power cut loses seconds, not the recording.

use crate::audio::{self, AudioOptions, AudioSession, Manifest, VideoPart};
use crate::ffmpeg::{self, AudioOut, CaptureSpec, Encoder, PartFiles, Pipeline, Target};
use crate::log::{log, tail};
use crate::platform::{self, LocalTime};
use crate::timeline::{self, VideoClock};
use std::io::{BufRead, BufReader, Write};
use std::sync::{Arc, Mutex};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Stdio};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// How long FFmpeg must keep running before a start counts as successful.
const START_GRACE: Duration = Duration::from_millis(2500);
/// How long to wait for FFmpeg to finish writing after asking it to stop.
const STOP_TIMEOUT: Duration = Duration::from_secs(20);

/// One FFmpeg run: a part of the recording (a new one starts each time the monitor changes).
struct Proc {
    child: Child,
    stdin: Option<ChildStdin>,
    clock: Arc<Mutex<VideoClock>>,
    index: usize,
    log_path: PathBuf,
    target: Target,
}

pub struct Active {
    proc: Proc,
    /// A part on another monitor that is starting up; it takes over once FFmpeg reports progress.
    pending: Option<Proc>,
    /// Parts that were taken over and are finishing their last segment.
    retiring: Vec<JoinHandle<()>>,
    /// Every part's clock, by index (to record when each one started).
    clocks: Vec<Arc<Mutex<VideoClock>>>,
    spec: CaptureSpec,
    kbps: u32,
    audio: Option<AudioSession>,
    manifest: Arc<Mutex<Manifest>>,
    pub seg_dir: PathBuf,
    pub started: Instant,
    pub started_unix_ms: u64,
    pub label: String,
    pub out_path: PathBuf,
    pub encoder: Encoder,
    pub pipeline: Pipeline,
    pub width: u32,
    pub height: u32,
}

pub struct Saved {
    pub path: PathBuf,
    pub duration_ms: u64,
    pub size_bytes: u64,
}

pub fn unix_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Makes a name safe for a Windows file or folder name.
pub fn sanitise(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if c.is_control() || "<>:\"/\\|?*".contains(c) { ' ' } else { c })
        .collect();
    let collapsed = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut s: String = collapsed.chars().take(60).collect();
    s = s.trim_matches(|c: char| c == '.' || c.is_whitespace()).to_string();
    const RESERVED: [&str; 22] = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9",
        "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    if s.is_empty() || RESERVED.contains(&s.to_ascii_uppercase().as_str()) {
        "Recording".into()
    } else {
        s
    }
}

/// "<Label> YYYY.MM.DD - HH.MM.SS.mp4"
pub fn file_name(label: &str, t: LocalTime) -> String {
    format!(
        "{} {:04}.{:02}.{:02} - {:02}.{:02}.{:02}.mp4",
        sanitise(label),
        t.year,
        t.month,
        t.day,
        t.hour,
        t.minute,
        t.second
    )
}

/// `<chosen folder>/Encore/<Label>/<file name>`, adding " (2)", " (3)"... if that file already
/// exists.
pub fn output_path(out_dir: &Path, label: &str, t: LocalTime) -> PathBuf {
    let folder = crate::paths::encore_root(out_dir).join(sanitise(label));
    let first = folder.join(file_name(label, t));
    if !first.exists() {
        return first;
    }
    let stem = first.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    (2..1000)
        .map(|n| folder.join(format!("{stem} ({n}).mp4")))
        .find(|p| !p.exists())
        .unwrap_or(first)
}

fn segments(seg_dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(seg_dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| {
                    p.extension().map(|x| x == "mkv").unwrap_or(false)
                        && std::fs::metadata(p).map(|m| m.len() > 0).unwrap_or(false)
                })
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    files
}

fn part_prefix(index: usize) -> String {
    if index == 0 { "seg_".into() } else { format!("p{index:03}_") }
}

fn part_log(seg_dir: &Path, index: usize) -> PathBuf {
    if index == 0 { seg_dir.join("ffmpeg.log") } else { seg_dir.join(format!("ffmpeg_p{index:03}.log")) }
}

/// Starts one FFmpeg run writing segments named for part `index`.
#[allow(clippy::too_many_arguments)]
fn spawn_part(
    ffmpeg_path: &Path,
    spec: &CaptureSpec,
    encoder: Encoder,
    pipeline: Pipeline,
    kbps: u32,
    seg_dir: &Path,
    index: usize,
    manifest: &Arc<Mutex<Manifest>>,
) -> Result<Proc, String> {
    let log_path = part_log(seg_dir, index);
    let log_file = std::fs::File::create(&log_path).map_err(|e| format!("Could not create log: {e}"))?;
    let args = ffmpeg::record_args(spec, encoder, pipeline, kbps, seg_dir, &part_prefix(index));
    log(&format!("starting capture part {index} ({pipeline:?}, {}): ffmpeg {}", encoder.name(), args.join(" ")));
    let mut child = ffmpeg::hidden_command(ffmpeg_path)
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::from(log_file))
        .spawn()
        .map_err(|e| format!("Could not start FFmpeg ({}): {e}", ffmpeg_path.display()))?;
    log(&format!("capture part {index}: {}", platform::boost_capture_priority(child.id())));
    let clock = Arc::new(Mutex::new(VideoClock::default()));
    if let Some(stdout) = child.stdout.take() {
        watch_progress(stdout, clock.clone(), manifest.clone(), seg_dir.to_path_buf(), index);
    }
    let stdin = child.stdin.take();
    Ok(Proc { child, stdin, clock, index, log_path, target: spec.target })
}

/// Asks an FFmpeg run to finish its current segment and exit; ends it if it hangs.
fn finish_proc(mut p: Proc) {
    if let Some(mut stdin) = p.stdin.take() {
        let _ = stdin.write_all(b"q\n");
        let _ = stdin.flush();
    }
    let begun = Instant::now();
    loop {
        match p.child.try_wait() {
            Ok(Some(_)) => return,
            Ok(None) if begun.elapsed() < STOP_TIMEOUT => std::thread::sleep(Duration::from_millis(100)),
            _ => {
                log(&format!("FFmpeg part {} did not stop in time; ending it. Its last segment may be cut short.", p.index));
                let _ = p.child.kill();
                let _ = p.child.wait();
                return;
            }
        }
    }
}

fn set_part(manifest: &Arc<Mutex<Manifest>>, index: usize, part: VideoPart) {
    let mut m = manifest.lock().unwrap();
    if m.parts.len() <= index {
        m.parts.resize(index + 1, VideoPart::default());
    }
    m.parts[index] = part;
}

/// Starts FFmpeg. Tries zero-copy first and falls back to copying frames through memory if this
/// PC's driver rejects zero-copy. Returns an error with FFmpeg's own message if both fail.
#[allow(clippy::too_many_arguments)]
pub fn start(
    ffmpeg_path: &Path,
    spec: &CaptureSpec,
    encoder: Encoder,
    kbps: u32,
    label: &str,
    out_path: PathBuf,
    temp_root: &Path,
    preferred: Option<Pipeline>,
    audio_opts: &AudioOptions,
    audio_offset_ms: i32,
) -> Result<Active, String> {
    let session = format!("rec-{}", unix_ms());
    let seg_dir = temp_root.join(&session);
    let order: Vec<Pipeline> = match preferred {
        Some(Pipeline::Download) => vec![Pipeline::Download],
        _ => vec![Pipeline::ZeroCopy, Pipeline::Download],
    };
    std::fs::create_dir_all(&seg_dir).map_err(|e| format!("Could not create temp folder: {e}"))?;
    // Audio starts first so it covers the very first video frame; both share this origin.
    let origin = platform::now_hns();
    let audio = audio::start(&seg_dir, origin, audio_offset_ms, audio_opts);
    let manifest = audio.manifest();
    let mut audio = Some(audio);
    let mut last_error = String::new();
    for pipeline in order {
        clear_video_files(&seg_dir);
        set_part(&manifest, 0, VideoPart { prefix: part_prefix(0), start_hns: None, dropped: false });
        let mut proc = match spawn_part(ffmpeg_path, spec, encoder, pipeline, kbps, &seg_dir, 0, &manifest) {
            Ok(p) => p,
            Err(e) => {
                if let Some(a) = audio.take() {
                    a.stop(platform::now_hns());
                }
                let _ = std::fs::remove_dir_all(&seg_dir);
                return Err(e);
            }
        };
        let begun = Instant::now();
        let mut exited = None;
        while begun.elapsed() < START_GRACE {
            if let Ok(Some(status)) = proc.child.try_wait() {
                exited = Some(status);
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        match exited {
            None => {
                let clocks = vec![proc.clock.clone()];
                return Ok(Active {
                    proc,
                    pending: None,
                    retiring: Vec::new(),
                    clocks,
                    spec: *spec,
                    kbps,
                    audio,
                    manifest,
                    seg_dir,
                    started: begun,
                    started_unix_ms: unix_ms(),
                    label: label.to_string(),
                    out_path,
                    encoder,
                    pipeline,
                    width: spec.width,
                    height: spec.height,
                });
            }
            Some(status) => {
                last_error = tail(&proc.log_path, 6);
                log(&format!("capture failed to start ({pipeline:?}, exit {status}): {last_error}"));
            }
        }
    }
    if let Some(a) = audio.take() {
        a.stop(platform::now_hns());
    }
    let _ = std::fs::remove_dir_all(&seg_dir);
    Err(if last_error.is_empty() {
        "FFmpeg stopped straight away without an error message.".into()
    } else {
        format!("Recording could not start. FFmpeg said:\n{last_error}")
    })
}

/// Removes video segments and FFmpeg logs from a failed start, keeping audio files.
fn clear_video_files(seg_dir: &Path) {
    for p in segments(seg_dir) {
        let _ = std::fs::remove_file(p);
    }
    let _ = std::fs::remove_file(seg_dir.join("ffmpeg.log"));
}

/// Reads FFmpeg's progress reports to pin down when this part's video started (see VideoClock),
/// and records it in the manifest so a crashed recording can still be lined up.
fn watch_progress(
    stdout: std::process::ChildStdout,
    clock: Arc<Mutex<VideoClock>>,
    manifest: Arc<Mutex<Manifest>>,
    dir: PathBuf,
    index: usize,
) {
    std::thread::spawn(move || {
        let mut last_saved: Option<u64> = None;
        let mut last_save_time: Option<Instant> = None;
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            let now = platform::now_hns();
            if let Some(us) = timeline::parse_out_time_us(&line) {
                let best = {
                    let mut c = clock.lock().unwrap();
                    c.observe(now, us);
                    c.start_hns()
                };
                let due = last_save_time.map(|t| t.elapsed() > Duration::from_secs(2)).unwrap_or(true);
                if best.is_some() && best != last_saved && due {
                    let mut m = manifest.lock().unwrap();
                    if index == 0 {
                        m.video_start_hns = best;
                    }
                    if let Some(p) = m.parts.get_mut(index) {
                        p.start_hns = best;
                    }
                    m.save(&dir);
                    last_saved = best;
                    last_save_time = Some(Instant::now());
                }
            }
        }
    });
}

/// What happened to a monitor switch, for the engine to report.
pub enum SwitchEvent {
    /// The new monitor's capture took over.
    Switched(Target),
    /// The new monitor's capture failed to start; recording carries on where it was.
    Failed(String),
}

impl Active {
    pub fn track_titles(&self) -> Vec<String> {
        self.audio.as_ref().map(|a| a.track_titles()).unwrap_or_default()
    }

    pub fn elapsed_ms(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }

    pub fn log_path(&self) -> PathBuf {
        self.proc.log_path.clone()
    }

    pub fn fps(&self) -> u32 {
        self.spec.fps
    }

    /// What is being recorded right now (the monitor or window).
    pub fn target(&self) -> Target {
        self.proc.target
    }

    /// The monitor a switch is under way to, if any.
    pub fn switching_to(&self) -> Option<Target> {
        self.pending.as_ref().map(|p| p.target)
    }

    /// Non-blocking check: true if FFmpeg has stopped on its own (crash, window closed, etc.).
    pub fn has_exited(&mut self) -> bool {
        matches!(self.proc.child.try_wait(), Ok(Some(_)))
    }

    /// Starts recording `target` alongside the current capture. It takes over (see `poll`) once
    /// it is producing video, so there is no gap; the join cuts the old part at that moment.
    pub fn switch_to(&mut self, ffmpeg_path: &Path, target: Target) {
        if self.pending.is_some() || self.proc.target == target {
            return;
        }
        let index = self.clocks.len();
        let spec = CaptureSpec { target, ..self.spec };
        set_part(&self.manifest, index, VideoPart { prefix: part_prefix(index), start_hns: None, dropped: false });
        match spawn_part(ffmpeg_path, &spec, self.encoder, self.pipeline, self.kbps, &self.seg_dir, index, &self.manifest) {
            Ok(p) => {
                self.clocks.push(p.clock.clone());
                self.pending = Some(p);
            }
            Err(e) => {
                log(&format!("switch failed to start: {e}"));
                self.clocks.push(Arc::new(Mutex::new(VideoClock::default())));
                self.drop_part(index);
            }
        }
    }

    fn drop_part(&self, index: usize) {
        let mut m = self.manifest.lock().unwrap();
        if let Some(p) = m.parts.get_mut(index) {
            p.dropped = true;
        }
        m.save(&self.seg_dir);
    }

    /// Moves a switch along. Called regularly by the engine.
    pub fn poll(&mut self) -> Option<SwitchEvent> {
        let pending = self.pending.as_mut()?;
        if let Ok(Some(status)) = pending.child.try_wait() {
            let p = self.pending.take().unwrap();
            let detail = tail(&p.log_path, 4);
            log(&format!("switch part {} exited ({status}): {detail}", p.index));
            self.drop_part(p.index);
            return Some(SwitchEvent::Failed(detail));
        }
        // Half a second of progress reports: video is flowing and its start time is pinned down.
        if pending.clock.lock().unwrap().samples() < 10 {
            return None;
        }
        let new = self.pending.take().unwrap();
        let old = std::mem::replace(&mut self.proc, new);
        log(&format!("switched to part {} ({:?})", self.proc.index, self.proc.target));
        self.retiring.push(std::thread::spawn(move || finish_proc(old)));
        Some(SwitchEvent::Switched(self.proc.target))
    }

    /// Asks FFmpeg to finish cleanly, waits, then joins the segments into the final file.
    pub fn stop_and_save(mut self, ffmpeg_path: &Path) -> Result<Saved, String> {
        let duration_ms = self.elapsed_ms();
        let end_hns = platform::now_hns();
        if let Some(p) = self.pending.take() {
            let index = p.index;
            finish_proc(p);
            self.drop_part(index);
        }
        let current = self.proc;
        let last = std::thread::spawn(move || finish_proc(current));
        for t in self.retiring.drain(..).chain(std::iter::once(last)) {
            let _ = t.join();
        }
        let mut manifest = match self.audio.take() {
            Some(a) => a.stop(end_hns),
            None => self.manifest.lock().unwrap().clone(),
        };
        // Final start times from each part's clock.
        let parts_now = self.manifest.lock().unwrap().parts.clone();
        manifest.parts = parts_now;
        for (i, c) in self.clocks.iter().enumerate() {
            let c = *c.lock().unwrap();
            if let Some(p) = manifest.parts.get_mut(i) {
                p.start_hns = c.start_hns().or(p.start_hns);
            }
            if i == 0 {
                manifest.video_start_hns = c.start_hns().or(manifest.video_start_hns);
                log(&format!(
                    "sync: video starts {} ms after origin ({} progress reports)",
                    manifest.video_start_hns.map(|v| (v as i64 - manifest.origin_hns as i64) / 10_000).unwrap_or(-1),
                    c.samples()
                ));
            }
        }
        manifest.save(&self.seg_dir);
        finalise(ffmpeg_path, &self.seg_dir, &self.out_path, duration_ms)
    }
}

/// Segment files of one part, in order.
fn part_segments(seg_dir: &Path, prefix: &str) -> Vec<PathBuf> {
    segments(seg_dir)
        .into_iter()
        .filter(|p| p.file_name().map(|n| n.to_string_lossy().starts_with(prefix)).unwrap_or(false))
        .collect()
}

/// What to join, in order: each part's segments and where to cut it (where the next one took
/// over). Sessions from before 0.5 have no parts listed: all their segments, uncut.
fn join_plan(seg_dir: &Path, manifest: Option<&Manifest>) -> Vec<PartFiles> {
    let parts: Vec<&VideoPart> = manifest.map(|m| m.parts.iter().filter(|p| !p.dropped).collect()).unwrap_or_default();
    if parts.is_empty() {
        return vec![PartFiles { files: segments(seg_dir), keep_secs: None }];
    }
    let mut out = Vec::new();
    for (i, p) in parts.iter().enumerate() {
        let files = part_segments(seg_dir, &p.prefix);
        // A later part that never reported a start can't be placed in time: leave it out.
        if files.is_empty() || (i > 0 && p.start_hns.is_none()) {
            continue;
        }
        let next = parts[i + 1..].iter().find(|n| n.start_hns.is_some() && !part_segments(seg_dir, &n.prefix).is_empty());
        let keep_secs = match (p.start_hns, next.and_then(|n| n.start_hns)) {
            (Some(a), Some(b)) if b > a => Some((b - a) as f64 / 1e7),
            _ => None,
        };
        out.push(PartFiles { files, keep_secs });
    }
    out
}

/// Joins every finished segment in `seg_dir` into `out_path`. Removes the temp folder only on success.
pub fn finalise(ffmpeg_path: &Path, seg_dir: &Path, out_path: &Path, duration_ms: u64) -> Result<Saved, String> {
    let files = segments(seg_dir);
    if files.is_empty() {
        let detail = tail(&seg_dir.join("ffmpeg.log"), 6);
        return Err(format!("No video was captured.{}", if detail.is_empty() { String::new() } else { format!(" FFmpeg said:\n{detail}") }));
    }
    if let Some(dir) = out_path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("Could not create output folder {}: {e}", dir.display()))?;
    }
    let list = seg_dir.join("segments.ffconcat");
    let plan = join_plan(seg_dir, Manifest::load(seg_dir).as_ref());
    std::fs::write(&list, ffmpeg::concat_list_parts(&plan)).map_err(|e| format!("Could not write segment list: {e}"))?;
    let (audio, skip) = audio_inputs(seg_dir);
    let out = ffmpeg::hidden_command(ffmpeg_path)
        .args(ffmpeg::finalise_args(&list, &audio, skip, out_path))
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("Could not run FFmpeg to save the file: {e}"))?;
    let size = std::fs::metadata(out_path).map(|m| m.len()).unwrap_or(0);
    if !out.status.success() || size == 0 {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(format!(
            "Saving failed; the raw segments are kept in {}. FFmpeg said:\n{}",
            seg_dir.display(),
            err.trim()
        ));
    }
    let _ = std::fs::remove_dir_all(seg_dir);
    log(&format!("saved {} ({} bytes)", out_path.display(), size));
    Ok(Saved { path: out_path.to_path_buf(), duration_ms, size_bytes: size })
}

/// The audio tracks for the saved file (planned from the session's manifest) and how much to trim
/// from the start of the recorded audio.
fn audio_inputs(seg_dir: &Path) -> (Vec<AudioOut>, f64) {
    let Some(m) = Manifest::load(seg_dir) else { return (Vec::new(), 0.0) };
    let present = |i: usize| std::fs::metadata(seg_dir.join(&m.tracks[i].file)).map(|md| md.len() > 0).unwrap_or(false);
    // Only plan with sources whose files actually have audio in them.
    let kept: Vec<usize> = (0..m.tracks.len()).filter(|&i| present(i)).collect();
    let tracks: Vec<audio::TrackInfo> = kept.iter().map(|&i| m.tracks[i].clone()).collect();
    let out = audio::plan_output(&tracks)
        .into_iter()
        .map(|t| AudioOut { title: t.title, files: t.sources.iter().map(|&i| seg_dir.join(&tracks[i].file)).collect() })
        .collect();
    let skip = timeline::audio_skip_seconds(m.origin_hns, m.video_start_hns, m.audio_offset_ms);
    (out, skip)
}

/// Recordings left behind by a crash (temp folders with segments). Used to recover on start-up.
pub fn leftover_sessions(temp_root: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(temp_root)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.is_dir() && !segments(p).is_empty())
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t() -> LocalTime {
        LocalTime { year: 2026, month: 10, day: 31, hour: 9, minute: 5, second: 7 }
    }

    #[test]
    fn sanitise_cleans_names() {
        assert_eq!(sanitise("VALORANT  "), "VALORANT");
        assert_eq!(sanitise("a<b>c:d\"e/f\\g|h?i*j"), "a b c d e f g h i j");
        assert_eq!(sanitise("..."), "Recording");
        assert_eq!(sanitise("con"), "Recording");
        assert_eq!(sanitise(""), "Recording");
        assert_eq!(sanitise(&"x".repeat(100)).len(), 60);
        assert_eq!(sanitise("Line\nbreak\ttab"), "Line break tab");
    }

    #[test]
    fn file_name_format() {
        assert_eq!(file_name("Desktop", t()), "Desktop 2026.10.31 - 09.05.07.mp4");
    }

    #[test]
    fn output_path_avoids_overwriting() {
        let dir = std::env::temp_dir().join(format!("encore-test-{}", unix_ms()));
        let first = output_path(&dir, "Game", t());
        assert_eq!(first, dir.join("Encore").join("Game").join("Game 2026.10.31 - 09.05.07.mp4"));
        std::fs::create_dir_all(first.parent().unwrap()).unwrap();
        std::fs::write(&first, b"x").unwrap();
        let second = output_path(&dir, "Game", t());
        assert!(second.to_string_lossy().ends_with("(2).mp4"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn segments_are_sorted_and_skip_empty_files() {
        let dir = std::env::temp_dir().join(format!("encore-seg-{}", unix_ms()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("seg_000002.mkv"), b"b").unwrap();
        std::fs::write(dir.join("seg_000001.mkv"), b"a").unwrap();
        std::fs::write(dir.join("seg_000003.mkv"), b"").unwrap();
        std::fs::write(dir.join("ffmpeg.log"), b"log").unwrap();
        let s = segments(&dir);
        assert_eq!(s.len(), 2);
        assert!(s[0].ends_with("seg_000001.mkv"));
        assert_eq!(leftover_sessions(dir.parent().unwrap()).iter().filter(|p| **p == dir).count(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn finalise_with_no_segments_reports_clearly() {
        let dir = std::env::temp_dir().join(format!("encore-empty-{}", unix_ms()));
        std::fs::create_dir_all(&dir).unwrap();
        let err = finalise(Path::new("ffmpeg"), &dir, &dir.join("out.mp4"), 0).err().unwrap();
        assert!(err.starts_with("No video was captured."));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod e2e {
    use super::*;
    use crate::audio::{Manifest, TrackInfo, TrackKind};

    /// Needs FFmpeg on PATH; run with `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn full_mix_contains_mic_and_stays_in_sync() {
        let dir = std::env::temp_dir().join(format!("encore-e2e-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let run = |args: &[&str]| {
            let st = std::process::Command::new("ffmpeg").args(["-hide_banner", "-loglevel", "error", "-y"]).args(args).status().unwrap();
            assert!(st.success());
        };
        // 4 s of black with a white flash at 2.000 s, in 2 s segments.
        run(&["-f", "lavfi", "-i", "color=black:s=320x180:r=60:d=4", "-vf", "drawbox=c=white:t=fill:enable='between(t,2,2.1)'",
            "-c:v", "libx264", "-bf", "0", "-g", "120", "-force_key_frames", "expr:gte(t,n_forced*2)",
            "-f", "segment", "-segment_time", "2", "-segment_format", "matroska", "-reset_timestamps", "1",
            dir.join("seg_%06d.mkv").to_str().unwrap()]);
        // Audio starts 0.35 s before video. Desktop: quiet 220 Hz hum. Mic: silence with a beep at 2.35 s.
        run(&["-f", "lavfi", "-i", "sine=f=220:d=4.35:r=48000,volume=0.1", "-ac", "2", "-f", "s16le", dir.join("a00_full.pcm").to_str().unwrap()]);
        run(&["-f", "lavfi", "-i", "aevalsrc='if(between(t,2.35,2.45),0.8*sin(2*PI*1000*t),0)':d=4.35:s=48000", "-ac", "2", "-f", "s16le", dir.join("a01_mic.pcm").to_str().unwrap()]);
        let m = Manifest {
            origin_hns: 10_000_000,
            video_start_hns: Some(13_500_000),
            audio_offset_ms: 0,
            tracks: vec![
                TrackInfo { file: "a00_full.pcm".into(), title: "Desktop audio".into(), kind: TrackKind::FullMix },
                TrackInfo { file: "a01_mic.pcm".into(), title: "Microphone".into(), kind: TrackKind::Mic },
            ],
            parts: Vec::new(),
        };
        m.save(&dir);
        let out = std::env::temp_dir().join(format!("encore-e2e-{}.mp4", std::process::id()));
        finalise(Path::new("ffmpeg"), &dir, &out, 4000).unwrap();
        println!("OUT {}", out.display());
    }
}

#[cfg(test)]
mod e2e_switch {
    use super::*;
    use crate::audio::{Manifest, TrackInfo, TrackKind, VideoPart};

    /// Two parts: the first (blue) runs 6 s, the second (green) starts 3.5 s in and runs 4 s.
    /// The saved file must cut over at 3.5 s, keep flashes and the mic beep where they happened,
    /// and drop the first part's flash at 4.0 s (after the cut). Needs FFmpeg on PATH.
    #[test]
    #[ignore]
    fn switch_cuts_at_takeover_and_stays_in_sync() {
        let dir = std::env::temp_dir().join(format!("encore-e2e-sw-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let run = |args: &[&str]| {
            let st = std::process::Command::new("ffmpeg").args(["-hide_banner", "-loglevel", "error", "-y"]).args(args).status().unwrap();
            assert!(st.success());
        };
        let seg = |color: &str, d: &str, flashes: &str, pattern: &str| {
            run(&["-f", "lavfi", "-i", &format!("color={color}:s=320x180:r=60:d={d}"),
                "-vf", &format!("drawbox=c=white:t=fill:enable='{flashes}'"),
                "-c:v", "libx264", "-bf", "0", "-g", "120", "-force_key_frames", "expr:gte(t,n_forced*2)",
                "-f", "segment", "-segment_time", "2", "-segment_format", "matroska", "-reset_timestamps", "1",
                dir.join(pattern).to_str().unwrap()]);
        };
        seg("blue", "6", "between(t,2,2.1)+between(t,4,4.1)", "seg_%06d.mkv");
        seg("green", "4", "between(t,1,1.1)", "p001_%06d.mkv");
        // Audio starts 0.35 s before the first part. Mic beep at 0.35 + 4.5 s.
        run(&["-f", "lavfi", "-i", "aevalsrc='if(between(t,4.85,4.95),0.8*sin(2*PI*1000*t),0)':d=7.85:s=48000", "-ac", "2", "-f", "s16le", dir.join("a01_mic.pcm").to_str().unwrap()]);
        let m = Manifest {
            origin_hns: 10_000_000,
            video_start_hns: Some(13_500_000),
            audio_offset_ms: 0,
            tracks: vec![TrackInfo { file: "a01_mic.pcm".into(), title: "Microphone".into(), kind: TrackKind::Mic }],
            parts: vec![
                VideoPart { prefix: "seg_".into(), start_hns: Some(13_500_000), dropped: false },
                VideoPart { prefix: "p001_".into(), start_hns: Some(48_500_000), dropped: false },
                // A switch that never took over: must be ignored.
                VideoPart { prefix: "p002_".into(), start_hns: None, dropped: true },
            ],
        };
        m.save(&dir);
        let out = std::env::temp_dir().join(format!("encore-e2e-sw-{}.mp4", std::process::id()));
        finalise(Path::new("ffmpeg"), &dir, &out, 7500).unwrap();
        println!("OUT {}", out.display());
    }
}
