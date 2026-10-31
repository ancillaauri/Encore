//! The shell's side of the engine protocol: launches encore-engine.exe, sends commands,
//! matches replies to requests, and forwards engine events to the interface.

use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

type Pending = Arc<Mutex<HashMap<u64, mpsc::Sender<Value>>>>;

pub struct EngineLink {
    stdin: Mutex<Option<ChildStdin>>,
    child: Mutex<Option<Child>>,
    pending: Pending,
    next_id: AtomicU64,
}

fn engine_path() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = exe.parent().ok_or("app folder not found")?;
    let name = if cfg!(windows) { "encore-engine.exe" } else { "encore-engine" };
    Ok(dir.join(name))
}

impl EngineLink {
    pub fn start(app: &AppHandle) -> Result<EngineLink, String> {
        let engine = engine_path()?;
        let ffmpeg = app
            .path()
            .resource_dir()
            .map_err(|e| e.to_string())?
            .join(if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" });

        let mut cmd = Command::new(&engine);
        cmd.arg("--ffmpeg").arg(&ffmpeg).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        let mut child = cmd.spawn().map_err(|e| format!("Could not start the recorder engine ({}): {e}", engine.display()))?;
        let stdin = child.stdin.take();
        let stdout = child.stdout.take().ok_or("engine has no stdout")?;

        let pending: Pending = Arc::new(Mutex::new(HashMap::new()));
        let reader_pending = pending.clone();
        let app_handle = app.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                let Ok(msg) = serde_json::from_str::<Value>(&line) else { continue };
                if let Some(id) = msg.get("id").and_then(|v| v.as_u64()) {
                    if let Some(tx) = reader_pending.lock().unwrap().remove(&id) {
                        let _ = tx.send(msg);
                    }
                } else if msg.get("event").is_some() {
                    let _ = app_handle.emit("engine-event", msg);
                }
            }
            // Engine gone: fail anything still waiting, and tell the interface.
            reader_pending.lock().unwrap().clear();
            let _ = app_handle.emit("engine-event", json!({ "event": "engine_exited", "data": {} }));
        });

        Ok(EngineLink {
            stdin: Mutex::new(stdin),
            child: Mutex::new(Some(child)),
            pending,
            next_id: AtomicU64::new(1),
        })
    }

    /// Sends a command and waits for its reply. Blocking: call from a worker thread.
    pub fn call(&self, cmd: &str, args: Value, timeout: Duration) -> Result<Value, String> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = mpsc::channel();
        self.pending.lock().unwrap().insert(id, tx);
        let line = json!({ "id": id, "cmd": cmd, "args": args }).to_string();
        {
            let mut guard = self.stdin.lock().unwrap();
            let stdin = guard.as_mut().ok_or("The recorder engine is not running.")?;
            if stdin.write_all(line.as_bytes()).and_then(|_| stdin.write_all(b"\n")).and_then(|_| stdin.flush()).is_err() {
                self.pending.lock().unwrap().remove(&id);
                return Err("The recorder engine is not running.".into());
            }
        }
        let reply = rx.recv_timeout(timeout).map_err(|_| {
            self.pending.lock().unwrap().remove(&id);
            "The recorder engine did not answer in time.".to_string()
        })?;
        if reply.get("ok").and_then(|v| v.as_bool()) == Some(true) {
            Ok(reply.get("result").cloned().unwrap_or(Value::Null))
        } else {
            Err(reply.get("error").and_then(|v| v.as_str()).unwrap_or("Unknown engine error").to_string())
        }
    }

    /// Asks the engine to save any recording and exit, then waits for it (bounded).
    pub fn shutdown(&self) {
        let _ = self.call("shutdown", json!({}), Duration::from_secs(60));
        self.stdin.lock().unwrap().take(); // closing stdin also tells the engine to exit
        if let Some(mut child) = self.child.lock().unwrap().take() {
            for _ in 0..100 {
                if matches!(child.try_wait(), Ok(Some(_))) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            let _ = child.kill();
        }
    }
}
