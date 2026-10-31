// No console window in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Encore app shell: window, engine process, and a thin bridge to the interface.
//! Keep this thin: no recording logic here (see ARCHITECTURE.md).

mod engine_link;
mod updater;

use engine_link::EngineLink;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tauri::{Manager, RunEvent};

/// Forwards a command to the engine. Runs on a worker thread so the window never freezes.
#[tauri::command]
async fn engine_call(
    link: tauri::State<'_, Arc<EngineLink>>,
    cmd: String,
    args: Option<Value>,
) -> Result<Value, String> {
    let link = link.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        link.call(&cmd, args.unwrap_or(Value::Null), Duration::from_secs(60))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Opens Explorer with the file selected.
#[tauri::command]
fn reveal_in_folder(path: String) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("explorer")
            .raw_arg(format!("/select,\"{}\"", path.replace('/', "\\")))
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(not(windows))]
    let _ = path;
    Ok(())
}

/// Opens a folder in Explorer, creating it first if needed.
#[tauri::command]
fn open_folder(path: String) -> Result<(), String> {
    std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    #[cfg(windows)]
    {
        std::process::Command::new("explorer").arg(&path).spawn().map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn main() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(updater::UpdateState::default())
        .setup(|app| {
            let link = EngineLink::start(app.handle())?;
            app.manage(Arc::new(link));
            updater::start(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            engine_call,
            reveal_in_folder,
            open_folder,
            updater::get_update,
            updater::install_update
        ])
        .build(tauri::generate_context!())
        .expect("Encore failed to start");

    app.run(|handle, event| {
        if let RunEvent::Exit = event {
            // Make sure a recording in progress is saved before the app goes away.
            if let Some(link) = handle.try_state::<Arc<EngineLink>>() {
                link.shutdown();
            }
        }
    });
}
