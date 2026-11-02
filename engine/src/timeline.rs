//! Places audio on the recording's timeline by timestamp.
//!
//! All times are in 100-nanosecond units on Windows' high-resolution clock (QPC). Audio packets
//! carry that timestamp; video frames are stamped on the same clock. Every track's file starts at
//! the session `origin`, so sample N of every track is the same moment in time.
//!
//! Windows sends no audio packets while a source is silent, so gaps are filled with silence here.
//! Overlaps (timestamps slightly behind what was already written) are trimmed. This also stops
//! slow drift between the sound card's clock and the system clock over long recordings.

// Used by Windows audio capture; on other platforms only by tests.
#![cfg_attr(not(windows), allow(dead_code))]

use std::io::{self, Write};

pub const RATE: u64 = 48_000;
pub const CHANNELS: usize = 2;
const HNS_PER_SEC: i128 = 10_000_000;
/// Timing jitter below this (2 ms) is ignored rather than corrected.
pub const TOLERANCE_FRAMES: i64 = 96;

/// Converts a duration in 100 ns units to audio frames (rounded to nearest).
pub fn hns_to_frames(hns: i64) -> i64 {
    let n = hns as i128 * RATE as i128;
    let half = HNS_PER_SEC / 2;
    (if n >= 0 { (n + half) / HNS_PER_SEC } else { (n - half) / HNS_PER_SEC }) as i64
}

pub fn frames_to_hns(frames: u64) -> u64 {
    (frames as u128 * HNS_PER_SEC as u128 / RATE as u128) as u64
}

pub fn f32_to_s16(x: f32) -> i16 {
    if x.is_nan() {
        return 0;
    }
    (x.clamp(-1.0, 1.0) * 32767.0).round() as i16
}

/// One track's file on disk: 48 kHz, stereo, 16-bit little-endian PCM, starting at `origin`.
pub struct Timeline<W: Write> {
    origin: u64,
    written: u64,
    out: W,
}

impl<W: Write> Timeline<W> {
    pub fn new(origin_hns: u64, out: W) -> Self {
        Timeline { origin: origin_hns, written: 0, out }
    }

    #[cfg(test)]
    pub fn frames_written(&self) -> u64 {
        self.written
    }

    fn position(&self, ts_hns: u64) -> i64 {
        hns_to_frames(ts_hns as i64 - self.origin as i64)
    }

    fn write_silence(&mut self, frames: u64) -> io::Result<()> {
        const CHUNK: usize = 4096;
        let zeros = [0u8; CHUNK * CHANNELS * 2];
        let mut left = frames as usize;
        while left > 0 {
            let n = left.min(CHUNK);
            self.out.write_all(&zeros[..n * CHANNELS * 2])?;
            left -= n;
        }
        self.written += frames;
        Ok(())
    }

    /// How many leading frames of a packet at `ts_hns` to drop, after padding any gap before it.
    fn align(&mut self, ts_hns: u64, frames: usize) -> io::Result<usize> {
        let pos = self.position(ts_hns);
        let w = self.written as i64;
        if pos > w + TOLERANCE_FRAMES {
            self.write_silence((pos - w) as u64)?;
            Ok(0)
        } else if pos < w - TOLERANCE_FRAMES {
            Ok(((w - pos) as usize).min(frames))
        } else {
            Ok(0)
        }
    }

    /// Adds interleaved stereo float samples (as raw little-endian bytes) captured at `ts_hns`.
    pub fn push_f32_bytes(&mut self, ts_hns: u64, bytes: &[u8]) -> io::Result<()> {
        let frames = bytes.len() / (4 * CHANNELS);
        let skip = self.align(ts_hns, frames)?;
        let keep = &bytes[skip * 4 * CHANNELS..frames * 4 * CHANNELS];
        let mut out = Vec::with_capacity(keep.len() / 2);
        for s in keep.chunks_exact(4) {
            let v = f32::from_le_bytes([s[0], s[1], s[2], s[3]]);
            out.extend_from_slice(&f32_to_s16(v).to_le_bytes());
        }
        self.out.write_all(&out)?;
        self.written += (frames - skip) as u64;
        Ok(())
    }

    /// A packet the system marked as silent (no sample data worth reading).
    pub fn push_silence(&mut self, ts_hns: u64, frames: usize) -> io::Result<()> {
        let skip = self.align(ts_hns, frames)?;
        self.write_silence((frames - skip) as u64)
    }

    /// Pads with silence up to `end_hns` (so every track lasts the whole recording) and flushes.
    pub fn finish(mut self, end_hns: u64) -> io::Result<u64> {
        let end = self.position(end_hns);
        if end > self.written as i64 {
            self.write_silence((end - self.written as i64) as u64)?;
        }
        self.out.flush()?;
        Ok(self.written)
    }
}

/// Works out when the video timeline started, on the same clock as the audio.
///
/// FFmpeg reports `out_time_us` (how far into the video it has written) many times a second.
/// Each report gives `arrival - out_time` >= the true start (plus a little encoder/muxer delay).
/// The smallest value seen is the best estimate of the start.
#[derive(Debug, Default, Clone, Copy)]
pub struct VideoClock {
    best: Option<u64>,
    samples: u32,
}

impl VideoClock {
    pub fn observe(&mut self, arrival_hns: u64, out_time_us: i64) {
        if out_time_us <= 0 {
            return;
        }
        let candidate = arrival_hns as i64 - out_time_us * 10;
        if candidate <= 0 {
            return;
        }
        let candidate = candidate as u64;
        self.samples += 1;
        self.best = Some(self.best.map_or(candidate, |b| b.min(candidate)));
    }

    pub fn start_hns(&self) -> Option<u64> {
        self.best
    }

    pub fn samples(&self) -> u32 {
        self.samples
    }
}

/// Reads `out_time_us=<n>` from an FFmpeg `-progress` line.
pub fn parse_out_time_us(line: &str) -> Option<i64> {
    line.trim().strip_prefix("out_time_us=")?.trim().parse::<i64>().ok()
}

/// Seconds of audio to skip so the audio files line up with the video (never negative).
pub fn audio_skip_seconds(origin_hns: u64, video_start_hns: Option<u64>) -> f64 {
    match video_start_hns {
        Some(v) if v > origin_hns => (v - origin_hns) as f64 / 1e7,
        _ => 0.0,
    }
}

/// Bytes per audio frame in the files (stereo, 16-bit).
pub const FRAME_BYTES: u64 = (CHANNELS * 2) as u64;
/// Instant replay writes each track in pieces of this many frames (10 seconds), so pieces older
/// than the replay length can be deleted while recording continues.
pub const CHUNK_FRAMES: u64 = RATE * 10;

/// The file holding chunk `index` of the track `base` (e.g. `a00_full.pcm` -> `a00_full.000003.pcm`).
pub fn chunk_path(base: &std::path::Path, index: u64) -> std::path::PathBuf {
    let stem = base.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    base.with_file_name(format!("{stem}.{index:06}.pcm"))
}

/// A track file written in 10-second pieces. Pieces that end before `keep_from` (a frame
/// position, shared with the engine) are deleted as new ones start.
pub struct ChunkedWriter {
    base: std::path::PathBuf,
    pos: u64,
    current: Option<(u64, std::io::BufWriter<std::fs::File>)>,
    oldest: u64,
    keep_from: std::sync::Arc<std::sync::atomic::AtomicU64>,
}

impl ChunkedWriter {
    pub fn new(base: std::path::PathBuf, keep_from: std::sync::Arc<std::sync::atomic::AtomicU64>) -> Self {
        ChunkedWriter { base, pos: 0, current: None, oldest: 0, keep_from }
    }

    fn prune(&mut self, up_to_chunk: u64) {
        let keep = self.keep_from.load(std::sync::atomic::Ordering::Relaxed);
        // A chunk can go once all of it is before `keep` (and it isn't being written).
        while self.oldest < up_to_chunk && (self.oldest + 1) * CHUNK_FRAMES <= keep {
            let _ = std::fs::remove_file(chunk_path(&self.base, self.oldest));
            self.oldest += 1;
        }
    }
}

impl Write for ChunkedWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let chunk_bytes = CHUNK_FRAMES * FRAME_BYTES;
        let index = self.pos / chunk_bytes;
        if self.current.as_ref().map(|(i, _)| *i) != Some(index) {
            if let Some((_, mut w)) = self.current.take() {
                w.flush()?;
            }
            let f = std::fs::OpenOptions::new().create(true).append(true).open(chunk_path(&self.base, index))?;
            self.current = Some((index, std::io::BufWriter::with_capacity(256 * 1024, f)));
            self.prune(index);
        }
        let room = (chunk_bytes - self.pos % chunk_bytes) as usize;
        let n = buf.len().min(room);
        let (_, w) = self.current.as_mut().unwrap();
        w.write_all(&buf[..n])?;
        self.pos += n as u64;
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        if let Some((_, w)) = self.current.as_mut() {
            w.flush()?;
        }
        Ok(())
    }
}

/// Copies frames `from..to` of a chunked track into one plain file, with silence where pieces are
/// missing (already deleted, or not written yet). Returns the number of frames written.
pub fn extract_frames(base: &std::path::Path, from: u64, to: u64, out: &std::path::Path) -> io::Result<u64> {
    use std::io::{Read, Seek, SeekFrom};
    let mut w = std::io::BufWriter::new(std::fs::File::create(out)?);
    let mut frame = from;
    let mut buf = vec![0u8; 64 * 1024];
    while frame < to {
        let index = frame / CHUNK_FRAMES;
        let chunk_end = ((index + 1) * CHUNK_FRAMES).min(to);
        let mut want = (chunk_end - frame) * FRAME_BYTES;
        if let Ok(mut f) = std::fs::File::open(chunk_path(base, index)) {
            f.seek(SeekFrom::Start((frame % CHUNK_FRAMES) * FRAME_BYTES))?;
            while want > 0 {
                let take = (want as usize).min(buf.len());
                let n = f.read(&mut buf[..take])?;
                if n == 0 {
                    break;
                }
                w.write_all(&buf[..n])?;
                want -= n as u64;
            }
        }
        // Missing or short piece: silence for the rest of it.
        let zeros = vec![0u8; 64 * 1024];
        while want > 0 {
            let n = (want as usize).min(zeros.len());
            w.write_all(&zeros[..n])?;
            want -= n as u64;
        }
        frame = chunk_end;
    }
    w.flush()?;
    Ok(to.saturating_sub(from))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunked_track_rolls_and_extracts() {
        use std::sync::atomic::AtomicU64;
        use std::sync::Arc;
        let dir = std::env::temp_dir().join(format!("encore-chunks-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let base = dir.join("a00_full.pcm");
        let keep = Arc::new(AtomicU64::new(0));
        let mut w = ChunkedWriter::new(base.clone(), keep.clone());
        // 35 s of audio where every frame holds its own second number, written in odd-sized pieces.
        let total = RATE * 35;
        let mut f = 0u64;
        while f < total {
            let n = 3001.min(total - f);
            let mut b = Vec::with_capacity((n * FRAME_BYTES) as usize);
            for i in 0..n {
                let sec = ((f + i) / RATE) as i16;
                b.extend_from_slice(&sec.to_le_bytes());
                b.extend_from_slice(&sec.to_le_bytes());
            }
            if f > RATE * 25 {
                keep.store(RATE * 21, std::sync::atomic::Ordering::Relaxed); // replay wants 21 s on
            }
            w.write_all(&b).unwrap();
            f += n;
        }
        w.flush().unwrap();
        assert!(chunk_path(&base, 3).exists());
        assert!(!chunk_path(&base, 0).exists() && !chunk_path(&base, 1).exists(), "old pieces deleted");
        assert!(chunk_path(&base, 2).exists(), "piece holding 21 s kept");
        // Extract 22.5 s .. 31.25 s and check every frame.
        let out = dir.join("x.pcm");
        let (from, to) = (RATE * 45 / 2, RATE * 125 / 4);
        extract_frames(&base, from, to, &out).unwrap();
        let data = std::fs::read(&out).unwrap();
        assert_eq!(data.len() as u64, (to - from) * FRAME_BYTES);
        for (i, fr) in data.chunks_exact(4).enumerate() {
            let want = ((from + i as u64) / RATE) as i16;
            assert_eq!(i16::from_le_bytes([fr[0], fr[1]]), want);
        }
        // Before what was kept: silence, not an error.
        extract_frames(&base, 0, RATE, &out).unwrap();
        assert!(std::fs::read(&out).unwrap().iter().all(|&b| b == 0));
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn packet(frames: usize, value: f32) -> Vec<u8> {
        let mut v = Vec::new();
        for _ in 0..frames * CHANNELS {
            v.extend_from_slice(&value.to_le_bytes());
        }
        v
    }

    fn samples(buf: &[u8]) -> Vec<i16> {
        buf.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect()
    }

    const MS: u64 = 10_000; // 1 ms in 100 ns units
    const ORIGIN: u64 = 5_000_000_000;

    #[test]
    fn conversions() {
        assert_eq!(hns_to_frames(10_000_000), 48_000);
        assert_eq!(hns_to_frames(10 * MS as i64), 480);
        assert_eq!(hns_to_frames(-10 * MS as i64), -480);
        assert_eq!(frames_to_hns(480), 10 * MS);
        assert_eq!(f32_to_s16(1.5), 32767);
        assert_eq!(f32_to_s16(-1.5), -32767);
        assert_eq!(f32_to_s16(f32::NAN), 0);
        assert_eq!(f32_to_s16(0.5), 16384);
    }

    #[test]
    fn back_to_back_packets_are_written_as_is() {
        let mut t = Timeline::new(ORIGIN, Vec::new());
        t.push_f32_bytes(ORIGIN, &packet(480, 0.5)).unwrap();
        t.push_f32_bytes(ORIGIN + 10 * MS, &packet(480, 0.5)).unwrap();
        assert_eq!(t.frames_written(), 960);
        let out = t.out.clone();
        assert!(samples(&out).iter().all(|&s| s == 16384));
    }

    #[test]
    fn silent_gap_is_filled() {
        let mut t = Timeline::new(ORIGIN, Vec::new());
        t.push_f32_bytes(ORIGIN, &packet(480, 0.5)).unwrap();
        // Next packet arrives 1 second later (source was silent in between).
        t.push_f32_bytes(ORIGIN + 1000 * MS, &packet(480, 0.5)).unwrap();
        assert_eq!(t.frames_written(), 48_000 + 480);
        let s = samples(&t.out);
        assert_eq!(s[480 * 2], 0); // silence right after the first packet
        assert_eq!(s[48_000 * 2], 16384); // second packet lands exactly at 1.000 s
    }

    #[test]
    fn late_starting_source_is_padded_from_the_start() {
        let mut t = Timeline::new(ORIGIN, Vec::new());
        t.push_f32_bytes(ORIGIN + 500 * MS, &packet(480, 0.25)).unwrap();
        assert_eq!(t.frames_written(), 24_000 + 480);
    }

    #[test]
    fn small_jitter_is_ignored() {
        let mut t = Timeline::new(ORIGIN, Vec::new());
        t.push_f32_bytes(ORIGIN, &packet(480, 0.5)).unwrap();
        t.push_f32_bytes(ORIGIN + 11 * MS, &packet(480, 0.5)).unwrap(); // 1 ms late
        t.push_f32_bytes(ORIGIN + 19 * MS, &packet(480, 0.5)).unwrap(); // 1 ms early
        assert_eq!(t.frames_written(), 1440);
    }

    #[test]
    fn overlap_is_trimmed_and_pre_origin_audio_dropped() {
        let mut t = Timeline::new(ORIGIN, Vec::new());
        t.push_f32_bytes(ORIGIN - 5 * MS, &packet(480, 0.5)).unwrap(); // half before origin
        assert_eq!(t.frames_written(), 240);
        t.push_f32_bytes(ORIGIN - 100 * MS, &packet(480, 0.5)).unwrap(); // entirely before origin
        assert_eq!(t.frames_written(), 240);
        let mut t = Timeline::new(ORIGIN, Vec::new());
        t.push_f32_bytes(ORIGIN, &packet(4800, 0.5)).unwrap(); // 100 ms
        t.push_f32_bytes(ORIGIN + 50 * MS, &packet(4800, 0.5)).unwrap(); // overlaps 50 ms
        assert_eq!(t.frames_written(), 4800 + 2400);
    }

    #[test]
    fn silent_packets_and_finish_padding() {
        let mut t = Timeline::new(ORIGIN, Vec::new());
        t.push_silence(ORIGIN, 480).unwrap();
        assert_eq!(t.frames_written(), 480);
        let mut buf = Vec::new();
        {
            let mut t = Timeline::new(ORIGIN, &mut buf);
            t.push_f32_bytes(ORIGIN, &packet(480, 0.5)).unwrap();
            assert_eq!(t.finish(ORIGIN + 2000 * MS).unwrap(), 96_000);
        }
        assert_eq!(buf.len(), 96_000 * 4);
    }

    #[test]
    fn video_clock_takes_the_earliest_estimate() {
        let mut c = VideoClock::default();
        assert_eq!(c.start_hns(), None);
        c.observe(ORIGIN + 400 * MS, 0); // ignored: nothing written yet
        c.observe(ORIGIN + 600 * MS, 200_000); // start estimate ORIGIN + 400 ms
        c.observe(ORIGIN + 700 * MS, 310_000); // ORIGIN + 390 ms (better)
        c.observe(ORIGIN + 800 * MS, 350_000); // ORIGIN + 450 ms (worse, a laggy report)
        assert_eq!(c.start_hns(), Some(ORIGIN + 390 * MS));
        assert_eq!(c.samples(), 3);
    }

    #[test]
    fn progress_lines() {
        assert_eq!(parse_out_time_us("out_time_us=1234567"), Some(1_234_567));
        assert_eq!(parse_out_time_us("out_time_us=N/A"), None);
        assert_eq!(parse_out_time_us("out_time_ms=12"), None);
        assert_eq!(parse_out_time_us("frame=10"), None);
    }

    #[test]
    fn skip_seconds() {
        assert!((audio_skip_seconds(ORIGIN, Some(ORIGIN + 3_500_000)) - 0.35).abs() < 1e-9);
        assert_eq!(audio_skip_seconds(ORIGIN, None), 0.0);
        assert_eq!(audio_skip_seconds(ORIGIN, Some(ORIGIN - 10)), 0.0);
    }
}
