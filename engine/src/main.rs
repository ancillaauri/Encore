//! encore-engine: the recorder. Launched by the Encore app; talks JSON lines over stdin/stdout.
//!
//! Usage:
//!   encore-engine [--ffmpeg <path>]                    normal mode (driven by the app)
//!   encore-engine --selftest [--ffmpeg <path>]         print a diagnostic report and exit
//!       [--selftest-capture <file.mkv>]                also try a 3-second CPU-path capture

mod audio;
mod engine;
mod ffmpeg;
mod log;
mod paths;
mod platform;
mod protocol;
mod recorder;
mod settings;
mod timeline;

use protocol::{parse_command, send_event, PROTOCOL_VERSION};
use serde_json::json;
use std::io::BufRead;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

struct Args {
    ffmpeg: Option<PathBuf>,
    selftest: bool,
    selftest_capture: Option<PathBuf>,
}

fn parse_args() -> Args {
    let mut a = Args { ffmpeg: None, selftest: false, selftest_capture: None };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--ffmpeg" => a.ffmpeg = it.next().map(PathBuf::from),
            "--selftest" => a.selftest = true,
            "--selftest-capture" => a.selftest_capture = it.next().map(PathBuf::from),
            _ => {}
        }
    }
    a
}

fn main() {
    let args = parse_args();
    platform::init_dpi();
    log::init();
    let ffmpeg_path = ffmpeg::locate(args.ffmpeg.clone());
    log::log(&format!("engine {} starting, ffmpeg at {}", env!("CARGO_PKG_VERSION"), ffmpeg_path.display()));

    if args.selftest {
        std::process::exit(selftest(&ffmpeg_path, args.selftest_capture));
    }

    // Read commands on a separate thread so the main loop can also watch the recording.
    let (tx, rx) = mpsc::channel::<Option<String>>();
    std::thread::spawn(move || {
        let stdin = std::io::stdin();
        for line in stdin.lock().lines() {
            match line {
                Ok(l) => {
                    if tx.send(Some(l)).is_err() {
                        return;
                    }
                }
                Err(_) => break,
            }
        }
        let _ = tx.send(None);
    });

    let mut eng = engine::Engine::new(ffmpeg_path);
    send_event("hello", json!({ "protocol": PROTOCOL_VERSION, "version": env!("CARGO_PKG_VERSION") }));
    eng.recover_leftovers();

    loop {
        match rx.recv_timeout(Duration::from_millis(400)) {
            Ok(Some(line)) => {
                if line.trim().is_empty() {
                    continue;
                }
                match parse_command(&line) {
                    Ok(cmd) => {
                        if eng.handle(cmd) {
                            break;
                        }
                    }
                    Err(e) => send_event("error", json!({ "message": e })),
                }
            }
            // The app closed our stdin (it exited or crashed): save any recording, then exit.
            Ok(None) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                eng.shutdown();
                break;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => eng.tick(),
        }
    }
    log::log("engine exiting");
}

fn selftest(ffmpeg_path: &std::path::Path, capture: Option<PathBuf>) -> i32 {
    let caps = engine::probe(ffmpeg_path);
    let monitors = platform::list_monitors();
    let windows = platform::list_windows();
    let mut report = json!({
        "engine_version": env!("CARGO_PKG_VERSION"),
        "capabilities": caps,
        "monitors": monitors,
        "audio_inputs": audio::list_inputs(),
        "audio_apps": audio::list_apps(),
        "window_count": windows.len(),
        "window_titles": windows.iter().take(8).map(|w| format!("{} ({})", w.title, w.exe)).collect::<Vec<_>>(),
    });
    if let (Some(out), Some(m)) = (capture, monitors.iter().find(|m| m.primary).or(monitors.first())) {
        report["test_capture"] = json!(test_capture(ffmpeg_path, m, &out));
    }
    println!("{}", serde_json::to_string_pretty(&report).unwrap_or_default());
    if caps.ffmpeg_found && caps.capture_supported { 0 } else { 1 }
}

/// Records 3 seconds of a monitor through the CPU path with a software encoder.
/// Lets build machines without a GPU check that capture itself works.
fn test_capture(ffmpeg_path: &std::path::Path, m: &platform::Monitor, out: &std::path::Path) -> serde_json::Value {
    let (w, h) = ffmpeg::output_size(m.width, m.height, settings::Resolution::P720);
    let spec = ffmpeg::CaptureSpec {
        target: ffmpeg::Target::Monitor { handle: m.handle },
        fps: 30,
        cursor: true,
        width: w,
        height: h,
    };
    let filter = format!(
        "{},format=yuv420p[v]",
        ffmpeg::capture_filter(&spec, ffmpeg::Pipeline::Download, ffmpeg::Vendor::Nvidia)
    );
    let args: Vec<String> = vec![
        "-hide_banner", "-loglevel", "warning", "-filter_complex", &filter, "-map", "[v]", "-t", "3",
        "-c:v", "mpeg4", "-q:v", "5", "-y",
    ]
    .into_iter()
    .map(String::from)
    .chain(std::iter::once(out.to_string_lossy().into_owned()))
    .collect();
    match ffmpeg::hidden_command(ffmpeg_path).args(&args).output() {
        Ok(o) => {
            let size = std::fs::metadata(out).map(|m| m.len()).unwrap_or(0);
            json!({
                "ok": o.status.success() && size > 0,
                "bytes": size,
                "stderr": String::from_utf8_lossy(&o.stderr).trim().to_string(),
            })
        }
        Err(e) => json!({ "ok": false, "error": e.to_string() }),
    }
}
