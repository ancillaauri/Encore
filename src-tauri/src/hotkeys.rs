//! Global hotkeys, read by checking the keyboard's state about every 10 ms.
//!
//! Windows' usual hotkey registration (RegisterHotKey) doesn't reach Encore while some games are
//! in front (League of Legends among them): the game reads the keyboard directly and the hotkey
//! message never arrives. Reading the key state instead works whatever has focus, which is how
//! other recorders do it. The keys still reach the game too (nothing is swallowed), so the
//! defaults avoid combinations games use.
//!
//! Each press is sent to the interface as a `hotkey` event (it knows what's selected to record)
//! and noted in the engine's log, so a hotkey that "did nothing" can be told apart from one that
//! was never seen.

#![cfg_attr(not(windows), allow(dead_code, unused_imports))]

use crate::engine_link::EngineLink;
use serde_json::json;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Combo {
    ctrl: bool,
    alt: bool,
    shift: bool,
    win: bool,
    key: i32,
}

/// Parses the interface's names, e.g. "Ctrl+Alt+R", "Shift+F9", "Numpad1", "PageUp".
pub fn parse(text: &str) -> Option<Combo> {
    let mut c = Combo { ctrl: false, alt: false, shift: false, win: false, key: 0 };
    for part in text.split('+').map(str::trim).filter(|p| !p.is_empty()) {
        match part {
            "Ctrl" => c.ctrl = true,
            "Alt" => c.alt = true,
            "Shift" => c.shift = true,
            "Super" | "Win" => c.win = true,
            k => {
                if c.key != 0 {
                    return None;
                }
                c.key = key_code(k)?;
            }
        }
    }
    (c.key != 0).then_some(c)
}

/// Windows virtual-key code for a key name.
fn key_code(name: &str) -> Option<i32> {
    let b = name.as_bytes();
    if b.len() == 1 && (b[0].is_ascii_uppercase() || b[0].is_ascii_digit()) {
        return Some(b[0] as i32);
    }
    if let Some(n) = name.strip_prefix("Numpad").and_then(|d| d.parse::<i32>().ok()) {
        return (0..=9).contains(&n).then_some(0x60 + n);
    }
    if let Some(n) = name.strip_prefix('F').and_then(|d| d.parse::<i32>().ok()) {
        return (1..=24).contains(&n).then_some(0x70 + n - 1);
    }
    Some(match name {
        "Home" => 0x24,
        "End" => 0x23,
        "Insert" => 0x2D,
        "Delete" => 0x2E,
        "PageUp" => 0x21,
        "PageDown" => 0x22,
        "Pause" => 0x13,
        "ScrollLock" => 0x91,
        "Space" => 0x20,
        _ => return None,
    })
}

/// The hotkeys being watched: (combo, action, text as shown).
#[derive(Default)]
pub struct Hotkeys {
    bindings: Mutex<Vec<(Combo, &'static str, String)>>,
}

impl Hotkeys {
    /// Replaces the watched hotkeys. Returns a message for each one that can't be used.
    pub fn set(&self, wanted: Vec<(String, &'static str)>) -> Vec<String> {
        let mut problems = Vec::new();
        let mut list: Vec<(Combo, &'static str, String)> = Vec::new();
        for (text, action) in wanted {
            if text.is_empty() {
                continue;
            }
            match parse(&text) {
                None => problems.push(format!("The hotkey {text} isn't one Encore can use. Pick a different one in Settings → Hotkeys.")),
                Some(c) => {
                    if list.iter().any(|(o, _, _)| *o == c) {
                        problems.push(format!("{text} is set for two different actions. Change one of them in Settings → Hotkeys."));
                    }
                    list.push((c, action, text));
                }
            }
        }
        *self.bindings.lock().unwrap() = list;
        problems
    }
}

#[cfg(windows)]
fn down(vk: i32) -> bool {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
    unsafe { (GetAsyncKeyState(vk) as u16 & 0x8000) != 0 }
}

/// A second press of the same hotkey within this long counts as the same press.
const REPEAT_GAP: Duration = Duration::from_millis(1500);

/// Watches the keyboard on a background thread for as long as Encore runs.
pub fn start(app: AppHandle) {
    #[cfg(windows)]
    std::thread::spawn(move || {
        let mut held: Vec<bool> = Vec::new();
        // When each action last fired: a second press of the same hotkey within REPEAT_GAP is
        // taken as one press (two identical replays were saved from one quick double tap).
        let mut last_fired: std::collections::HashMap<&'static str, std::time::Instant> = Default::default();
        loop {
            std::thread::sleep(Duration::from_millis(10));
            let bindings = app.state::<Hotkeys>().bindings.lock().unwrap().clone();
            if held.len() != bindings.len() {
                held = vec![true; bindings.len()]; // don't fire for keys already down when settings change
            }
            if bindings.is_empty() {
                continue;
            }
            let ctrl = down(0x11);
            let alt = down(0x12);
            let shift = down(0x10);
            let win = down(0x5B) || down(0x5C);
            for (i, (c, action, text)) in bindings.iter().enumerate() {
                let now = down(c.key) && ctrl == c.ctrl && alt == c.alt && shift == c.shift && win == c.win;
                if now && !held[i] {
                    let repeat = last_fired.get(action).map(|t| t.elapsed() < REPEAT_GAP).unwrap_or(false);
                    if repeat {
                        note(&app, format!("hotkey {text} pressed again straight away ({action}): taken as one press"));
                    } else {
                        last_fired.insert(*action, std::time::Instant::now());
                        let _ = app.emit("hotkey", *action);
                        note(&app, format!("hotkey {text} pressed ({action})"));
                    }
                }
                held[i] = now;
            }
        }
    });
    #[cfg(not(windows))]
    let _ = app;
}

/// Writes a line to the engine's log (on another thread, so the caller never waits).
pub fn note(app: &AppHandle, message: String) {
    let link = app.state::<Arc<EngineLink>>().inner().clone();
    std::thread::spawn(move || {
        let _ = link.call("note", json!({ "message": message }), Duration::from_secs(5));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_names() {
        let c = parse("Ctrl+Alt+R").unwrap();
        assert!(c.ctrl && c.alt && !c.shift && !c.win && c.key == b'R' as i32);
        assert_eq!(parse("Ctrl+Alt+1").unwrap().key, 0x31);
        assert_eq!(parse("F9").unwrap().key, 0x78);
        assert_eq!(parse("Shift+Numpad3").unwrap().key, 0x63);
        assert_eq!(parse("Ctrl+PageUp").unwrap().key, 0x21);
        assert!(parse("Ctrl+Alt").is_none());
        assert!(parse("Ctrl+A+B").is_none());
        assert!(parse("Ctrl+Banana").is_none());
    }
}
