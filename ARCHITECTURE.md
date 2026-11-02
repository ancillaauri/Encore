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
| `startReplay()` / `stopReplay()` / `saveReplay()` | Echoes (the instant replay buffer) |
| `showNotification(device, pos, item)` | A notification over the game (shell window, never recorded) |
| `getStatus()` | Current state (idle / recording / replay on, elapsed time, disk use) |
| `getSettings()` / `setSettings(obj)` | Read/write settings |
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
`get_status`, `get_settings`, `set_settings`, `list_monitors`, `list_windows`,
`get_capabilities`, `shutdown`. If the engine's stdin closes (the shell exited), it finishes any
recording in progress, saves it, and exits.

Events: `hello`, `capabilities`, `status`, `recording_started`, `recording_saved`,
`replay_status`, `replay_saving`, `replay_saved`, `clip_thumbnail`, `games`, `game_switched`,
`focus_monitor`, `monitor_switched`, `audio_app_added`, `capture_behind`, `mic_found`,
`priority_capture_off`, `warning`, `error`.

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

Monitor selection (built in 0.5):
- Game mode: not needed; capture follows the game window across monitors.
- Desktop setting `monitor`: `follow` (default) records the monitor of the window last clicked on
  (the focused window) and moves with it; `id:<monitor>` always records that monitor. Chosen in the
  hologram row under the record button: the Follow tile, or a monitor to pin it (0.8.1).
- Focus rules (`platform::focus_rule`): Encore's own windows, Alt+Tab and Start are ignored; the
  desktop and taskbars use the monitor under the mouse; anything else uses its window's monitor.
  The focus must stay on the new monitor for 0.3 s before the recording moves.
- Monitors are identified by a stable device ID and shown by name, never by index
  (indexes change when monitors are plugged/unplugged).
- Tray and window always show what is being captured (mode + game or monitor name).

Switching monitor mid-recording (built in 0.5; Game <-> Desktop and replay will reuse it):
- A recording is made of video parts. A switch starts a second FFmpeg run on the new monitor
  (segments `p001_*.mkv`, `p002_*`...) while the first keeps recording. Once the new run has
  reported half a second of progress, it takes over and the old one finishes its segment.
- Each part's start is measured on the shared clock (VideoClock per run) and stored in
  `audio.json` (`parts`). When saving, each part is cut exactly where the next one started, using
  the concat list's `outpoint` (no re-encoding: segments are exactly 2 s and there are no
  B-frames). So there is no gap, no repeated footage, and audio stays in sync.
- A switch that never took over is marked `dropped` and left out.
- All parts share one output size and frame rate (scale_aspect; black bars if aspect differs).
- Two encoder sessions run at once for about a second during a switch.

Game detection (step 6):
- Candidate = foreground window that is fullscreen or borderless and using the GPU for 3D.
- User list overrides: "always a game" / "never a game", plus "pick a window" manually.
- The detected game names the file, the per-game folder, and its own audio track.

Audio (steps 4 and 5) — built in batch 2:
- Full System Audio (the `Desktop audio` track): WASAPI process loopback in exclude mode, aimed
  at the Encore app, so everything except Encore; on older Windows, loopback on the default output
  device. Interface names: Full System Audio, Microphone, Split Application Audio; the track
  titles in files stay Full mix / Microphone / Desktop audio / app names (file contract).
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
  `video start - origin` when the file is assembled. There's no manual offset: any track can be
  moved in the editor if ever needed.
- B-frames are disabled: with them, joining segments shifts video 2 frames late.
- Output tracks are planned from the recorded sources (`audio::plan_output`) when the file is
  saved: Full mix = desktop audio (or, if that is off, every app) plus the mic, mixed with FFmpeg's
  `amix` at full level (`normalize=0`); then Microphone, Desktop audio (only when a mic is
  recorded) and each app. A track that would only repeat another is left out.
- Limit: 8 app tracks. Further apps are only in Desktop audio and the Full mix (an "Other" track would need a
  different capture method; not built). Per-app always/never lists: later.
- Audio is encoded to AAC when the recording is saved, so saving a long recording with many
  tracks takes a little while. Encoding during recording is a later optimisation.

Echoes, the instant replay (built in 0.7; called Echoes from 0.9.1; "replay" in the code):
- `start_replay` starts a capture session like a recording, in a `replay-<time>` temp folder. Video
  is the usual 2 s segments (parts for monitor switches); each audio track is written in 10 s
  pieces (`a00_full.000123.pcm`, `timeline::ChunkedWriter`).
- Every engine tick deletes segments and audio pieces older than the replay length plus a margin,
  never past a recording or save still in progress (`pins`).
- `save_replay` joins `now - replay_seconds .. now` on a worker thread (`recorder::save_window`):
  the video starts at the keyframe at or before that moment (up to 2 s extra), is cut exactly at
  the end and at monitor switches, and the audio is cut from its pieces to match. The buffer keeps
  running. Reports `replay_saved`.
- Recording while replay is on uses the same capture: the recording is the stretch from Start to
  Stop, saved the same way, so there is only ever one capture running.
- Turning replay off (or quitting) waits for saves, then deletes the session. A replay folder left
  by a crash is deleted at the next start (not recovered).
- Settings that change what's captured restart the buffer.

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
- Filename: `<Game or App> YYYY.MM.DD - HH.MM.SS.mp4`, saved in
  `<chosen folder>\Encore\<Game, app or Desktop>\` (default chosen folder: Videos). A chosen folder
  already named Encore is used as it is.
- Audio bitrate: Full mix 192 kbps, other tracks 128 kbps (AAC, 48 kHz stereo).

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

## Tray, hotkeys, closing (shell, 0.6)

`src-tauri/src/tray.rs`. A tray icon (red dot while recording) with Open / Start-Stop / Quit.
Closing the window hides it to the tray when `close_to_tray` is on (default), so hotkeys keep
working; the first time, a Windows notification says where Encore went. Main Menu → File → Exit and the
tray's Quit really quit (the engine saves any recording first). A second launch shows the existing
window (single instance). Global hotkeys are watched by the shell (`hotkeys.rs`, reading the key state every 10 ms, since
RegisterHotKey doesn't fire while some games such as League of Legends are in front) from the
engine's settings (`hotkey_*`); each press is noted in the engine log; a press is sent to the interface as a `hotkey` event, and the interface does
what clicking the record button would, so the hotkey records whatever is selected.

## Starting as administrator (shell, 0.7.3)

`src-tauri/src/admin.rs`. Raising the capture's graphics priority needs administrator rights.
"Start Encore as administrator" (Settings → General, and offered in setup) registers a Windows
scheduled task with no trigger that starts Encore with the account's highest rights
(`--from-admin-task`). Turning it on or off asks Windows once (schtasks through the "Do you want to
allow…" prompt, or directly if already elevated). The switch is a marker file,
`%LOCALAPPDATA%\Encore\start-as-admin`, checked first thing in `main()`: if it's there and this
copy isn't elevated, Encore runs the task and exits, so it comes back elevated with no prompt. If the
task is missing, Encore just opens normally. The task-started copy waits 0.4 s so the single-instance
check doesn't see the copy that's closing. Restart as Administrator uses the task too when it's set
up, after waiting for the old copy to exit.

## Notifications (shell, 0.9.15)
Drawn by Encore itself, on the processor, so they need no graphics card time from Encore (a web
page drawn by the card never got its turn with a game using all of the card and the capture at
top priority). `src-tauri/src/notify_draw.rs` turns each notification into a picture once (a
small SVG rendered by resvg: glow, glassy card, border, icon, text wrapped to fit) and composes
the stack each frame (fade and slide in from its edge, fade out, cards moving to their new place,
timer line). `src-tauri/src/notify.rs` runs a plain Win32 layered window on its own thread:
`WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE`,
`WDA_EXCLUDEFROMCAPTURE`, frames handed over with `UpdateLayeredWindow` (about 30 a second, only
while something shows; hidden otherwise), put back on top every 200 ms while showing, and any
window still above it logged. The interface decides whether to show one (settings, Do not
disturb, its own window in front) and calls `show_notification(device, window, pos, item)`; the
shell resolves the screen (chosen by Windows name, else the game's window's screen, else the one
holding the foreground window).

## Updates (shell)

`src-tauri/src/updater.rs`. Checks `releases/latest/download/latest.json` on GitHub according to
the `update_mode` setting (manual / auto_download / never), 10 seconds after start and every 4 hours. A failed check is tried once more after 3 seconds before a
plain "couldn't reach GitHub" message is shown.
States sent to the interface as `update-status`: none / available / ready. Installing is refused
while recording; the engine is shut down (saving any recording) just before the installer runs.

## Linking the two apps

First: the companion editor adds our output folder to the folders it scans. No API needed.
Later, only if useful: a small local API (e.g. "replay saved, open in trim view").

## Game detection (engine, 0.9)

`engine/src/games.rs` decides which windows are games (`classify`) and what they're called
(`game_name`); `Watch` keeps the open games, the lock, and which one to record (newest, or the one
last focused). Order of rules: never list, always list, known launchers/clients/browsers/players/
Windows parts (not games; the League client is one), well-known games by program name, game
library folders (Steam, Epic, Riot, EA, Ubisoft, GOG, Xbox, Amazon), then full screen with no title
bar. Names: well-known list, Steam's `appmanifest_*.acf` for the folder, the library folder name,
then the program's description cleaned ("(TM)", "Client", "-Win64-Shipping" removed). Only window
and file information is read; nothing touches a game's process.

The engine scans once a second when `game_detection` is on (straight away when a recorded game
window disappears). With a desktop session running, the capture switches to the game's window
(`Target::Window`) with the same seamless part switch used for monitors, and back to the chosen
screen (Follow or a pinned monitor) when no game is open. A game window that closes ends its
FFmpeg part before the next one starts; the join stretches that part's last file (`duration` in
the concat list) so everything after stays in time with the audio. Each part carries a label
(game name or "Desktop"); saves and recordings are named after the label on screen for most of the
saved stretch. Settings: `game_pick` (newest / focused), `game_end` (continue / stop),
`games_always`, `games_never`, `desktop_capture` (off, games only: Echoes waits for a game and its
buffer is dropped when games close; recordings end with their game; recording with no game is refused), hotkeys `hotkey_lock_game`, `hotkey_next_game`. Select Application
(`source: window`) pauses detection. Commands: `lock_game`, `lock_focused_game`,
`next_game`; event `games` (list, active, locked, paused) and `game_switched`.
Not yet: telling the person when a game in exclusive full screen captures black.

## Problem reports

Help → Report a problem gathers the engine's `report_details` (log tail with user names taken
out, Windows version, encoders, settings summary) and posts it with the person's words to the
report server (`report-worker/worker.js`, a Cloudflare Worker): `POST /report` files an issue in
a private repository and returns `{ number, key }`; `GET /replies?n=&key=` returns the issue's
state and comments (not ones starting with `//`). The key is an HMAC of the number, so only the
sending PC can read replies. Sent reports are kept in settings (`reports`). Setup:
report-worker/SETUP.md.

