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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CaptureSpec {
    pub target: Target,
    pub fps: u32,
    pub cursor: bool,
    /// Output size (always even). Set once at the start; the source is scaled to fit it.
    pub width: u32,
    pub height: u32,
    /// The quality preset: sets the encoder's quality target (the bitrate is its ceiling).
    pub quality: Quality,
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
        Quality::Ultra => 0.30,
        Quality::High => 0.24,
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

/// NVIDIA's quality target for each preset (lower is better; 0 to 51).
pub fn nvenc_cq(quality: Quality) -> u32 {
    match quality {
        Quality::Ultra => 19,
        Quality::High => 21,
        Quality::Medium => 24,
        Quality::Low => 27,
    }
}

fn encoder_args(enc: Encoder, kbps: u32, quality: Quality) -> Vec<String> {
    let b = format!("{kbps}k");
    let max = format!("{}k", kbps * 3 / 2);
    let buf = format!("{}k", kbps * 2);
    let mut a: Vec<String> = Vec::new();
    match enc.vendor {
        Vendor::Nvidia => {
            // Aim for a quality level, with the preset's bitrate as the ceiling: with only an
            // average to aim for, the encoder spent half of it in simple scenes (Low at 1440p 60
            // used 11 of 22 Mbps) and starved the frames between keyframes. Spatial AQ spends
            // bits where the eye notices (dark areas, gradients, edges); every NVIDIA encoder
            // generation supports it.
            a.extend(["-preset", "p4", "-tune", "hq", "-rc", "vbr"].map(String::from));
            a.extend(["-cq".into(), nvenc_cq(quality).to_string(), "-spatial-aq".into(), "1".into()]);
            if enc.codec != Codec::Av1 {
                a.extend(["-forced-idr", "1"].map(String::from));
            }
            // Ceiling only: the average comes from the quality target.
            let cap = format!("{}k", kbps * 5 / 4);
            a.extend(["-b:v".into(), "0".into(), "-maxrate".into(), cap, "-bufsize".into(), buf]);
            a.extend(["-bf".into(), "0".into()]);
            return a;
        }
        Vendor::Amd => {
            a.extend(["-usage", "transcoding", "-quality", "balanced", "-rc", "vbr_peak"].map(String::from));
        }
        Vendor::Intel => {
            a.extend(["-preset", "medium"].map(String::from));
        }
    }
    a.extend(["-b:v".into(), b, "-maxrate".into(), max, "-bufsize".into(), buf]);
    // No B-frames: with them, joining the 2-second segments shifted the whole video 2 frames
    // later than the audio (found in testing). The compression cost is a few percent.
    a.extend(["-bf".into(), "0".into()]);
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
    prefix: &str,
) -> Vec<String> {
    let gop = spec.fps * SEGMENT_SECONDS;
    let mut a: Vec<String> = vec![
        "-hide_banner".into(),
        "-nostats".into(),
        "-loglevel".into(),
        "warning".into(),
        // Progress reports on stdout let the engine work out exactly when the video started,
        // which is how the audio tracks are lined up with it.
        "-progress".into(),
        "pipe:1".into(),
        "-stats_period".into(),
        "0.05".into(),
        "-filter_complex".into(),
        format!("{}[v]", capture_filter(spec, pipeline, enc.vendor)),
        "-map".into(),
        "[v]".into(),
        "-c:v".into(),
        enc.name().into(),
    ];
    a.extend(encoder_args(enc, kbps, spec.quality));
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
        seg_dir.join(format!("{prefix}%06d.mkv")).to_string_lossy().into_owned(),
    ]);
    a
}

/// One part's segments for the join, and how long to keep of it (None: all of it).
#[derive(Debug, Clone)]
pub struct PartFiles {
    /// The part's segments in order, without gaps, starting at segment number `first_index`.
    pub files: Vec<PathBuf>,
    pub first_index: u64,
    /// How much of the part to keep, from the part's own start (its segment 0).
    pub keep_secs: Option<f64>,
}

/// A concat list that plays the parts end to end, each cut at `keep_secs`. Every segment but a
/// part's last is exactly SEGMENT_SECONDS long (keyframes are forced on that grid and the frame
/// rate is constant), so the cut lands in a known segment; frames after it are dropped with
/// `outpoint`, which needs no re-encoding because nothing refers back to later frames (no B-frames).
pub fn concat_list_parts(parts: &[PartFiles]) -> String {
    let seg = SEGMENT_SECONDS as f64;
    let mut s = String::from("ffconcat version 1.0\n");
    for part in parts {
        let mut last: Option<(f64, bool)> = None; // (start of the last file listed, cut with outpoint)
        for (k, f) in part.files.iter().enumerate() {
            let begins = (part.first_index + k as u64) as f64 * seg;
            if let Some(keep) = part.keep_secs {
                if begins >= keep - 0.0005 {
                    break;
                }
            }
            let p = f.to_string_lossy().replace('\\', "/").replace('\'', "'\\''");
            s.push_str(&format!("file '{p}'\n"));
            let mut cut = false;
            if let Some(keep) = part.keep_secs {
                if keep < begins + seg - 0.0005 {
                    s.push_str(&format!("outpoint {:.6}\n", keep - begins));
                    cut = true;
                }
            }
            last = Some((begins, cut));
        }
        // The part stopped before the next one took over (e.g. a game's window closed, so its
        // capture ended a moment before the new one started). Stretch its last file to the
        // hand-over so everything after stays in time with the audio; players hold the last frame.
        if let (Some(keep), Some((begins, false))) = (part.keep_secs, last) {
            if keep > begins + seg + 0.0005 {
                s.push_str(&format!("duration {:.6}\n", keep - begins));
            }
        }
    }
    s
}

/// An audio track for the saved file: one or more recorded PCM files (raw 48 kHz stereo s16le,
/// aligned to the session origin). Several files are mixed together into the one track.
#[derive(Debug, Clone)]
pub struct AudioOut {
    pub title: String,
    pub files: Vec<PathBuf>,
}

/// Joins video segments (listed in `list_file`) without re-encoding them, and attaches the audio
/// tracks in order (encoded to AAC). `audio_skip` seconds are cut from the start of every audio
/// file so audio and video start at the same moment. The first audio track is the default one
/// players use.
pub fn finalise_args(list_file: &Path, audio: &[AudioOut], audio_skip: f64, out: &Path) -> Vec<String> {
    let mut a: Vec<String> = vec![
        "-hide_banner".into(),
        "-loglevel".into(),
        "error".into(),
        "-f".into(),
        "concat".into(),
        "-safe".into(),
        "0".into(),
        "-i".into(),
        list_file.to_string_lossy().into_owned(),
    ];
    // Each recorded file is opened once, even if it feeds several tracks.
    let mut files: Vec<&PathBuf> = Vec::new();
    for t in audio {
        for f in &t.files {
            if !files.contains(&f) {
                files.push(f);
            }
        }
    }
    for f in &files {
        a.extend([
            "-ss".into(),
            format!("{audio_skip:.6}"),
            "-f".into(),
            "s16le".into(),
            "-ar".into(),
            "48000".into(),
            "-ac".into(),
            "2".into(),
            "-i".into(),
            f.to_string_lossy().into_owned(),
        ]);
    }
    let input_of = |f: &PathBuf| files.iter().position(|x| *x == f).unwrap() + 1;
    // Mixed tracks: a file used by more than one track has to be split before mixing.
    let mut uses: Vec<usize> = vec![0; files.len()];
    for t in audio {
        for f in &t.files {
            uses[input_of(f) - 1] += 1;
        }
    }
    let mut graph: Vec<String> = Vec::new();
    let mut taken: Vec<usize> = vec![0; files.len()];
    let mut label = |f: &PathBuf, graph: &mut Vec<String>| -> String {
        let i = input_of(f);
        if uses[i - 1] <= 1 {
            return format!("[{i}:a]");
        }
        if taken[i - 1] == 0 {
            let outs: String = (0..uses[i - 1]).map(|n| format!("[s{i}_{n}]")).collect();
            graph.push(format!("[{i}:a]asplit={}{outs}", uses[i - 1]));
        }
        let l = format!("[s{i}_{}]", taken[i - 1]);
        taken[i - 1] += 1;
        l
    };
    let mut maps: Vec<String> = Vec::new();
    for (n, t) in audio.iter().enumerate() {
        if t.files.len() == 1 && uses[input_of(&t.files[0]) - 1] <= 1 {
            maps.push(format!("{}:a:0", input_of(&t.files[0])));
            continue;
        }
        let ins: String = t.files.iter().map(|f| label(f, &mut graph)).collect();
        if t.files.len() == 1 {
            graph.push(format!("{ins}anull[t{n}]"));
        } else {
            // normalize=0: sources keep their own volume rather than being turned down.
            graph.push(format!("{ins}amix=inputs={}:duration=longest:dropout_transition=0:normalize=0[t{n}]", t.files.len()));
        }
        maps.push(format!("[t{n}]"));
    }
    if !graph.is_empty() {
        a.extend(["-filter_complex".into(), graph.join(";")]);
    }
    a.extend(["-map".into(), "0:v:0".into()]);
    for m in maps {
        a.extend(["-map".into(), m]);
    }
    a.extend(["-c:v".into(), "copy".into()]);
    if !audio.is_empty() {
        a.extend(["-c:a".into(), "aac".into()]);
        for (i, t) in audio.iter().enumerate() {
            // The first track (Full mix) is what people play and share: full quality. The others
            // are for editing; 128k AAC is still clean and keeps files smaller.
            a.extend([format!("-b:a:{i}"), if i == 0 { "192k".into() } else { "128k".into() }]);
            a.extend([
                format!("-metadata:s:a:{i}"),
                format!("title={}", t.title),
                format!("-metadata:s:a:{i}"),
                format!("handler_name={}", t.title),
                format!("-disposition:a:{i}"),
                if i == 0 { "default".into() } else { "0".into() },
            ]);
        }
        // Audio is padded to the moment recording stopped; stop at whichever stream ends first.
        a.push("-shortest".into());
    }
    a.extend([
        "-movflags".into(),
        "+faststart".into(),
        "-metadata".into(),
        "comment=rec-format:2".into(),
        "-y".into(),
        out.to_string_lossy().into_owned(),
    ]);
    a
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

/// A small JPEG of one frame of a saved clip (for the clip list), `at` seconds in.
pub fn thumbnail(ffmpeg: &Path, file: &Path, at: f64) -> Result<Vec<u8>, String> {
    let out = hidden_command(ffmpeg)
        .args(["-hide_banner", "-loglevel", "error", "-ss", &format!("{:.2}", at.max(0.0)), "-i"])
        .arg(file)
        .args(["-frames:v", "1", "-vf", "scale=256:-2", "-q:v", "5", "-an", "-f", "image2pipe", "-c:v", "mjpeg", "pipe:1"])
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|e| format!("couldn't start FFmpeg: {e}"))?;
    if !out.status.success() || out.stdout.len() < 100 {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(out.stdout)
}

/// Standard base64, for sending a thumbnail to the interface as a data URL.
pub fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

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

    #[test]
    fn base64_matches_the_standard() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    fn spec() -> CaptureSpec {
        CaptureSpec { target: Target::Monitor { handle: 65537 }, fps: 60, cursor: true, width: 1920, height: 1080, quality: Quality::Ultra }
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
    fn a_part_that_stopped_early_keeps_its_slot() {
        // Part 1 has only 2 segments (4 s) but part 2 took over at 5 s: its last file is stretched.
        let parts = vec![
            PartFiles { files: vec![PathBuf::from("a0.mkv"), PathBuf::from("a1.mkv")], first_index: 0, keep_secs: Some(5.0) },
            PartFiles { files: vec![PathBuf::from("b0.mkv")], first_index: 0, keep_secs: None },
        ];
        let list = concat_list_parts(&parts);
        assert!(list.contains("file 'a1.mkv'\nduration 3.000000\n"), "{list}");
        // A normal hand-over (the part reached the moment) is cut with outpoint, no duration.
        let normal = vec![PartFiles { files: vec![PathBuf::from("a0.mkv"), PathBuf::from("a1.mkv"), PathBuf::from("a2.mkv")], first_index: 0, keep_secs: Some(5.0) }];
        let l2 = concat_list_parts(&normal);
        assert!(l2.contains("file 'a2.mkv'\noutpoint 1.000000\n") && !l2.contains("duration"), "{l2}");
    }

    #[test]
    fn bitrate_is_sensible() {
        let b = bitrate_kbps(1920, 1080, 60, Quality::Ultra, Codec::H264);
        assert!((30_000..=45_000).contains(&b), "{b}");
        let h = bitrate_kbps(1920, 1080, 60, Quality::High, Codec::H264);
        let m = bitrate_kbps(1920, 1080, 60, Quality::Medium, Codec::H264);
        assert!(m < h && h < b, "High sits between Medium and Ultra: {m} {h} {b}");
        assert!(bitrate_kbps(1920, 1080, 60, Quality::Ultra, Codec::Av1) < b);
        assert!(bitrate_kbps(1920, 1080, 60, Quality::Low, Codec::H264) < b);
        assert_eq!(bitrate_kbps(320, 200, 30, Quality::Low, Codec::H264), 2_000);
        assert_eq!(bitrate_kbps(7680, 4320, 240, Quality::Ultra, Codec::H264), 150_000);
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
        let a = record_args(&spec(), enc, Pipeline::ZeroCopy, 37_000, Path::new("/tmp/seg"), "seg_");
        let joined = a.join(" ");
        assert!(joined.contains("-c:v h264_nvenc"));
        // NVIDIA: a quality target, with the preset's bitrate (plus a quarter) as the ceiling.
        assert!(joined.contains("-rc vbr -cq 19 -spatial-aq 1"));
        assert!(joined.contains("-b:v 0 -maxrate 46250k -bufsize 74000k"));
        let amd = Encoder { codec: Codec::H264, vendor: Vendor::Amd };
        assert!(record_args(&spec(), amd, Pipeline::ZeroCopy, 37_000, Path::new("x"), "seg_").join(" ").contains("-b:v 37000k -maxrate 55500k -bufsize 74000k"));
        let low = CaptureSpec { quality: Quality::Low, ..spec() };
        assert!(record_args(&low, enc, Pipeline::ZeroCopy, 22_000, Path::new("x"), "seg_").join(" ").contains("-cq 27"));
        assert!(joined.contains("-g 120"));
        assert!(joined.contains("-forced-idr 1"));
        assert!(joined.contains("-bf 0"));
        for v in VENDOR_ORDER {
            let e = Encoder { codec: Codec::Hevc, vendor: v };
            assert!(record_args(&spec(), e, Pipeline::ZeroCopy, 1, Path::new("x"), "seg_").join(" ").contains("-bf 0"));
        }
        assert!(joined.contains("-f segment -segment_time 2 -segment_format matroska -reset_timestamps 1"));
        assert!(!a.contains(&"-nostdin".to_string()));
        assert!(joined.contains("-progress pipe:1 -stats_period 0.05"));
        assert!(a.last().unwrap().ends_with("seg_%06d.mkv"));
        let av1 = Encoder { codec: Codec::Av1, vendor: Vendor::Nvidia };
        assert!(!record_args(&spec(), av1, Pipeline::ZeroCopy, 1, Path::new("x"), "seg_").contains(&"-forced-idr".to_string()));
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
        let l = concat_list_parts(&[PartFiles { files: vec![PathBuf::from(r"C:\Temp\it's\seg_000001.mkv")], first_index: 0, keep_secs: None }]);
        assert!(l.starts_with("ffconcat version 1.0\n"));
        assert!(l.contains("file 'C:/Temp/it'\\''s/seg_000001.mkv'"));
    }

    #[test]
    fn finalise_tags_file_for_the_editor() {
        let a = finalise_args(Path::new("list.txt"), &[], 0.0, Path::new("out.mp4"));
        assert!(a.windows(2).any(|w| w[0] == "-metadata" && w[1] == "comment=rec-format:2"));
        assert!(a.windows(2).any(|w| w[0] == "-c:v" && w[1] == "copy"));
        assert!(!a.contains(&"-shortest".to_string()));
    }

    #[test]
    fn finalise_attaches_titled_audio_tracks_in_order() {
        let p = |s: &str| PathBuf::from(s);
        let audio = vec![
            AudioOut { title: "Full mix".into(), files: vec![p("a00_full.pcm"), p("a01_mic.pcm")] },
            AudioOut { title: "Microphone".into(), files: vec![p("a01_mic.pcm")] },
            AudioOut { title: "Desktop audio".into(), files: vec![p("a00_full.pcm")] },
            AudioOut { title: "Spotify".into(), files: vec![p("a02_app.pcm")] },
        ];
        let a = finalise_args(Path::new("list.txt"), &audio, 0.3125, Path::new("out.mp4"));
        let j = a.join(" ");
        // each file opened once
        assert_eq!(a.iter().filter(|x| x.ends_with(".pcm")).count(), 3);
        assert!(j.contains("-ss 0.312500 -f s16le -ar 48000 -ac 2 -i a00_full.pcm"));
        // desktop and mic each feed two tracks, so they are split, then mixed for Full mix
        assert!(j.contains("[1:a]asplit=2[s1_0][s1_1]"));
        assert!(j.contains("[2:a]asplit=2[s2_0][s2_1]"));
        assert!(j.contains("[s1_0][s2_0]amix=inputs=2:duration=longest:dropout_transition=0:normalize=0[t0]"));
        assert!(j.contains("[s2_1]anull[t1]"));
        assert!(j.contains("[s1_1]anull[t2]"));
        assert!(j.contains("-map 0:v:0 -map [t0] -map [t1] -map [t2] -map 3:a:0"));
        assert!(j.contains("-metadata:s:a:0 title=Full mix"));
        assert!(j.contains("-metadata:s:a:3 title=Spotify -metadata:s:a:3 handler_name=Spotify"));
        assert!(j.contains("-disposition:a:0 default"));
        assert!(j.contains("-disposition:a:1 0"));
        assert!(j.contains("-shortest"));
        assert!(j.contains("-b:a:0 192k") && j.contains("-b:a:1 128k") && j.contains("-b:a:3 128k"));
    }

    #[test]
    fn concat_parts_cut_where_the_next_part_takes_over() {
        let f = |n: &str| PathBuf::from(n);
        let parts = vec![
            PartFiles { files: vec![f("seg_000000.mkv"), f("seg_000001.mkv"), f("seg_000002.mkv"), f("seg_000003.mkv")], first_index: 0, keep_secs: Some(5.25) },
            PartFiles { files: vec![f("p001_000000.mkv"), f("p001_000001.mkv")], first_index: 0, keep_secs: Some(4.0) },
            PartFiles { files: vec![f("p002_000000.mkv")], first_index: 0, keep_secs: None },
        ];
        let l = concat_list_parts(&parts);
        assert_eq!(
            l,
            "ffconcat version 1.0\n\
file 'seg_000000.mkv'\nfile 'seg_000001.mkv'\nfile 'seg_000002.mkv'\noutpoint 1.250000\n\
file 'p001_000000.mkv'\nfile 'p001_000001.mkv'\n\
file 'p002_000000.mkv'\n"
        );
    }

    #[test]
    fn finalise_simple_tracks_need_no_filter() {
        let audio = vec![
            AudioOut { title: "Full mix".into(), files: vec![PathBuf::from("a.pcm")] },
            AudioOut { title: "Spotify".into(), files: vec![PathBuf::from("b.pcm")] },
        ];
        let a = finalise_args(Path::new("l"), &audio, 0.0, Path::new("o.mp4"));
        assert!(!a.contains(&"-filter_complex".to_string()));
        assert!(a.join(" ").contains("-map 0:v:0 -map 1:a:0 -map 2:a:0"));
    }
}
