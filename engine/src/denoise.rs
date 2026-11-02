//! Microphone noise suppression, applied when a clip is saved (never while recording, so it can't
//! make the capture fall behind). Uses RNNoise, a small neural network trained to keep speech and
//! remove keyboard, fan and room noise. Only the Microphone source is touched.
//!
//! Strong: the cleaned sound. Light: mostly cleaned, with some of the original mixed back in,
//! which sounds more natural and removes less.

use nnnoiseless::DenoiseState;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Level {
    Off,
    Light,
    Strong,
}

pub const LEVELS: [&str; 3] = ["off", "light", "strong"];

impl Level {
    pub fn parse(s: &str) -> Level {
        match s {
            "light" => Level::Light,
            "strong" => Level::Strong,
            _ => Level::Off,
        }
    }
    /// How much of the original stays in, on top of the cleaned sound.
    fn dry(self) -> f32 {
        match self {
            Level::Light => 0.3,
            _ => 0.0,
        }
    }
}

/// The current setting, read when a clip is saved (so a change applies to the next save, Echoes
/// included, without restarting anything).
static LEVEL: AtomicU8 = AtomicU8::new(0);

pub fn set(level: &str) {
    LEVEL.store(Level::parse(level) as u8, Ordering::Relaxed);
}

pub fn current() -> Level {
    match LEVEL.load(Ordering::Relaxed) {
        1 => Level::Light,
        2 => Level::Strong,
        _ => Level::Off,
    }
}

const N: usize = DenoiseState::FRAME_SIZE;
/// RNNoise gives each frame back one frame (10 ms) later: taken off again, so the voice stays in
/// time with the video and the other tracks.
const DELAY: usize = N;

/// Cleans a microphone track: 48 kHz, 16-bit, stereo, interleaved (Encore's audio files). The
/// voice is cleaned as one channel and written to both.
pub fn clean_file(input: &Path, output: &Path, level: Level) -> Result<(), String> {
    let mut reader = BufReader::with_capacity(256 * 1024, std::fs::File::open(input).map_err(|e| e.to_string())?);
    let mut writer = BufWriter::with_capacity(256 * 1024, std::fs::File::create(output).map_err(|e| e.to_string())?);
    let mut state = DenoiseState::new();
    let dry_gain = level.dry();
    let wet_gain = 1.0 - dry_gain;
    // The original, kept back by DELAY samples to line up with the cleaned sound.
    let mut dry_queue: std::collections::VecDeque<f32> = std::collections::VecDeque::with_capacity(2 * N);
    let mut produced = 0usize; // cleaned samples made so far (the first DELAY are dropped)
    let mut total_in = 0usize;
    let mut bytes = vec![0u8; N * 4];
    let mut frame = [0f32; N];
    let mut clean = [0f32; N];
    let mut out_bytes: Vec<u8> = Vec::with_capacity(N * 4);
    let mut finished = false;
    while !finished {
        // One frame of input (or the end: silence to flush out the last DELAY samples).
        let got = read_full(&mut reader, &mut bytes)?;
        let samples = got / 4;
        for (i, f) in frame.iter_mut().enumerate() {
            *f = if i < samples {
                let l = i16::from_le_bytes([bytes[i * 4], bytes[i * 4 + 1]]) as f32;
                let r = i16::from_le_bytes([bytes[i * 4 + 2], bytes[i * 4 + 3]]) as f32;
                (l + r) * 0.5
            } else {
                0.0
            };
        }
        total_in += samples;
        dry_queue.extend(frame[..samples].iter().copied());
        if samples < N {
            // Past the end: once the delayed tail is out, stop.
            if produced >= total_in + DELAY {
                break;
            }
            finished = produced + N >= total_in + DELAY;
        }
        state.process_frame(&mut clean, &frame);
        out_bytes.clear();
        for &c in clean.iter() {
            produced += 1;
            if produced <= DELAY {
                continue; // RNNoise's delay
            }
            if produced - DELAY > total_in {
                break; // nothing past the end of the original
            }
            let d = dry_queue.pop_front().unwrap_or(0.0);
            let v = (c * wet_gain + d * dry_gain).round().clamp(-32768.0, 32767.0) as i16;
            out_bytes.extend_from_slice(&v.to_le_bytes());
            out_bytes.extend_from_slice(&v.to_le_bytes());
        }
        writer.write_all(&out_bytes).map_err(|e| e.to_string())?;
    }
    writer.flush().map_err(|e| e.to_string())
}

/// Reads until `buf` is full or the file ends; returns how many bytes were read.
fn read_full(r: &mut impl Read, buf: &mut [u8]) -> Result<usize, String> {
    let mut n = 0;
    while n < buf.len() {
        match r.read(&mut buf[n..]) {
            Ok(0) => break,
            Ok(k) => n += k,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_pcm(path: &Path, samples: &[i16]) {
        let mut b = Vec::new();
        for &s in samples {
            b.extend_from_slice(&s.to_le_bytes());
            b.extend_from_slice(&s.to_le_bytes());
        }
        std::fs::write(path, b).unwrap();
    }
    fn read_pcm(path: &Path) -> Vec<i16> {
        let b = std::fs::read(path).unwrap();
        b.chunks(4).map(|c| i16::from_le_bytes([c[0], c[1]])).collect()
    }

    #[test]
    fn cleaned_voice_keeps_its_length_and_timing_and_loses_the_noise() {
        let dir = std::env::temp_dir().join(format!("encore-denoise-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // 2 s: a voice-like tone for the first second (with noise), then noise alone.
        let n = 96_000 + 123; // not a whole number of frames
        let mut seed = 7u32;
        let x: Vec<i16> = (0..n)
            .map(|i| {
                let t = i as f32 / 48_000.0;
                seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                let noise = ((seed >> 16) as f32 / 32_768.0 - 1.0) * 1500.0;
                let voice = if t < 1.0 { (1..8).map(|h| (2.0 * std::f32::consts::PI * 150.0 * h as f32 * t).sin() / h as f32).sum::<f32>() * 6000.0 } else { 0.0 };
                (voice + noise) as i16
            })
            .collect();
        let input = dir.join("mic.pcm");
        write_pcm(&input, &x);
        for level in [Level::Strong, Level::Light] {
            let out = dir.join("clean.pcm");
            clean_file(&input, &out, level).unwrap();
            let y = read_pcm(&out);
            assert_eq!(y.len(), x.len(), "same length as the original ({level:?})");
            // In time: the best match between original and cleaned is at no shift.
            let score = |lag: i64| -> f64 {
                (20_000..40_000).map(|i| x[i] as f64 * y[(i as i64 + lag) as usize] as f64).sum()
            };
            let best = (-600..=600).max_by(|&a, &b| score(a).partial_cmp(&score(b)).unwrap()).unwrap();
            assert!(best.abs() <= 2, "in time ({level:?}): best match at {best} samples");
            // The noise-only second is much quieter after cleaning.
            let rms = |v: &[i16]| (v.iter().map(|&s| (s as f64).powi(2)).sum::<f64>() / v.len() as f64).sqrt();
            let before = rms(&x[60_000..90_000]);
            let after = rms(&y[60_000..90_000]);
            let want = if level == Level::Strong { 0.2 } else { 0.5 };
            assert!(after < before * want, "noise reduced ({level:?}): {before:.0} -> {after:.0}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn levels() {
        assert_eq!(Level::parse("strong"), Level::Strong);
        assert_eq!(Level::parse("light"), Level::Light);
        assert_eq!(Level::parse("nonsense"), Level::Off);
        set("light");
        assert_eq!(current(), Level::Light);
        set("off");
        assert_eq!(current(), Level::Off);
    }
}
