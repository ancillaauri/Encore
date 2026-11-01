//! Tray icon, close-to-tray and global hotkeys. The interface owns what a hotkey does (it knows
//! what is selected to record), so the shell only reports presses as `hotkey` events.

use crate::engine_link::EngineLink;
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, Wry};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

const RECORDING_ICON: &[u8] = include_bytes!("../icons/tray-recording.png");

#[derive(Default)]
pub struct ShellState {
    /// Closing the window hides it to the tray instead of quitting (setting `close_to_tray`).
    pub close_to_tray: AtomicBool,
    /// The tray tip about closing to the tray has been shown once this session.
    told_about_tray: AtomicBool,
    tray: Mutex<Option<TrayIcon<Wry>>>,
    record_item: Mutex<Option<MenuItem<Wry>>>,
    /// Registered hotkeys by setting name (e.g. "hotkey_record" -> "Ctrl+Alt+R").
    hotkeys: Mutex<std::collections::HashMap<String, String>>,
}

/// Each hotkey setting and the action its press sends to the interface.
const HOTKEYS: [(&str, &str); 2] = [("hotkey_record", "toggle_record"), ("hotkey_save_replay", "save_replay")];

pub fn show_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open Encore", true, None::<&str>)?;
    let record = MenuItem::with_id(app, "record", "Start recording", true, None::<&str>)?;
    let replay = MenuItem::with_id(app, "replay", "Save replay", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Encore", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &record, &replay, &PredefinedMenuItem::separator(app)?, &quit])?;
    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip("Encore")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_window(app),
            "record" => {
                let _ = app.emit("hotkey", "toggle_record");
            }
            "replay" => {
                let _ = app.emit("hotkey", "save_replay");
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_window(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    let tray = builder.build(app)?;
    let state = app.state::<ShellState>();
    *state.tray.lock().unwrap() = Some(tray);
    *state.record_item.lock().unwrap() = Some(record);
    Ok(())
}

/// Called when the main window's close button is pressed. Returns true if it was hidden instead.
pub fn hide_instead_of_close(app: &AppHandle) -> bool {
    let state = app.state::<ShellState>();
    if !state.close_to_tray.load(Ordering::Relaxed) {
        return false;
    }
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.hide();
    }
    if !state.told_about_tray.swap(true, Ordering::Relaxed) {
        use tauri_plugin_notification::NotificationExt;
        let _ = app
            .notification()
            .builder()
            .title("Encore is still running")
            .body("It's in the tray by the clock, so your hotkeys keep working. Right-click it to quit.")
            .show();
    }
    true
}

/// Shows recording or idle in the tray (icon, tooltip and the menu's record item).
#[tauri::command]
pub fn set_recording_state(app: AppHandle, recording: bool, label: Option<String>) {
    let state = app.state::<ShellState>();
    if let Some(item) = state.record_item.lock().unwrap().as_ref() {
        let _ = item.set_text(if recording { "Stop and save recording" } else { "Start recording" });
    }
    let tray_guard = state.tray.lock().unwrap();
    if let Some(tray) = tray_guard.as_ref() {
        let icon = if recording {
            Image::from_bytes(RECORDING_ICON).ok()
        } else {
            app.default_window_icon().cloned()
        };
        let _ = tray.set_icon(icon);
        let tip = match (recording, label) {
            (true, Some(l)) => format!("Encore: recording {l}"),
            (true, None) => "Encore: recording".to_string(),
            _ => "Encore".to_string(),
        };
        let _ = tray.set_tooltip(Some(tip));
    }
}

/// Quits Encore for real (≡ → File → Exit), even when closing hides to the tray.
#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}

/// Reads the shell's settings from the engine and applies them: close-to-tray and the hotkeys.
/// Returns a message for each hotkey that couldn't be registered (e.g. taken by another app).
#[tauri::command]
pub async fn apply_shell_settings(app: AppHandle) -> Result<Vec<String>, String> {
    let link = app.state::<Arc<EngineLink>>().inner().clone();
    let settings = tauri::async_runtime::spawn_blocking(move || {
        link.call("get_settings", Value::Null, Duration::from_secs(10))
    })
    .await
    .map_err(|e| e.to_string())??;
    Ok(apply(&app, &settings))
}

pub fn apply(app: &AppHandle, settings: &Value) -> Vec<String> {
    let state = app.state::<ShellState>();
    let close = settings.get("close_to_tray").and_then(|v| v.as_bool()).unwrap_or(true);
    state.close_to_tray.store(close, Ordering::Relaxed);

    let mut registered = state.hotkeys.lock().unwrap();
    let mut problems = Vec::new();
    let gs = app.global_shortcut();
    for (setting, action) in HOTKEYS {
        let wanted = settings.get(setting).and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
        let current = registered.get(setting).cloned().unwrap_or_default();
        if current == wanted {
            continue;
        }
        if !current.is_empty() {
            let _ = gs.unregister(current.as_str());
        }
        registered.remove(setting);
        if wanted.is_empty() {
            continue;
        }
        let result = gs.on_shortcut(wanted.as_str(), move |app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                let _ = app.emit("hotkey", action);
            }
        });
        match result {
            Ok(()) => {
                registered.insert(setting.to_string(), wanted);
            }
            Err(e) => problems.push(format!(
                "The hotkey {wanted} couldn't be set up ({e}). Another app may be using it; pick a different one in Settings → Hotkeys."
            )),
        }
    }
    problems
}

/// At start: apply the saved settings once the engine is up.
pub fn start(app: AppHandle) {
    std::thread::spawn(move || {
        let link = app.state::<Arc<EngineLink>>().inner().clone();
        for _ in 0..20 {
            if let Ok(settings) = link.call("get_settings", Value::Null, Duration::from_secs(10)) {
                for p in apply(&app, &settings) {
                    let _ = app.emit("engine-event", serde_json::json!({ "event": "warning", "data": { "message": p } }));
                }
                return;
            }
            std::thread::sleep(Duration::from_millis(500));
        }
    });
}
