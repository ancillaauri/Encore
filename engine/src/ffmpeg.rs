//! Everything that talks to FFmpeg: finding it, probing it, and building command lines.
//! The command-line builders are pure functions so they can be unit-tested anywhere.

use crate::settings::{Codec, Quality, Resolution};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Segment length in seconds. Keyframes are forced on this boundary so segments cut cleanly,
/// and it bounds how much is lost if anything crashes.
pub const SEGMENT_SECONDS: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Vendor {
    Nvidia,
    Amd,
    Intel,
}

pub const VENDOR_ORDER: [Vendor; 3] = [Vendor::Nvidia, Vendor::Amd, Vendor::Intel];
pub const ALL_CODECS: [Codec; 3] = [Codec::H264, Codec::Hevc, Codec::Av1];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Encoder {
    pub codec: Codec,
    pub vendor: Vendor,
}

impl Encoder {
    pub fn name(&self) -> &'static str {
        match (self.codec, self.vendor) {
            (Codec::H264, Vendor::Nvidia) => "h264_nvenc",
            (Codec::H264, Vendor::Amd) => "h264_amf",
            (Codec::H264, Vendor::Intel) => "h264_qsv",
            (Codec::Hevc, Vendor::Nvidia) => "hevc_nvenc",
            (Codec::Hevc, Vendor::Amd) => "hevc_amf",
            (Codec::Hevc, Vendor::Intel) => "hevc_qsv",
            (Codec::Av1, Vendor::Nvidia) => "av1_nvenc",
            (Codec::Av1, Vendor::Amd) => "av1_amf",
            (Codec::Av1, Vendor::Intel) => "av1_qsv",
        }
    }
}

/// How frames get from the capture source to the encoder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Pipeline {
    /// Frames stay on the GPU the whole way (preferred).
    ZeroCopy,
    /// Frames are copied to system memory first (fallback if zero-copy fails on a PC).
    Download,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Monitor { handle: u64 },
    Window { handle: u64 },
}

#[derive(Debug, Clone, PartialEq)]
pub struct CaptureSpec {
    pub target: Target,
    pub fps: u32,
    pub cursor: bool,
    /// Output size (always even). Set once at the start; the source is scaled to fit it.
    pub width: u32,
    pub height: u32,
}

/// Output size for a source of `src_w` x `src_h`. Never upscales. Always even, aspect kept.
pub fn output_size(src_w: u32, src_h: u32, res: Resolution) -> (u32, u32) {
    let even = |v: u32| (v.max(2)) & !1;
    let (src_w, src_h) = (src_w.max(2), src_h.max(2));
    let target_h = match res {
        Resolution::Native => src_h,
        Resolution::P1440 => 1440,
        Resolution::P1080 => 1080,
        Resolution::P720 => 720,
    };
    if target_h >= src_h {
        return (even(src_w), even(src_h));
    }
    let w = (src_w as f64 * target_h as f64 / src_h as f64).round() as u32;
    (even(w), even(target_h))
}

/// Target bitrate in kbit/s. Bits per pixel per frame, adjusted for codec efficiency.
pub fn bitrate_kbps(width: u32, height: u32, fps: u32, quality: Quality, codec: Codec) -> u32 {
    let bpp = match quality {
        Quality::High => 0.30,
        Quality::Medium => 0.18,
        Quality::Low => 0.10,
    };
    let factor = match codec {
        Codec::H264 => 1.0,
        Codec::Hevc => 0.7,
        Codec::Av1 => 0.6,
    };
    let kbps = width as f64 * height as f64 * fps as f64 * bpp * factor / 1000.0;
    (kbps.round() as u32).clamp(2_000, 150_000)
}

/// The capture part of the filter graph (no output label).
pub fn capture_filter(spec: &CaptureSpec, pipeline: Pipeline, vendor: Vendor) -> String {
    let target = match spec.target {
        Target::Monitor { handle } => format!("hmonitor={handle}"),
        Target::Window { handle } => format!("hwnd={handle}"),
    };
    let mut f = format!(
        "gfxcapture={target}:max_framerate={fps}:capture_cursor={cursor}:display_border=0:\
width={w}:height={h}:resize_mode=scale_aspect,fps={fps}",
        fps = spec.fps,
        cursor = spec.cursor as u8,
        w = spec.width,
        h = spec.height,
    );
    match pipeline {
        Pipeline::ZeroCopy => {
            if vendor == Vendor::Intel {
                f.push_str(",hwmap=derive_device=qsv,format=qsv");
            }
        }
        Pipeline::Download => f.push_str(",hwdownload,format=bgra,format=nv12"),
    }
    f
}

fn encoder_args(enc: Encoder, kbps: u32) -> Vec<String> {
    let b = format!("{kbps}k");
    let max = format!("{}k", kbps * 3 / 2);
    let buf = format!("{}k", kbps * 2);
    let mut a: Vec<String> = Vec::new();
    match enc.vendor {
        Vendor::Nvidia => {
            a.extend(["-preset", "p4", "-tune", "hq", "-rc", "vbr"].map(String::from));
            if enc.codec != Codec::Av1 {
                a.extend(["-forced-idr", "1"].map(String::from));
            }
        }
        Vendor::Amd => {
            a.extend(["-usage", "transcoding", "-quality", "balanced", "-rc", "vbr_peak"].map(String::from));
        }
        Vendor::Intel => {
            a.extend(["-preset", "medium"].map(String::from));
        }
    }
    a.extend(["-b:v".into(), b, "-maxrate".into(), max, "-bufsize".into(), buf]);
    a
}

/// Full argument list for a recording that writes numbered segments into `seg_dir`.
/// FFmpeg is stopped by writing `q` to its stdin, so `-nostdin` must not be used.
pub fn record_args(
    spec: &CaptureSpec,
    enc: Encoder,
    pipeline: Pipeline,
    kbps: u32,
    seg_dir: &Path,
) -> Vec<String> {
    let gop = spec.fps * SEGMENT_SECONDS;
    let mut a: Vec<String> = vec![
        "-hide_banner".into(),
        "-nostats".into(),
        "-loglevel".into(),
        "warning".into(),
        "-filter_complex".into(),
        format!("{}[v]", capture_filter(spec, pipeline, enc.vendor)),
        "-map".into(),
        "[v]".into(),
        "-c:v".into(),
        enc.name().into(),
    ];
    a.extend(encoder_args(enc, kbps));
    a.extend([
        "-g".into(),
        gop.to_string(),
        "-force_key_frames".into(),
        format!("expr:gte(t,n_forced*{SEGMENT_SECONDS})"),
        "-an".into(),
        "-f".into(),
        "segment".into(),
        "-segment_time".into(),
        SEGMENT_SECONDS.to_string(),
        "-segment_format".into(),
        "matroska".into(),
        // Each segment starts at zero; the concat step lays them end to end. Keeping the original
        // timestamps here makes the joined file far too long, with gaps (found in testing).
        "-reset_timestamps".into(),
        "1".into(),
        "-y".into(),
        seg_dir.join("seg_%06d.mkv").to_string_lossy().into_owned(),
    ]);
    a
}

/// Contents of an FFmpeg concat list for the given segment files.
pub fn concat_list(files: &[PathBuf]) -> String {
    let mut s = String::from("ffconcat version 1.0\n");
    for f in files {
        let p = f.to_string_lossy().replace('\\', "/").replace('\'', "'\\''");
        s.push_str(&format!("file '{p}'\n"));
    }
    s
}

/// Joins segments (listed in `list_file`) into the final MP4 without re-encoding.
pub fn finalise_args(list_file: &Path, out: &Path) -> Vec<String> {
    vec![
        "-hide_banner".into(),
        "-loglevel".into(),
        "error".into(),
        "-f".into(),
        "concat".into(),
        "-safe".into(),
        "0".into(),
        "-i".into(),
        list_file.to_string_lossy().into_owned(),
        "-map".into(),
        "0".into(),
        "-c".into(),
        "copy".into(),
        "-movflags".into(),
        "+faststart".into(),
        "-metadata".into(),
        "comment=rec-format:1".into(),
        "-y".into(),
        out.to_string_lossy().into_owned(),
    ]
}

/// Encodes a few blank frames to see whether this PC can use an encoder.
pub fn probe_args(encoder_name: &str) -> Vec<String> {
    [
        "-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i", "color=c=black:s=640x360:r=30",
        "-frames:v", "3", "-c:v", encoder_name, "-f", "null", "-",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

// ---------- running FFmpeg ----------

/// A `Command` that never opens a console window on Windows.
pub fn hidden_command(program: &Path) -> Command {
    #[allow(unused_mut)]
    let mut c = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        c.creation_flags(CREATE_NO_WINDOW);
    }
    c
}

/// Finds FFmpeg: explicit path, then next to the engine, then on PATH.
pub fn locate(explicit: Option<PathBuf>) -> PathBuf {
    if let Some(p) = explicit {
        return p;
    }
    let exe_name = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };
    if let Ok(me) = std::env::current_exe() {
        if let Some(dir) = me.parent() {
            let candidate = dir.join(exe_name);
            if candidate.exists() {
                return candidate;
            }
        }
    }
    PathBuf::from(exe_name)
}

fn run_capture_output(ffmpeg: &Path, args: &[String]) -> Option<(bool, String)> {
    let out = hidden_command(ffmpeg)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .ok()?;
    let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&out.stderr));
    Some((out.status.success(), text))
}

pub fn version(ffmpeg: &Path) -> Option<String> {
    let (_, text) = run_capture_output(ffmpeg, &["-hide_banner".into(), "-version".into()])?;
    text.lines().next().map(|l| l.trim().to_string())
}

/// True if this FFmpeg has the Windows Graphics Capture source.
pub fn has_gfxcapture(ffmpeg: &Path) -> bool {
    match run_capture_output(ffmpeg, &["-hide_banner".into(), "-h".into(), "filter=gfxcapture".into()]) {
        Some((_, text)) => text.contains("gfxcapture") && !text.contains("Unknown filter"),
        None => false,
    }
}

pub fn probe_encoder(ffmpeg: &Path, enc: Encoder) -> bool {
    matches!(run_capture_output(ffmpeg, &probe_args(enc.name())), Some((true, _)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> CaptureSpec {
        CaptureSpec { target: Target::Monitor { handle: 65537 }, fps: 60, cursor: true, width: 1920, height: 1080 }
    }

    #[test]
    fn output_size_never_upscales_and_stays_even() {
        assert_eq!(output_size(1920, 1080, Resolution::Native), (1920, 1080));
        assert_eq!(output_size(1920, 1080, Resolution::P1440), (1920, 1080));
        assert_eq!(output_size(2560, 1440, Resolution::P1080), (1920, 1080));
        assert_eq!(output_size(3440, 1440, Resolution::P1080), (2580, 1080));
        assert_eq!(output_size(1365, 767, Resolution::Native), (1364, 766));
        let (w, h) = output_size(1001, 1001, Resolution::P720);
        assert!(w % 2 == 0 && h % 2 == 0);
        assert_eq!(output_size(0, 0, Resolution::Native), (2, 2));
    }

    #[test]
    fn bitrate_is_sensible() {
        let b = bitrate_kbps(1920, 1080, 60, Quality::High, Codec::H264);
        assert!((30_000..=45_000).contains(&b), "{b}");
        assert!(bitrate_kbps(1920, 1080, 60, Quality::High, Codec::Av1) < b);
        assert!(bitrate_kbps(1920, 1080, 60, Quality::Low, Codec::H264) < b);
        assert_eq!(bitrate_kbps(320, 200, 30, Quality::Low, Codec::H264), 2_000);
        assert_eq!(bitrate_kbps(7680, 4320, 240, Quality::High, Codec::H264), 150_000);
    }

    #[test]
    fn capture_filter_variants() {
        let s = spec();
        let f = capture_filter(&s, Pipeline::ZeroCopy, Vendor::Nvidia);
        assert!(f.starts_with("gfxcapture=hmonitor=65537:max_framerate=60:capture_cursor=1"));
        assert!(f.contains("width=1920:height=1080:resize_mode=scale_aspect"));
        assert!(f.ends_with(",fps=60"));
        assert!(capture_filter(&s, Pipeline::ZeroCopy, Vendor::Intel).ends_with("hwmap=derive_device=qsv,format=qsv"));
        assert!(capture_filter(&s, Pipeline::Download, Vendor::Amd).ends_with("hwdownload,format=bgra,format=nv12"));
        let w = CaptureSpec { target: Target::Window { handle: 42 }, cursor: false, ..s };
        let f = capture_filter(&w, Pipeline::ZeroCopy, Vendor::Amd);
        assert!(f.starts_with("gfxcapture=hwnd=42:"));
        assert!(f.contains("capture_cursor=0"));
    }

    #[test]
    fn record_args_shape() {
        let enc = Encoder { codec: Codec::H264, vendor: Vendor::Nvidia };
        let a = record_args(&spec(), enc, Pipeline::ZeroCopy, 37_000, Path::new("/tmp/seg"));
        let joined = a.join(" ");
        assert!(joined.contains("-c:v h264_nvenc"));
        assert!(joined.contains("-b:v 37000k -maxrate 55500k -bufsize 74000k"));
        assert!(joined.contains("-g 120"));
        assert!(joined.contains("-forced-idr 1"));
        assert!(joined.contains("-f segment -segment_time 2 -segment_format matroska -reset_timestamps 1"));
        assert!(!a.contains(&"-nostdin".to_string()));
        assert!(a.last().unwrap().ends_with("seg_%06d.mkv"));
        let av1 = Encoder { codec: Codec::Av1, vendor: Vendor::Nvidia };
        assert!(!record_args(&spec(), av1, Pipeline::ZeroCopy, 1, Path::new("x")).contains(&"-forced-idr".to_string()));
    }

    #[test]
    fn encoder_names_cover_every_combination() {
        let mut names = std::collections::HashSet::new();
        for c in ALL_CODECS {
            for v in VENDOR_ORDER {
                names.insert(Encoder { codec: c, vendor: v }.name());
            }
        }
        assert_eq!(names.len(), 9);
    }

    #[test]
    fn concat_list_escapes_paths() {
        let l = concat_list(&[PathBuf::from(r"C:\Temp\it's\seg_000001.mkv")]);
        assert!(l.starts_with("ffconcat version 1.0\n"));
        assert!(l.contains("file 'C:/Temp/it'\\''s/seg_000001.mkv'"));
    }

    #[test]
    fn finalise_tags_file_for_the_editor() {
        let a = finalise_args(Path::new("list.txt"), Path::new("out.mp4"));
        assert!(a.windows(2).any(|w| w[0] == "-metadata" && w[1] == "comment=rec-format:1"));
        assert!(a.windows(2).any(|w| w[0] == "-c" && w[1] == "copy"));
    }
}
