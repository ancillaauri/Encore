//! Tray icon, close-to-tray and global hotkeys. The interface owns what a hotkey does (it knows
//! what is selected to record), so the shell only reports presses as `hotkey` events.

use crate::engine_link::EngineLink;
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, Wry};

const RECORDING_ICON: &[u8] = include_bytes!("../icons/tray-recording.png");

#[derive(Default)]
pub struct ShellState {
    /// Closing the window hides it to the tray instead of quitting (setting `close_to_tray`).
    pub close_to_tray: AtomicBool,
    /// The tray tip about closing to the tray has been shown once this session.
    told_about_tray: AtomicBool,
    tray: Mutex<Option<TrayIcon<Wry>>>,
    record_item: Mutex<Option<MenuItem<Wry>>>,
}

/// Each hotkey setting and the action its press sends to the interface.
const HOTKEYS: [(&str, &str); 7] = [
    ("hotkey_record", "toggle_record"),
    ("hotkey_save_replay", "save_replay"),
    ("hotkey_save_30s", "save_replay_30"),
    ("hotkey_save_60s", "save_replay_60"),
    ("hotkey_save_custom", "save_replay_custom"),
    ("hotkey_lock_game", "lock_game"),
    ("hotkey_next_game", "next_game"),
];

pub fn show_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// A game in the tray menu (from the interface, which hears about games from the engine).
#[derive(serde::Deserialize, Clone)]
pub struct TrayGame {
    pub pid: u32,
    pub name: String,
}

/// Builds the tray menu. With game detection on, a "Game" submenu lists the open games (the one
/// being recorded ticked) so one can be locked from the tray.
fn build_menu(app: &AppHandle, recording: bool, games: Option<(&[TrayGame], Option<u32>, Option<u32>)>) -> tauri::Result<Menu<Wry>> {
    let open = MenuItem::with_id(app, "open", "Open Encore", true, None::<&str>)?;
    let record = MenuItem::with_id(app, "record", if recording { "Stop and save recording" } else { "Start recording" }, true, None::<&str>)?;
    let replay = MenuItem::with_id(app, "replay", "Save an Echo", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Encore", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&open, &record, &replay])?;
    if let Some((list, active, locked)) = games {
        let title = match list.iter().find(|g| Some(g.pid) == active) {
            Some(g) if locked == Some(g.pid) => format!("Game: {} (locked)", g.name),
            Some(g) => format!("Game: {}", g.name),
            None => "Game: none open".to_string(),
        };
        let sub = Submenu::with_id(app, "games", title, !list.is_empty())?;
        for g in list {
            let item = CheckMenuItem::with_id(app, format!("game:{}", g.pid), &g.name, true, Some(g.pid) == active, None::<&str>)?;
            sub.append(&item)?;
        }
        if locked.is_some() {
            sub.append(&PredefinedMenuItem::separator(app)?)?;
            sub.append(&MenuItem::with_id(app, "game:unlock", "Unlock (follow the newest game)", true, None::<&str>)?)?;
        }
        menu.append(&sub)?;
    }
    menu.append(&sep)?;
    menu.append(&quit)?;
    *app.state::<ShellState>().record_item.lock().unwrap() = Some(record);
    Ok(menu)
}

pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let menu = build_menu(app, false, None)?;
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
            other => {
                if let Some(g) = other.strip_prefix("game:") {
                    let _ = app.emit("tray-game", g.to_string());
                }
            }
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
    *app.state::<ShellState>().tray.lock().unwrap() = Some(tray);
    Ok(())
}

/// Rebuilds the tray menu with the open games (`on` false: game detection is off, no submenu).
#[tauri::command]
pub fn set_tray_games(app: AppHandle, on: bool, games: Vec<TrayGame>, active: Option<u32>, locked: Option<u32>, recording: bool) {
    let menu = match build_menu(&app, recording, if on { Some((&games, active, locked)) } else { None }) {
        Ok(m) => m,
        Err(_) => return,
    };
    let state = app.state::<ShellState>();
    let tray_guard = state.tray.lock().unwrap();
    if let Some(tray) = tray_guard.as_ref() {
        let _ = tray.set_menu(Some(menu));
    }
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

/// Quits Encore for real (Main Menu → File → Exit), even when closing hides to the tray.
#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}

/// Closes Encore and opens it again as administrator. If "Start Encore as administrator" is on,
/// the scheduled task reopens it with no prompt; otherwise Windows asks, and if the person says no
/// Encore opens again normally. The new copy waits for this one to finish saving and exit, so the
/// one-copy-only rule doesn't stop it.
#[tauri::command]
pub fn restart_as_admin(app: AppHandle) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let exe = exe.to_string_lossy().trim_start_matches(r"\\?\").replace('\'', "''");
        let start = if crate::admin::wanted() {
            format!(
                "schtasks.exe /Run /TN '{}' | Out-Null; if ($LASTEXITCODE -ne 0) {{ Start-Process -FilePath '{exe}' -Verb RunAs }}",
                crate::admin::TASK_NAME
            )
        } else {
            format!("Start-Process -FilePath '{exe}' -Verb RunAs -ErrorAction Stop")
        };
        let script = format!(
            "Wait-Process -Id {pid} -ErrorAction SilentlyContinue; \
             try {{ {start} }} catch {{ Start-Process -FilePath '{exe}' }}",
            pid = std::process::id()
        );
        std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-Command", &script])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| format!("Couldn't restart Encore: {e}"))?;
        app.exit(0);
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = app;
        Err("Only available on Windows.".into())
    }
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

    let wanted = HOTKEYS
        .iter()
        .map(|(setting, action)| {
            (settings.get(*setting).and_then(|v| v.as_str()).unwrap_or("").trim().to_string(), *action)
        })
        .collect();
    app.state::<crate::hotkeys::Hotkeys>().set(wanted)
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
