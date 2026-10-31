//! One recording session: FFmpeg writing segments to a temp folder, then joined into the final MP4.
//! Segments mean a crash or power cut loses seconds, not the recording.

use crate::ffmpeg::{self, CaptureSpec, Encoder, Pipeline};
use crate::log::{log, tail};
use crate::platform::LocalTime;
use std::io::Write;
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
) -> Result<Active, String> {
    let session = format!("rec-{}", unix_ms());
    let seg_dir = temp_root.join(&session);
    let order: Vec<Pipeline> = match preferred {
        Some(Pipeline::Download) => vec![Pipeline::Download],
        _ => vec![Pipeline::ZeroCopy, Pipeline::Download],
    };
    let mut last_error = String::new();
    for pipeline in order {
        let _ = std::fs::remove_dir_all(&seg_dir);
        std::fs::create_dir_all(&seg_dir).map_err(|e| format!("Could not create temp folder: {e}"))?;
        let log_path = seg_dir.join("ffmpeg.log");
        let log_file = std::fs::File::create(&log_path).map_err(|e| format!("Could not create log: {e}"))?;
        let args = ffmpeg::record_args(spec, encoder, pipeline, kbps, &seg_dir);
        log(&format!("starting capture ({pipeline:?}, {}): ffmpeg {}", encoder.name(), args.join(" ")));
        let mut child = ffmpeg::hidden_command(ffmpeg_path)
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::from(log_file))
            .spawn()
            .map_err(|e| format!("Could not start FFmpeg ({}): {e}", ffmpeg_path.display()))?;

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
    let _ = std::fs::remove_dir_all(&seg_dir);
    Err(if last_error.is_empty() {
        "FFmpeg stopped straight away without an error message.".into()
    } else {
        format!("Recording could not start. FFmpeg said:\n{last_error}")
    })
}

impl Active {
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
    let out = ffmpeg::hidden_command(ffmpeg_path)
        .args(ffmpeg::finalise_args(&list, out_path))
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
