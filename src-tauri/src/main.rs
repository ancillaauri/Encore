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

/// Windows' folder picker. Returns the chosen folder, or None if cancelled.
#[tauri::command]
async fn pick_folder(app: tauri::AppHandle, start: Option<String>) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    let mut dialog = app.dialog().file().set_title("Choose where to save recordings");
    if let Some(dir) = start.filter(|d| std::path::Path::new(d).is_dir()) {
        dialog = dialog.set_directory(dir);
    }
    if let Some(win) = app.get_webview_window("main") {
        dialog = dialog.set_parent(&win);
    }
    dialog.blocking_pick_folder().and_then(|p| p.into_path().ok()).map(|p| p.display().to_string())
}

/// Opens a web link in the default browser. Only the project's own GitHub pages are allowed.
#[tauri::command]
fn open_link(url: String) -> Result<(), String> {
    // Only Encore's own pages and the pages About links to.
    const ALLOWED: [&str; 3] = [
        "https://github.com/ancillaauri/Encore",
        "https://github.com/BtbN/FFmpeg-Builds",
        "https://ffmpeg.org/download.html",
    ];
    if !ALLOWED.iter().any(|a| url == *a || url.starts_with(&format!("{a}/"))) {
        return Err("That link isn't allowed.".into());
    }
    #[cfg(windows)]
    {
        std::process::Command::new("explorer").arg(&url).spawn().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// The window opens at the companion editor's size (1500x920); on a smaller screen it is shrunk
/// to fit the screen's work area (the screen minus the taskbar) and centred.
fn fit_to_screen(app: &tauri::AppHandle) {
    let Some(win) = app.get_webview_window("main") else { return };
    let (Ok(size), Ok(Some(mon))) = (win.outer_size(), win.current_monitor()) else { return };
    let area = mon.work_area().size;
    if size.width > area.width || size.height > area.height {
        let fitted = tauri::PhysicalSize::new(size.width.min(area.width), size.height.min(area.height));
        let _ = win.set_size(fitted);
        let _ = win.center();
    }
}

fn main() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        // Reopens at the size and place it was closed at, like the companion editor.
        .plugin(tauri_plugin_window_state::Builder::new().build())
        .manage(updater::UpdateState::default())
        .setup(|app| {
            let link = EngineLink::start(app.handle())?;
            app.manage(Arc::new(link));
            updater::start(app.handle().clone());
            fit_to_screen(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            engine_call,
            reveal_in_folder,
            open_folder,
            open_link,
            pick_folder,
            updater::get_update,
            updater::check_for_updates,
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
