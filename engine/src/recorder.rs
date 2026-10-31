//! One recording session: FFmpeg writing segments to a temp folder, then joined into the final MP4.
//! Segments mean a crash or power cut loses seconds, not the recording.

use crate::audio::{self, AudioOptions, AudioSession, Manifest};
use crate::ffmpeg::{self, AudioInput, CaptureSpec, Encoder, Pipeline};
use crate::log::{log, tail};
use crate::platform::{self, LocalTime};
use crate::timeline::{self, VideoClock};
use std::io::{BufRead, BufReader, Write};
use std::sync::{Arc, Mutex};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// How long FFmpeg must keep running before a start counts as successful.
const START_GRACE: Duration = Duration::from_millis(2500);
/// How long to wait for FFmpeg to finish writing after asking it to stop.
const STOP_TIMEOUT: Duration = Duration::from_secs(20);

pub struct Active {
    child: Child,
    stdin: Option<ChildStdin>,
    audio: Option<AudioSession>,
    manifest: Arc<Mutex<Manifest>>,
    clock: Arc<Mutex<VideoClock>>,
    pub seg_dir: PathBuf,
    pub log_path: PathBuf,
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

/// `<out_dir>/<Label>/<file name>`, adding " (2)", " (3)"... if that file already exists.
pub fn output_path(out_dir: &Path, label: &str, t: LocalTime) -> PathBuf {
    let folder = out_dir.join(sanitise(label));
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
        let log_path = seg_dir.join("ffmpeg.log");
        let log_file = std::fs::File::create(&log_path).map_err(|e| format!("Could not create log: {e}"))?;
        let args = ffmpeg::record_args(spec, encoder, pipeline, kbps, &seg_dir);
        log(&format!("starting capture ({pipeline:?}, {}): ffmpeg {}", encoder.name(), args.join(" ")));
        let spawned = ffmpeg::hidden_command(ffmpeg_path)
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::from(log_file))
            .spawn();
        let mut child = match spawned {
            Ok(c) => c,
            Err(e) => {
                if let Some(a) = audio.take() {
                    a.stop(platform::now_hns());
                }
                let _ = std::fs::remove_dir_all(&seg_dir);
                return Err(format!("Could not start FFmpeg ({}): {e}", ffmpeg_path.display()));
            }
        };
        let clock = Arc::new(Mutex::new(VideoClock::default()));
        if let Some(stdout) = child.stdout.take() {
            watch_progress(stdout, clock.clone(), manifest.clone(), seg_dir.clone());
        }

        let begun = Instant::now();
        let mut exited = None;
        while begun.elapsed() < START_GRACE {
            if let Ok(Some(status)) = child.try_wait() {
                exited = Some(status);
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        match exited {
            None => {
                let stdin = child.stdin.take();
                return Ok(Active {
                    child,
                    stdin,
                    audio,
                    manifest,
                    clock,
                    seg_dir,
                    log_path,
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
                last_error = tail(&log_path, 6);
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

/// Removes video segments and the FFmpeg log from a failed start, keeping audio files.
fn clear_video_files(seg_dir: &Path) {
    for p in segments(seg_dir) {
        let _ = std::fs::remove_file(p);
    }
    let _ = std::fs::remove_file(seg_dir.join("ffmpeg.log"));
}

/// Reads FFmpeg's progress reports to pin down when the video timeline started (see VideoClock),
/// and records it in the manifest so a crashed recording can still be lined up.
fn watch_progress(
    stdout: std::process::ChildStdout,
    clock: Arc<Mutex<VideoClock>>,
    manifest: Arc<Mutex<Manifest>>,
    dir: PathBuf,
) {
    std::thread::spawn(move || {
        let mut last_saved: Option<u64> = None;
        let mut last_save_time = Instant::now();
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            let now = platform::now_hns();
            if let Some(us) = timeline::parse_out_time_us(&line) {
                let best = {
                    let mut c = clock.lock().unwrap();
                    c.observe(now, us);
                    c.start_hns()
                };
                if best != last_saved && last_save_time.elapsed() > Duration::from_secs(2) {
                    let mut m = manifest.lock().unwrap();
                    m.video_start_hns = best;
                    m.save(&dir);
                    last_saved = best;
                    last_save_time = Instant::now();
                }
            }
        }
    });
}

impl Active {
    pub fn track_titles(&self) -> Vec<String> {
        self.audio.as_ref().map(|a| a.track_titles()).unwrap_or_default()
    }

    pub fn elapsed_ms(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }

    /// Non-blocking check: true if FFmpeg has stopped on its own (crash, window closed, etc.).
    pub fn has_exited(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(Some(_)))
    }

    /// Asks FFmpeg to finish cleanly, waits, then joins the segments into the final file.
    pub fn stop_and_save(mut self, ffmpeg_path: &Path) -> Result<Saved, String> {
        let duration_ms = self.elapsed_ms();
        let end_hns = platform::now_hns();
        if let Some(mut stdin) = self.stdin.take() {
            let _ = stdin.write_all(b"q\n");
            let _ = stdin.flush();
        }
        let begun = Instant::now();
        loop {
            match self.child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) if begun.elapsed() < STOP_TIMEOUT => std::thread::sleep(Duration::from_millis(100)),
                _ => {
                    log("FFmpeg did not stop in time; ending it. The last segment may be cut short.");
                    let _ = self.child.kill();
                    let _ = self.child.wait();
                    break;
                }
            }
        }
        let mut manifest = match self.audio.take() {
            Some(a) => a.stop(end_hns),
            None => self.manifest.lock().unwrap().clone(),
        };
        let clock = *self.clock.lock().unwrap();
        manifest.video_start_hns = clock.start_hns().or(manifest.video_start_hns);
        manifest.save(&self.seg_dir);
        log(&format!(
            "sync: video starts {} ms after origin ({} progress reports)",
            manifest.video_start_hns.map(|v| (v as i64 - manifest.origin_hns as i64) / 10_000).unwrap_or(-1),
            clock.samples()
        ));
        finalise(ffmpeg_path, &self.seg_dir, &self.out_path, duration_ms)
    }
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
    std::fs::write(&list, ffmpeg::concat_list(&files)).map_err(|e| format!("Could not write segment list: {e}"))?;
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

/// The audio tracks recorded in `seg_dir` (from its manifest) and how much to trim from their start.
fn audio_inputs(seg_dir: &Path) -> (Vec<AudioInput>, f64) {
    let Some(m) = Manifest::load(seg_dir) else { return (Vec::new(), 0.0) };
    let tracks: Vec<AudioInput> = m
        .tracks
        .iter()
        .map(|t| AudioInput { path: seg_dir.join(&t.file), title: t.title.clone() })
        .filter(|t| std::fs::metadata(&t.path).map(|md| md.len() > 0).unwrap_or(false))
        .collect();
    let skip = timeline::audio_skip_seconds(m.origin_hns, m.video_start_hns, m.audio_offset_ms);
    (tracks, skip)
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
        assert_eq!(first, dir.join("Game").join("Game 2026.10.31 - 09.05.07.mp4"));
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
