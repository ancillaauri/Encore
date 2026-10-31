# ARCHITECTURE.md

Status: draft, written before any code. Sections marked PROVISIONAL must be proven in the build
step named next to them, and updated here once tested.

## The three layers

```
+---------------------------+
| Interface (ui/)           |  HTML/CSS/JS, companion editor look. Talks ONLY to ui/bridge.js
+-------------+-------------+
              | bridge.js (fixed function list, below)
+-------------v-------------+
| Shell (Tauri 2)           |  Window, tray icon, global hotkeys, installer, auto-update.
|                           |  Launches and supervises the engine. No recording logic.
+-------------+-------------+
              | Engine protocol (JSON lines over stdin/stdout)
+-------------v-------------+
| Engine (Rust .exe)        |  Screen capture control, per-app audio, replay buffer,
|                           |  segment files, final MP4. Drives FFmpeg.
+---------------------------+
```

Why: the shell can be replaced (Tauri <-> Electron) by rewriting only the shell and one
implementation of bridge.js. The capture method can be replaced (FFmpeg -> native Windows
capture) inside the engine without the interface noticing.

Video frames and audio samples never leave the engine. Only small commands and status messages cross layers.

## bridge.js (interface <-> shell)

The interface calls only these. Each shell provides its own implementation of this one file.

| Function | Purpose |
|---|---|
| `startRecording()` / `stopRecording()` | Manual recording |
| `startReplay()` / `stopReplay()` / `saveReplay()` | Instant replay buffer |
| `getStatus()` | Current state (idle / recording / replay on, elapsed time, disk use) |
| `getSettings()` / `setSettings(obj)` | Read/write settings |
| `listAudioApps()` | Apps currently making sound, for the settings screen |
| `listMonitors()` / `listWindows()` | Capture targets for the source picker |
| `getCapabilities()` | FFmpeg version, whether window/monitor capture is supported, available encoders |
| `openFolder(path)` | Show a recording in Explorer |
| `onEvent(callback)` | Receive engine events (see below) |

## Engine protocol (shell <-> engine)

Newline-delimited JSON. Shell writes commands to the engine's stdin; engine writes replies and
events to stdout. Logs go to stderr only.

Command: `{"id": 7, "cmd": "start_recording", "args": {}}`
Reply:   `{"id": 7, "ok": true, "result": {...}}` or `{"id": 7, "ok": false, "error": "message"}`
Event:   `{"event": "recording_saved", "data": {"path": "D:\\Recordings\\...mp4"}}`

Commands: `start_recording`, `stop_recording`, `start_replay`, `stop_replay`, `save_replay`,
`get_status`, `get_settings`, `set_settings`, `list_audio_apps`, `list_monitors`, `list_windows`,
`get_capabilities`, `shutdown`. If the engine's stdin closes (the shell exited), it finishes any
recording in progress, saves it, and exits.

Events: `status`, `recording_started`, `recording_saved`, `replay_saved`, `audio_app_added`,
`audio_app_removed`, `warning`, `error`.

Protocol version is sent in the first message (`{"event":"hello","data":{"protocol":1}}`).
Any breaking change bumps the number and gets a DECISIONS.md entry.

## Capture pipeline (PROVISIONAL, steps 3 to 7)

Video (step 3):
- FFmpeg 8.1+ with the `gfxcapture` source (Windows Graphics Capture). Frames are Direct3D 11
  GPU surfaces, handed straight to the GPU encoder.
- Two capture modes (setting + toggle in the UI):
  - Game: one window, selected by `hwnd` from game detection. Alt-tab does not leak the desktop.
  - Desktop: one monitor, selected by `monitor_idx`.
  - Auto / two-position toggle and minimised-window behaviour: see DECISIONS.md open items.
- Fixed output size; `resize_mode=scale_aspect` so resolution changes mid-game do not break the file.
- `display_border` off (no yellow border); cursor capture on by default, setting to turn off.
- Encoder preference: `h264_nvenc` -> `h264_amf` -> `h264_qsv` (proven order in the companion editor).
- Written as short segments (about 2 seconds) into a temp folder. Manual recording keeps all
  segments; replay keeps a rolling window. Final file is assembled at stop/save.
- To verify: our FFmpeg build includes `gfxcapture` and the GPU encoders; frames stay on the GPU;
  behaviour with exclusive-fullscreen games (fallback to Desktop mode if a game records black).
- No injection/hooking into game processes, ever (anti-cheat safety).

Monitor selection (step 3):
- Game mode: not needed; capture follows the game window across monitors.
- Desktop mode setting: Follow the game (default) / Follow the mouse / Primary / Named monitor.
- Monitors are identified by a stable device ID and shown by name, never by index
  (indexes change when monitors are plugged/unplugged).
- Tray and window always show what is being captured (mode + game or monitor name).

Swapping source mid-recording (step 6, PROVISIONAL):
- Applies to monitor swaps, Game <-> Desktop, and Auto mode falling back when the game closes.
- Engine starts the new capture first, cuts over at the next segment boundary, then stops the old one.
- All segments share one fixed output size and frame rate (scale_aspect; black bars if aspect differs),
  so they join cleanly. Audio is unaffected.
- Must measure the hitch at the swap point; if visible, lengthen the overlap before cut-over.
- Replay buffer uses the same mechanism.

Game detection (step 6):
- Candidate = foreground window that is fullscreen or borderless and using the GPU for 3D.
- User list overrides: "always a game" / "never a game", plus "pick a window" manually.
- The detected game names the file, the per-game folder, and its own audio track.

Audio (steps 4 and 5) — built in batch 2:
- Desktop audio: WASAPI loopback on the default output device.
- Microphone: WASAPI capture on the default input device (selectable in settings).
- Per app: WASAPI process loopback, "include process tree" on the app's top-level process
  (so Chrome's child processes count as Chrome).
- App discovery: audio session manager on the default output device; new sessions raise
  `audio_app_added`. Our own app is excluded.
- All sources timestamped against one clock (QueryPerformanceCounter). Silence is inserted for gaps.
- Each source is written as raw 48 kHz stereo 16-bit PCM to its own file, placed by packet
  timestamp (gaps filled with silence, overlaps trimmed): see `engine/src/timeline.rs`.
  `audio.json` in the session folder records the tracks and timing, so crashes recover with audio.
- Sync: FFmpeg's video timestamps and WASAPI packet timestamps are both Windows' QPC clock, but
  FFmpeg rebases video to start at zero. FFmpeg reports progress 20 times a second; the earliest
  `arrival time - video time` is taken as the video's start. Audio is trimmed by
  `video start - origin` when the file is assembled. A hidden `audio_offset_ms` setting allows
  fine adjustment if real-world tests show a constant offset.
- B-frames are disabled: with them, joining segments shifts video 2 frames late.
- Output tracks are planned from the recorded sources (`audio::plan_output`) when the file is
  saved: Full mix = desktop audio (or, if that is off, every app) plus the mic, mixed with FFmpeg's
  `amix` at full level (`normalize=0`); then Microphone, Desktop audio (only when a mic is
  recorded) and each app. A track that would only repeat another is left out.
- Limit: 8 app tracks. Further apps are only in Desktop audio and the Full mix (an "Other" track would need a
  different capture method; not built). Per-app always/never lists: later.
- Audio is encoded to AAC when the recording is saved, so saving a long recording with many
  tracks takes a little while. Encoding during recording is a later optimisation.

Instant replay (step 7):
- Rolling window of video and audio segments on disk (like ShadowPlay), length set in settings.
- `save_replay` assembles the last N minutes into an MP4 without stopping the buffer.

## File contract (what the companion editor relies on)

- Container: MP4. Video H.264 (HEVC/AV1 may be added later as options). Audio AAC, 48 kHz stereo.
- Audio stream order (a track switched off in settings is simply absent, so identify tracks by name):
  - `Full mix` first: everything, including the microphone (the default track, so the file plays
    normally anywhere). Contains every other track.
  - `Microphone`
  - `Desktop audio`: everything the PC played, without the mic (only present when there is a mic
    track). Contains every app track.
  - then one per app, named from the app's file description (e.g. `Spotify`, `Google Chrome`,
    `Discord`, the game's name); duplicates become `Discord 2` etc.
- The track name is stored in each audio stream's `handler_name` (the MP4 muxer does not keep a
  separate `title` tag). NVIDIA's own multi-track files use the same field.
- File-level metadata `comment` = `rec-format:2` so the companion editor can recognise our files.
  History: `rec-format:1` (0.2–0.3.x, and 0.4.0) had Full mix without the mic and no Desktop audio
  track (0.4.0 files have the new layout but the old marker; only test builds made them).
- Apps that start mid-recording still get a full-length track, silent before they started.
- Filename: `<Game or App> YYYY.MM.DD - HH.MM.SS.mp4`, saved in a per-game subfolder.

Any change to this section is a breaking change for the companion editor: bump `rec-format`, log it in DECISIONS.md, and notify the companion editor's maintainer.

## Companion editor change (step 9)

Its `audioGroups()` in `main.js` currently treats any non-Discord, non-NVIDIA multi-track file as
"Game / Desktop" + "Microphone". With our files that would play the full mix AND every app track
together (everything doubled). The change needed mirrors how it already handles Discord's "all" track:

- If `comment` contains `rec-format:1` or `rec-format:2`: one row per audio stream except `Full mix`, named from the
  stream's `handler_name` (Microphone gets the mic icon). `Desktop audio` gets a row only when the
  file has no per-app tracks; otherwise it is hidden like `Full mix` (it already contains them).
- When no slider has been touched, export using the `Full mix` stream only for `rec-format:2`;
  for `rec-format:1` mix `Full mix` + `Microphone` (that Full mix had no mic).
- A `rec-format:1` file that has a `Desktop audio` track was made by 0.4.0 (new layout, old
  marker): treat it as `rec-format:2`.

## Updates (shell)

`src-tauri/src/updater.rs`. Checks `releases/latest/download/latest.json` on GitHub according to
the `update_mode` setting (manual / auto_download / never), 10 seconds after start and every 4 hours.
States sent to the interface as `update-status`: none / available / ready. Installing is refused
while recording; the engine is shut down (saving any recording) just before the installer runs.

## Linking the two apps

First: the companion editor adds our output folder to the folders it scans. No API needed.
Later, only if useful: a small local API (e.g. "replay saved, open in trim view").
