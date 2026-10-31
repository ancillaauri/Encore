//! Updates (shell job, see ARCHITECTURE.md). The person chooses how updates behave
//! (the `update_mode` setting, stored with the engine's settings):
//!   - "manual" (default): check now and then and show "Update available"; nothing is downloaded
//!     until they click it.
//!   - "auto_download": download quietly when found and show "Update ready"; they choose when to install.
//!   - "never": no automatic checks. "Check for updates" still works when they ask.
//! Updates are signed; Encore refuses anything not signed with its own key.

use crate::engine_link::EngineLink;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

const FIRST_CHECK_AFTER: Duration = Duration::from_secs(10);
const CHECK_EVERY: Duration = Duration::from_secs(4 * 60 * 60);

enum Stage {
    None,
    Available(Update),
    Ready(Update, Vec<u8>),
}

pub struct UpdateState {
    stage: Mutex<Stage>,
    busy: Mutex<bool>,
}

impl Default for UpdateState {
    fn default() -> Self {
        UpdateState { stage: Mutex::new(Stage::None), busy: Mutex::new(false) }
    }
}

fn describe(stage: &Stage, current: &str) -> Value {
    match stage {
        Stage::None => json!({ "status": "none", "current": current }),
        Stage::Available(u) => json!({ "status": "available", "version": u.version, "current": current }),
        Stage::Ready(u, _) => json!({ "status": "ready", "version": u.version, "current": current }),
    }
}

fn current_version(app: &AppHandle) -> String {
    app.package_info().version.to_string()
}

fn link(app: &AppHandle) -> Arc<EngineLink> {
    app.state::<Arc<EngineLink>>().inner().clone()
}

fn update_mode(app: &AppHandle) -> String {
    link(app)
        .call("get_settings", Value::Null, Duration::from_secs(10))
        .ok()
        .and_then(|s| s.get("update_mode").and_then(|m| m.as_str()).map(String::from))
        .unwrap_or_else(|| "manual".into())
}

async fn find_update(app: &AppHandle) -> Result<Option<Update>, String> {
    let handle = app.clone();
    app.updater_builder()
        // Runs just before the installer takes over: the engine saves any recording and exits.
        .on_before_exit(move || {
            if let Some(l) = handle.try_state::<Arc<EngineLink>>() {
                l.shutdown();
            }
        })
        .build()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())
}

fn set_stage(app: &AppHandle, stage: Stage) -> Value {
    let state = app.state::<UpdateState>();
    let mut s = state.stage.lock().unwrap();
    *s = stage;
    let info = describe(&s, &current_version(app));
    let _ = app.emit("update-status", info.clone());
    info
}

/// Looks for an update. In auto-download mode (and only if asked to) it also downloads it.
async fn check(app: &AppHandle, download: bool) -> Result<Value, String> {
    {
        let state = app.state::<UpdateState>();
        let s = state.stage.lock().unwrap();
        if matches!(*s, Stage::Ready(..)) {
            return Ok(describe(&s, &current_version(app)));
        }
    }
    match find_update(app).await? {
        None => Ok(set_stage(app, Stage::None)),
        Some(update) if download => {
            let bytes = update.download(|_, _| {}, || {}).await.map_err(|e| e.to_string())?;
            Ok(set_stage(app, Stage::Ready(update, bytes)))
        }
        Some(update) => Ok(set_stage(app, Stage::Available(update))),
    }
}

/// Automatic checks, following the person's choice (re-read each time, so changes apply).
pub fn start(app: AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(FIRST_CHECK_AFTER);
        loop {
            let mode = update_mode(&app);
            if mode != "never" {
                if let Err(e) = tauri::async_runtime::block_on(check(&app, mode == "auto_download")) {
                    eprintln!("update check failed: {e}");
                }
            }
            std::thread::sleep(CHECK_EVERY);
        }
    });
}

#[tauri::command]
pub fn get_update(app: AppHandle, state: tauri::State<'_, UpdateState>) -> Value {
    describe(&state.stage.lock().unwrap(), &current_version(&app))
}

/// "Check for updates": always works, whatever the setting. Never downloads.
#[tauri::command]
pub async fn check_for_updates(app: AppHandle) -> Result<Value, String> {
    check(&app, false).await
}

/// Downloads the update if needed (with progress events), then installs it and restarts Encore.
/// Refused while recording.
#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<(), String> {
    {
        let state = app.state::<UpdateState>();
        let mut busy = state.busy.lock().unwrap();
        if *busy {
            return Err("The update is already being installed.".into());
        }
        *busy = true;
    }
    let result = install_inner(&app).await;
    *app.state::<UpdateState>().busy.lock().unwrap() = false;
    result
}

async fn install_inner(app: &AppHandle) -> Result<(), String> {
    let l = link(app);
    let status = tauri::async_runtime::spawn_blocking(move || l.call("get_status", Value::Null, Duration::from_secs(10)))
        .await
        .map_err(|e| e.to_string())??;
    if status.get("state").and_then(|s| s.as_str()) != Some("idle") {
        return Err("Stop the recording first, then install the update.".into());
    }
    let stage = std::mem::replace(&mut *app.state::<UpdateState>().stage.lock().unwrap(), Stage::None);
    let (update, bytes) = match stage {
        Stage::Ready(u, b) => (u, b),
        Stage::Available(u) => {
            let progress = app.clone();
            let mut done: usize = 0;
            match u
                .download(
                    move |chunk, total| {
                        done += chunk;
                        let _ = progress.emit("update-progress", json!({ "done": done, "total": total }));
                    },
                    || {},
                )
                .await
            {
                Ok(b) => (u, b),
                Err(e) => {
                    set_stage(app, Stage::Available(u));
                    return Err(format!("The update could not be downloaded: {e}"));
                }
            }
        }
        Stage::None => return Err("No update is available.".into()),
    };
    // On Windows this launches the installer, runs on_before_exit, and closes Encore.
    // The installer reopens Encore on the new version when it's done.
    let result = update.install(&bytes);
    if let Err(e) = result {
        set_stage(app, Stage::Ready(update, bytes));
        return Err(format!("The update could not be installed: {e}"));
    }
    Ok(())
}
