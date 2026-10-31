//! Automatic updates (shell job, see ARCHITECTURE.md): checks GitHub Releases in the background,
//! downloads a new version quietly, then the interface shows "Update ready". Clicking it installs
//! and restarts Encore. Updates are signed; the app refuses anything not signed with Encore's key.

use crate::engine_link::EngineLink;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

const FIRST_CHECK_AFTER: Duration = Duration::from_secs(10);
const CHECK_EVERY: Duration = Duration::from_secs(4 * 60 * 60);

struct Ready {
    update: Update,
    bytes: Vec<u8>,
}

#[derive(Default)]
pub struct UpdateState {
    ready: Mutex<Option<Ready>>,
}

fn info(u: &Update) -> Value {
    json!({ "version": u.version, "current": u.current_version, "notes": u.body })
}

/// Checks once; if there is a newer version, downloads and verifies it, then tells the interface.
async fn check_once(app: &AppHandle) -> Result<(), String> {
    if app.state::<UpdateState>().ready.lock().unwrap().is_some() {
        return Ok(());
    }
    let handle = app.clone();
    let updater = app
        .updater_builder()
        // Runs just before the installer takes over: the engine saves any recording and exits.
        .on_before_exit(move || {
            if let Some(link) = handle.try_state::<Arc<EngineLink>>() {
                link.shutdown();
            }
        })
        .build()
        .map_err(|e| e.to_string())?;
    let Some(update) = updater.check().await.map_err(|e| e.to_string())? else {
        return Ok(());
    };
    let bytes = update.download(|_, _| {}, || {}).await.map_err(|e| e.to_string())?;
    let details = info(&update);
    *app.state::<UpdateState>().ready.lock().unwrap() = Some(Ready { update, bytes });
    let _ = app.emit("update-ready", details);
    Ok(())
}

/// Background checks: shortly after start-up, then every few hours.
pub fn start(app: AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(FIRST_CHECK_AFTER);
        loop {
            if let Err(e) = tauri::async_runtime::block_on(check_once(&app)) {
                eprintln!("update check failed: {e}");
            }
            std::thread::sleep(CHECK_EVERY);
        }
    });
}

/// The downloaded update waiting to be installed, if any.
#[tauri::command]
pub fn get_update(state: tauri::State<'_, UpdateState>) -> Option<Value> {
    state.ready.lock().unwrap().as_ref().map(|r| info(&r.update))
}

/// Installs the downloaded update and restarts Encore. Refused while recording.
#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<(), String> {
    let link = app.state::<Arc<EngineLink>>().inner().clone();
    let status = tauri::async_runtime::spawn_blocking(move || {
        link.call("get_status", Value::Null, Duration::from_secs(10))
    })
    .await
    .map_err(|e| e.to_string())??;
    if status.get("state").and_then(|s| s.as_str()) != Some("idle") {
        return Err("Stop the recording first, then install the update.".into());
    }
    let ready = app
        .state::<UpdateState>()
        .ready
        .lock()
        .unwrap()
        .take()
        .ok_or("No update is ready.")?;
    // On Windows this launches the installer, runs on_before_exit, and closes the app.
    // The installer reopens Encore when it's done.
    let result = ready.update.install(&ready.bytes);
    if let Err(e) = result {
        let msg = format!("The update could not be installed: {e}");
        *app.state::<UpdateState>().ready.lock().unwrap() = Some(ready);
        return Err(msg);
    }
    Ok(())
}
