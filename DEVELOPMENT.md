# DEVELOPMENT.md

Read this before working on Encore. Read ARCHITECTURE.md and DECISIONS.md before changing structure,
and FEATURES.md for what is planned.

## What Encore is

A Windows 11 screen recorder for games, designed to pair with a companion clip editor app. Two modes:

- Manual recording: hotkey to start, hotkey to stop.
- Echoes (instant replay): always recording in the background, hotkey saves the last N minutes.

Every recording carries separate audio tracks: full mix, microphone, and one track per app
(game, Spotify, browser, Discord, etc.) so each can be adjusted independently when editing.

## How to work in this repo

- Build in batches, each ending in a hands-on test on a real gaming PC. Do not start the next
  batch until the current one is confirmed working.
- Explain every change in plain English in the commit message and release notes.
- UK English in UI text and docs.
- When a decision is made or changed, add a dated entry to DECISIONS.md with the reason.
- If something is unknown or unverified, say so. Do not guess at Windows API behaviour; test it.
- Build notes, commits and docs do not name individuals or tools used to write the code.

## Hard rules

1. Three layers, fixed contracts (see ARCHITECTURE.md). The interface only talks to `ui/bridge.js`.
   The shell only talks to the engine via the engine protocol. Nothing reaches around these.
2. Video never crosses a layer boundary. Frames go capture -> GPU encoder -> disk, inside the
   engine only. No live preview through the bridge.
3. Performance first. The engine must run with the window closed; the interface must use nothing
   while hidden. Measure idle RAM, CPU while recording and game FPS before and after any change
   that could affect a game, and log the numbers in DECISIONS.md.
4. Never lose a recording. Write to disk in segments as we go, so a crash loses seconds, not the file.
5. Keep the shell thin: window, tray, hotkeys, installer, updates only. Everything else lives in the
   engine or the interface, so the shell can be swapped (Tauri <-> Electron) without touching them.
6. Respect the file contract in ARCHITECTURE.md exactly. The companion editor depends on it.
7. No injection or hooking into game processes, ever (anti-cheat safety).

## Design

**Hard rule: Encore and the companion clip editor are a pair and must look identical.** Same title bar,
menu, panels, dialogs, controls, colours, type, spacing, icons and motion. When the editor's look and
Encore's differ, Encore changes to match. Before adding any new UI, check how the editor does the
equivalent thing and follow it exactly; only invent where the editor has no equivalent, and then in
its style. (Match by recreating its values, not by copying its code, unless its MIT notice is added
to THIRD_PARTY_NOTICES.md.)

Colour tokens (the editor's Discord "Midnight" palette):

```css
:root{
  --bg:#0a090c; --side:#000000; --panel:#1a191c; --panel2:#242327; --line:#1f1e22; --line2:#2e2d31;
  --text:#e2e1e4; --mute:#949398; --dim:#6b6a6f; --acc:#5865f2; --acc2:#4752c4; --red:#f23f43; --ok:#23a55a;
}
body{font:14px/1.4 system-ui,"Segoe UI",sans-serif}
```

Known differences still to fix: see FEATURES.md ("Match the editor exactly").

## Stack

- Interface: plain HTML/CSS/JavaScript, no framework.
- Shell: Tauri 2 (Rust). Swappable for Electron; see ARCHITECTURE.md.
- Engine: Rust, standalone executable (`encore-engine.exe`), launched by the shell.
- Encoding/muxing: FFmpeg 8.1+, bundled, driven by the engine. GPU encoders: NVENC, then AMF, then QSV.
- Audio capture: native Windows WASAPI from the engine (FFmpeg cannot capture per-app audio).
- Builds: GitHub Actions on Windows produces the installer; see BUILDING.md.

## Known gotchas

- WASAPI loopback delivers no packets while a source is silent. The engine fills gaps with
  silence based on the clock, or tracks drift out of sync.
- Browsers play all tabs through one audio process: YouTube is a "Chrome" track, not its own.
- Discord is one track (no per-person split).
- Per-app capture (process loopback) needs Windows 10 build 20348+, i.e. Windows 11 in practice.
- Until the companion editor reads our track titles, it will play the full mix and app tracks
  together (everything doubled). See ARCHITECTURE.md "Companion editor change".
- CI build machines have no GPU, monitor or audio devices: they prove the app builds, not that it records.
