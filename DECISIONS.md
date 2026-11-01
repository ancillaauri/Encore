# DECISIONS.md

Dated log of decisions and why. Newest at the bottom. Never delete entries; mark them superseded.

## 2026-10-31: Separate app, own repo
Encore is its own app, separate from the companion clip editor. The two connect through the file
format and a shared folder first; an API only if a real use appears.

## 2026-10-31: Features in scope
Manual recording and instant replay (ShadowPlay-style). Separate audio tracks: full mix, mic,
one per app. Target: Windows 11 only (per-app audio needs it).

## 2026-10-31: Three-layer architecture
Interface / shell / engine with fixed contracts (ARCHITECTURE.md). Reason: lets us change the shell
or capture method later after measuring real performance, without rebuilding the app. Cost is one
extra small process and a few MB of RAM; no measurable CPU or FPS cost because video never crosses layers.

## 2026-10-31: Shell = Tauri 2 (provisional, review after batch 1)
Over Electron: much smaller install and idle RAM, which matters for an app that runs in the
background during games. Over C#/.NET: Tauri keeps a web interface, matching the companion editor's
style. Electron remains a fallback; switching means rewriting the shell and bridge.js only.

## 2026-10-31: Engine = Rust + FFmpeg + WASAPI
FFmpeg for GPU encoding and building the MP4. FFmpeg cannot capture per-app audio, so audio capture
is native WASAPI in Rust.

## 2026-10-31: Track layout
Full mix, then Microphone, then one track per app (named). Full mix first so files play normally
everywhere (players and Discord use the first audio track).

## 2026-10-31: Design
Own stylesheet in the Discord "Midnight" style used by the companion editor, so the two look like
a set. No code copied from the editor.

## 2026-10-31: Capture modes, Game window or Desktop
Users can record only the detected game's window or the whole desktop. Capture uses FFmpeg's
`gfxcapture` source (Windows Graphics Capture, FFmpeg 8.1+), which handles both a single window
and a whole monitor, keeping frames on the GPU. Supersedes the earlier `ddagrab` (desktop-only) plan.
No game injection/hooking (OBS-style game capture) because of anti-cheat risk.
Consequence: we bundle our own FFmpeg 8.1+ build (the companion editor bundles FFmpeg 6.1.1, which lacks `gfxcapture`).

## 2026-10-31: Feature backlog agreed
All "core" features and all "later" features from the competitor review accepted, except HDR
recording (not wanted). Full list and build-step slots in FEATURES.md.

## 2026-10-31: Monitor selection and mid-recording swaps
Desktop mode offers Follow the game (default) / Follow the mouse / Primary / Named monitor.
Monitors tracked by stable ID, shown by name. Mid-recording swaps (monitor, mode, Auto fallback)
work by overlapping the new capture and cutting over at a segment boundary, with fixed output size.
Hitch at the swap point to be measured in batch 3.

## 2026-10-31: Name, builds and licence
- Name: Encore (a moment played again).
- Builds: GitHub Actions on Windows produces the installer; nobody needs a local toolchain to test.
  CI cannot test capture (no GPU/monitor/audio), so each batch ends with a hands-on test.
- Licence: MIT for Encore's own code.
- FFmpeg: use an LGPL build (we only need hardware encoders, not GPL-only ones like x264), run as a
  separate program, with its licence text and source link shipped in THIRD_PARTY_NOTICES.md.
- Build notes, commits and docs do not name individuals or the tools used to write the code.

## 2026-10-31: Mode toggle and minimised windows (defaults, can be revisited after testing)
- Toggle has three positions: Auto (default) / Game / Desktop. Auto records the game if one is
  detected, otherwise the desktop.
- Minimised game window: black frames for that stretch. Freezing the last frame could suggest the
  game was still being played.

## 2026-10-31: Batch 1 findings
- Segment files must reset their timestamps (`-reset_timestamps 1`). Keeping the originals made a
  7-second test recording join into a 342-second file full of gaps. With the reset: 7.03 s, 60 fps,
  no gaps across joins, decodes cleanly.
- Crash recovery tested: leftover segments are joined into a "Recovered" file on the next start.
- The Windows build machine found an LGPL FFmpeg 8.1+ build with `gfxcapture` (BtbN).
- "Show mouse cursor", quality and resolution settings apply per recording; locked while recording.
- The source picker offers Desktop or One window for now. The Auto / Game / Desktop toggle needs
  game detection and arrives in batch 3.

## 2026-10-31: Bitrate kept as is
First real test (2560x1440, 120 fps, H.264, High) looked excellent. The High preset targets about
133 Mbps at those settings (about 1 GB per minute of fast gameplay); file size is acceptable for now.
Later: a Custom bitrate option and an expected-size-per-minute readout beside Quality.

## 2026-10-31: Batch 2 (audio) design and findings
- Each source (full mix, mic, each app) captured on its own thread via WASAPI, written to its own
  file and placed by packet timestamp. App tracks are found by watching active audio sessions every
  half second and grouped by app (Chrome's helper processes count as Chrome).
- Sync by shared clock (see ARCHITECTURE.md). End-to-end test: a flash at 2.000 s in the video and
  a beep in the audio files offset by 0.35 s came out at 2.000 s and 2.000 s in the saved file.
- Found in that test: B-frames shifted video 33 ms late after joining segments (constant, not
  cumulative). B-frames are now off in every encoder; timing exact afterwards.
- MP4 keeps each track's name in `handler_name`, not `title`. The companion editor reads either.
- Mic is recorded by default; it is its own track, so it can be muted when editing.
- More than 8 apps: extras are only in the Full mix (no "Other" track for now).

## 2026-10-31: App icon
Dark tile with a glowing blue-white E over a faint waveform, matching the companion editor's
icon style (dark background, blue accents, glowing bars). The waveform is tallest in the middle,
curves down and fades outwards, then rises back slightly near the edges. Generated by
`scripts/make-icon.py`, which holds every parameter, so it can be adjusted and regenerated.

## 2026-10-31: Automatic updates
Tauri's updater, checking `releases/latest/download/latest.json` on GitHub 10 seconds after start
and every 4 hours. A newer version downloads quietly; a green "Update ready" button installs it and
restarts Encore (refused while recording; the engine saves any recording before the installer runs).
Updates are signed with Encore's own key: the public half is in tauri.conf.json, the private half is
the TAURI_SIGNING_PRIVATE_KEY repository secret (never committed). Builds without the secret still
work but can't be delivered as updates. Test releases are now published as normal "latest" releases,
because GitHub's "latest" link skips pre-releases.

## 2026-10-31: Updates are the person's choice (supersedes "Automatic updates" above)
No background downloads unless chosen. Settings → Updates offers: "Tell me when there's an update"
(default: a quiet check, nothing downloads until they click), "Download updates automatically"
(they still choose when to install), and "Never check for updates (not recommended)".
"Check for updates" (≡ menu and Settings) works in every mode. Stored as `update_mode` in the
engine's settings file; the shell reads it before each check.

## 2026-10-31: Layout matched to the companion editor
≡ menu top-left (Settings, Check for updates, Changelog, About Encore); "What to record" panel on the
left, collapsible to an icon rail; Settings, Changelog and About open as dialogs. Measurements and
colours follow the editor's (title bar, menu, dialog, radio options, tinted "Update" button); the
code is Encore's own. Versions stay 0.x and the app is labelled Beta until 1.0; the changelog
(CHANGELOG.md and ui/changelog.js) starts at 1.0 and is empty until then.

## 2026-10-31: Design rule, identical to the editor
Encore and the companion clip editor are made as a pair, so Encore must look exactly like it,
not just similar. Recorded as a hard rule in DEVELOPMENT.md. First known gap: the editor's custom
merged title bar (backlogged, not started yet at the owner's request).

## 2026-10-31: First real audio test (0.3.0)
Desktop recording, 2560x1440 at 120 fps, Spotify and YouTube (Edge) playing, mic on. File had the
expected tracks; per-app separation and mic isolation confirmed by measurement; inter-track offset
1.5–4.2 ms, constant. Issues were in playback: VLC plays Full mix (no mic, by design), and the
companion editor doesn't yet read per-app tracks. See FEATURES.md "Found in testing".

## 2026-10-31: Full mix includes the mic; Desktop audio track added
Players and Discord use the first track, so it is now everything, mic included. System sound
without the mic is a new "Desktop audio" track (only when a mic is recorded). Mixing happens when
the file is saved, at full level (no automatic lowering).

## 2026-10-31: File format marker bumped to rec-format:2 (corrects the entry above)
The entry above kept `rec-format:1`, which was wrong: files from 0.3.x carry the same marker but
their Full mix has no mic, so the editor could not tell the two layouts apart and an untouched
export of an old file would lose the voice. Raised by the companion editor's side. From 0.4.1
files are `rec-format:2`; the editor treats `:1` files as "Full mix + Microphone".

## 2026-10-31: Own title bar and audit against the editor (0.4.0)
The window has no Windows frame; Encore draws the title bar and the minimise / maximise / close
buttons, sized and coloured like the editor's (46×40, Windows' own glyph font, the editor's
colours). Side-by-side audit fixed every difference found (FEATURES.md "Match the editor
exactly"). Known gap: snap layouts on the Maximise button.

## 2026-10-31: Desktop recording follows the screen you click on
"Monitor with the mouse" only picked a monitor when recording started; switching mid-recording
was never built. Decided: Desktop recording switches to the monitor of the window you click on
(the focused window), not where the mouse is, so glancing at a second screen doesn't move the
recording. One behaviour, no setting. Built with the layout redesign (monitor picker under the
record button, settings and this session's recordings in the left panel).
Built in 0.5: overlapping FFmpeg runs, joined at the measured takeover time (ARCHITECTURE.md).
End-to-end test: cut lands at exactly 3.500 s, flashes and a mic beep stay on their frames.
Clicking a monitor in the picker pins it ("Always"); clicking it again goes back to following.

## 2026-10-31: Layout A chosen
From three mock-ups: settings and "Saved this session" in the left panel; under the record button,
the monitors as holograms laid out as in Windows, then "One app" and "Game" tiles. The monitor
being recorded glows red. "Game" shows "Coming soon" until game detection is built.

## 2026-10-31: Window size matches the editor
Opens at 1500x920 (shrunk to fit smaller screens), minimum 1100x700 (was 980x640), and reopens at the size and
place it was closed at, as the editor does. The monitor picker stays on one line at any size.

## 2026-10-31: In-game stutter, save folder, audio size (0.5.2)
- Stutter: three clips showed only 0–10 new frames a second while the game was in front; Task
  Manager showed the game at 90.8 % GPU. The capture (inside FFmpeg) was being starved. FFmpeg now
  gets raised GPU scheduling priority (realtime when Encore runs as administrator, otherwise high)
  and above-normal CPU priority, as other recorders do. Capping the game's frame rate also helps.
- Save folder: recordings go in `<chosen folder>\Encore\<category>\`. The default chosen folder
  becomes Videos, so the default location is unchanged. A folder already named Encore is used as it
  is. The folder is checked for write access when the setting changes.
- Audio size: measured at 1–3 % of the file, so the file size is the video. Extra tracks drop to
  128 kbps (Full mix stays 192). Bigger savings: custom bitrate, HEVC/AV1.
- Apps whose details can't be read are named after their program file instead of "App <number>".

## 2026-10-31: First-run setup and the real Videos folder (0.5.3)
- Setup on first start (three steps: save folder, video, sound) in the editor's small-dialog style.
  Closing it counts as done; Help → Run Setup Again reopens it. `setup_done` in settings; files
  saved before it existed show it once.
- Tests: Warframe capped at 30 fps captured every frame (30 new a second); capped at 120 with no
  V-Sync, the capture froze for 9 s. Confirms GPU starvation; the setup's video step tells people
  that capping the game's frame rate keeps recordings smooth.
- Default save folder is Windows' real Videos folder (known-folder lookup), not
  %USERPROFILE%\Videos, which may not exist when Videos was moved. A setting still on an old guessed
  default is moved to it once, before setup is done.

## 2026-10-31: Folder check creates nothing; menus above dialogs (0.5.4)
Picking a save folder only tests a write in the nearest existing folder and removes the test file;
the Encore folder is made when setup finishes or at the first recording. Menus sit above every
dialog (dropdowns in the setup opened behind it). The top bar names the encoder actually used
(NVIDIA, then AMD, then Intel), since PCs with a processor's built-in graphics list two.

## 2026-11-01: Feature name; Discord voice split dropped
The per-app audio feature is called **Split App Tracks**. Splitting Discord voice chat into a
track per person is dropped at the companion editor's request (none of the available methods
suited it). Discord user ID stays in the backlog until its purpose is confirmed.

## 2026-11-01: Confirmed on the editor developer's PC (0.5.4)
Three monitors shown in the right order and sizes, picker on one line with five tiles, and the
real Videos folder found on another drive (F:\Videos\Encore).

## 2026-11-01: Tray, hotkeys and sounds (0.6.0)
- Closing the window keeps Encore in the tray by default (like other game recorders), so hotkeys
  and later instant replay keep working; a setting switches it to quitting. Exit and the tray's
  Quit always quit. One Windows notification the first time it hides.
- Default record hotkey Ctrl+Alt+R: clear of NVIDIA's Alt+F9/F10 and overlay keys. Editable in
  Settings → Hotkeys (press the keys); plain letters and digits need a modifier.
- Short start/stop sounds (on by default) confirm a hotkey worked without looking.
- Single instance: launching Encore again brings the running one back.

## 2026-11-01: Instant replay design (0.7.0)
- One capture at a time: with replay on, a recording is cut from the replay capture (Start to
  Stop) instead of starting a second capture, which would double the load on the graphics card.
- Audio is kept in 10-second pieces so old audio can be deleted while capturing; video uses the
  existing 2-second segments. Saves start at a keyframe (up to 2 s early) and need no re-encoding.
- Saving runs on a worker thread; the buffer never stops. End-to-end test: a 5.3–11 s window over
  a monitor switch came out 7.0 s long, switching at 3.0 s, flash and beep together at 5.0 s.
- Default length 2 minutes; save hotkey Ctrl+Alt+S; the side panel shows the disk it uses.
- Replay stays on next time if it was on when Encore closed.

## Open items
- Confirm zero-copy GPU capture works on real NVIDIA/AMD/Intel cards (fallback path exists if not).
- Test exclusive-fullscreen games for black capture; define the fallback.
- Default hotkeys: pick ones that do not clash with NVIDIA ShadowPlay (Alt+F9 / Alt+F10).
- Real-world sync accuracy of the progress-report method: confirm with a clap test (batch 2).
- Switching the default output device mid-recording is not handled yet.
- Installer is not code-signed: Windows SmartScreen will warn on first run.

## Performance log
Record idle RAM, CPU while recording, and game FPS (with and without recording) after batches 1, 2 and 3.

Test PCs (from DxDiag, 2026-11-01):

| PC | Graphics | Processor | RAM | Monitors (in use) | Windows |
|---|---|---|---|---|---|
| Owner's | NVIDIA RTX 3060 Ti, 8 GB | Intel Core i9-9900K | 32 GB | ED323QUR 2560×1440 at 75 Hz; LS32AG55x 2560×1440 at 120 Hz (supports 165 Hz) | 11 Pro 23H2 (22631) |
| Companion editor developer's | NVIDIA RTX 5080, 16 GB (+ AMD Radeon graphics in the processor) | AMD Ryzen 7 9800X3D | 64 GB | AORUS FI27Q-P 2560×1440 at 165 Hz; ZOWIE XL 1920×1080 at 120 Hz; ZOWIE RL 1920×1080 at 60 Hz | 11 Pro (26200) |

Both have hardware-accelerated GPU scheduling on.

| Date | Batch | Game / test | Idle RAM | CPU recording | FPS off / on | Notes |
|---|---|---|---|---|---|---|
| 2026-10-31 | 0.5.1 | Warframe, 1440p, owner's PC | | | | Capture starved while the game is in front: V-Sync on, no cap → 0–10 new frames/s; capped 30 → every frame captured; capped 120, no V-Sync → froze ~9 s. Game at 90.8 % GPU. |
| 2026-10-31 | 0.4.x | Deadlock, 1440p 60 fps, editor developer's PC | | | | Smooth: ~47 new frames/s (the game's own rate) |

## 2026-11-01: Graphics priority needs administrator (0.7.1)
- The logs showed "GPU priority refused" on every capture: without administrator rights Windows
  refuses both the realtime and the high level, so the 0.5.2 stutter fix never applied. The earlier
  note that high works without admin was wrong.
- Administrators hold the right to raise priority, but it starts switched off. Encore now switches
  it on before asking. The log records whether Encore is running as administrator, whether the
  right is on, and the error code for each refused level.
- When priority is refused, Encore says so once per run instead of only logging it.
- ≡ → File → Restart as Administrator closes Encore and opens it again elevated (Windows asks
  first; if the answer is no, it opens normally). The new copy waits for the old one to finish
  saving and exit. Hidden when already elevated.
- Setting "Run as administrator" in the program's Properties doesn't apply when the updater or
  the tray copy starts Encore, so the in-app restart is the reliable route for testing.

## 2026-11-01: Graphics priority is set once FFmpeg has opened the card (0.7.2)
- Run as administrator with the right switched on, Windows still refused both levels with
  0xC000000D (invalid parameter). Encore asked the moment FFmpeg started, before FFmpeg had opened
  the graphics card; Windows has nothing to attach a graphics priority to until then. Other
  recorders set it after creating their graphics device, which fits.
- Encore now raises CPU priority at once and retries the graphics priority every 0.2 s for up to
  10 s, logging how long it took. The warning only shows if every try fails.
- With Windows' hardware-accelerated GPU scheduling on, Encore asks for high rather than realtime,
  as realtime can make the whole system stutter in that mode.

## 2026-11-01: Stutter fixed; starting as administrator without prompts (0.7.3)
- 0.7.2 test (Warframe capped at 120, run as administrator): all 24 seconds had 60 of 60 new frames.
  Before, the same test dropped to 0–6 a second. Graphics priority, set once FFmpeg has opened the
  card, is the fix; it needs administrator rights.
- "Start Encore as administrator" uses a scheduled task approved once, rather than asking on every
  launch (the "Run as administrator" box) or relaunching with a prompt each time. Setup offers it,
  on by default for new installs, with one line on why. Off removes the task.
- Update checks retry once after 3 s; the error says GitHub couldn't be reached instead of showing
  the raw network error.

## 2026-11-01: Notes for game detection (League of Legends)
- League runs as two programs. The client (LeagueClient.exe with LeagueClientUx.exe and
  LeagueClientUxRender.exe: lobby, champion select, end-of-game screen) stays open, and each match
  starts a separate program, League of Legends.exe, in its own window. When the match ends that
  window closes and the client comes back. (Seen in the logs as "League of Legends" and
  "League of Legends (TM) Client", then "… Client 2" when a second game process started.)
- So: the match window is the game; the client is a launcher, not a game. Both should save under
  one folder, "League of Legends". When the match window appears, capture and instant replay
  should move to it on their own, and move back (or stop) when it closes. Several games use this
  launcher-then-game pattern, so detection should go by the game window, not the first program
  that opened.
- Hotkeys didn't work in a League match. Other recorders report the same, fixed by running as
  administrator (the match most likely runs with raised rights, which blocks keys from a non-admin
  app). To confirm with "Start Encore as administrator" on.

## 2026-11-01: Save-length hotkeys (0.7.4)
- As well as Save replay (the whole replay length), three hotkeys save just the last 30 seconds
  (Ctrl+Alt+1), the last minute (Ctrl+Alt+2) or a custom length (Ctrl+Alt+3, default 90 seconds,
  5 seconds to 20 minutes, set in seconds or minutes). A save never goes past the replay length;
  Settings says so when the custom length is longer. All four save from the same buffer.

## 2026-11-01: Sound choices (0.7.5)
- Settings → Sounds: a sound and a volume for recording started, recording stopped and replay
  saved, with Play to hear it, plus the existing on/off switch (moved here from General). Eight
  sounds plus "No sound", made in the interface with Web Audio, so there are no sound files to
  license. Volume 50 is the level the old fixed sounds used.
- Known: the sounds come out of Encore's own window, so the desktop-audio track (and an
  "Encore"/WebView2 app track) can pick them up. To do: leave Encore's own audio out of recordings
  (Windows can capture "everything except this app").

## 2026-11-01: Hotkeys read the key state (0.7.6)
- Hotkeys still didn't work in a League match with Encore running as administrator, so the rights
  weren't the cause. Windows' RegisterHotKey message never arrives while some games read the
  keyboard directly. The shell now checks the key state every 10 ms instead (the approach other
  recorders use), which works whatever is in front. Trade-off: the keys also reach the game (they
  were swallowed before), so Settings says to pick keys the game doesn't use. A combination set
  for two actions is reported. Every press is written to the engine log.
- While a new hotkey is being chosen in Settings, presses don't trigger actions.
- Replay messages now give the length actually saved ("Saving the last 30 seconds", "Saved replay
  (30 seconds)"), not the replay length.

## 2026-11-01: Encore's own sounds left out of recordings (0.7.7)
- Desktop audio is now captured as "everything except Encore" (Windows process loopback in
  exclude mode, aimed at the Encore app and everything it started), so the start/stop/save sounds
  aren't recorded. On a Windows too old for that, it falls back to recording the whole output
  device, as before, and says so in the log.
- Split App Tracks skips every process Encore started (its window's WebView2 processes included),
  so there's no "Microsoft Edge WebView2" track any more.

## 2026-11-01: Ultra quality, working folder, start with Windows, settings contents (0.8.0)
- Quality: the old High (0.30 bits per pixel) is now **Ultra** and stays the default. A new
  **High** (0.24) sits between it and Medium (0.18). Settings files from before are moved over
  once (`settings_version` 1): a saved "high" becomes "ultra", so nobody's quality changes.
- Defaults stay as they are; people tune to their own PCs.
- Working folder (`work_dir`, Settings → Saving): where recordings in progress and the instant
  replay buffer go. Empty = %LOCALAPPDATA%\Encore\temp. A chosen folder gets an "Encore temp"
  folder inside. Leftovers from a crash are recovered from both. Can't change while recording;
  instant replay restarts in the new place.
- Start with Windows: a value in the current user's Run key starting Encore with `--autostart`,
  which keeps the window hidden (tray only). With "Start as administrator" on, the hand-over to the
  elevated copy passes this on through a short-lived marker file.
- Settings has a contents list on the left: click a section to jump to it; the section in view is
  highlighted. Hidden on narrow windows.

## 2026-11-01: Game detection plan, and the picker changes that come first (0.8.1)
Agreed design for game detection (to build next):
- Game detection is its own on/off switch, separate from the monitor choice. With no game
  running, Encore records the chosen monitor (or follows clicks), shown in green to say detection
  is still on. A game opening takes over; when all games close it goes back to the fallback.
- League: only the match window counts; the client never does, so between matches it's the
  fallback monitor.
- Several games: newest wins by default (setting: "Newest game" or "The game I'm playing").
  "Record this game" hotkey (Ctrl+Alt+G), clicking a game in the Game tile, or the tray menu locks
  onto one; the lock holds while other games open and is released when that game closes.
  "Next game" hotkey (Ctrl+Alt+N) moves between open games.
- Select Application is separate; picking it pauses game detection; picking a screen resumes it.
- Games are captured as their window only (Alt+Tab and the desktop never appear). If a game in
  exclusive full screen comes out black, Encore says to use borderless.
- A manual recording when the game closes either carries on with the fallback or stops and saves
  (setting).
- Instant replay stays one global switch; "Keep replay on the desktop" off means it only buffers
  while a game runs.
- Detection: Always/Never lists, known launchers/clients/browsers excluded, game-library folders,
  then full screen with heavy graphics use as the last guess. Names from Steam's records where
  possible; "(TM) Client" style endings cleaned.
- After that: Encore's own notices (never captured, hologram position picker, Do not disturb in
  the main panel), designed together first.

0.8.1 (first step):
- "Follow" is its own tile in the picker (the screen you click on). Picking a monitor always
  records it (no more click-again-to-follow or "ALWAYS" badge); when following, the screen in use
  has a lighter highlight and "In use".
- "One app" is renamed "Select Application".
- On narrow windows the picker tiles shrink so it stays on one line.
- 0.8.2: the record button sits higher (top of the stage, not centred), and Select Application
  and Game moved to a second row under the screens. The picker shrinks to fit the window's height
  as well as its width.
- 0.8.3: the top area has three parts. Left, **Switches** (Instant replay, moved from the side
  panel; Game detection and Do not disturb shown greyed until they arrive). Middle, a plain round
  record button (dot, or square while recording) with the timer and "Start recording · hotkey"
  under it. Right, **Status**: one line per feature with a coloured light (Standing by /
  Recording / Saving; Following you / Locked to screen / Application; Instant replay; audio tracks
  while recording; Saving to). A divider that fades at both ends separates it from What to record.
- 0.8.4: the monitors are centred on the page, with Follow just to their left (it no longer
  pulls them off centre). The Game tile is gone: Game detection is the switch at the top, and
  when it finds games they'll appear as tiles next to Select Application, to see and lock.
- Layout (with the game detection build): Instant replay's controls moved to the top-left panel;
  Do not disturb is a single button at the bottom of the side panel (moon glows soft red when
  on); Game detection is a bar under the screens whose controller icon is the button (glows soft
  purple), with the games it finds as chips beside it. Select Application sits right of the
  monitors, mirroring Follow. Status lines are one line each. In small windows Follow and Select
  Application shrink before the monitors do.
- Minimum window size 1100x700: the smallest at which everything fits without scrolling with three
  monitors, a recording running and two games found (checked in a browser at each size).
- Side panel audio (with the game detection build): Video and Audio are split by a line. Each audio
  option has an icon whose lines glow: Full System Audio (was "Desktop audio"; PC and monitor)
  and Split Application Audio (was "Split App Tracks"; mixer sliders) glow blue when on and soft
  red when off; Microphone glows green when on and red, with a line through it, when off. Split
  Application Audio has an (i) explaining it on hover. The icons are the switches (no sliders):
  click the icon to turn the option on or off. Other switches use the monitors' lighter blue.
  Only the labels changed: the tracks in the files keep their names ("Desktop audio" etc.), so the
  editor's handling is unchanged.
- **Full System Audio** keeps its name and gets an (i): all sound in one track including Windows'
  own; off keeps only the apps (with Split Application Audio) or, with all three off, gives a
  silent recording. ("Everything You Hear" was tried and dropped: the explanation does the job.)

## 2026-11-01: Health check (old names and leftovers)
- Interface names now: Follow, Select Application, Game detection, Full System Audio,
  Microphone, Split Application Audio, Do not disturb. Old names ("One app", "Split App Tracks",
  "Desktop audio" as a label, the "ALWAYS" pin, the Game tile) are gone from
  the interface, README, FEATURES and ARCHITECTURE; DECISIONS keeps them as history. `Desktop
  audio` stays as a track title inside files (file contract with the editor).
- Removed leftover styles (old second picker row, separator, "coming soon" switch, unused helper
  classes) and an unused bridge call. Every setting the interface uses exists in the engine, and
  every engine setting is used. README's feature list brought up to date.

## 2026-11-01: Game detection built (0.9.0)
- As agreed above. Details in ARCHITECTURE.md ("Game detection").
- A closing game ends its capture a moment before the screen's capture takes over. Rather than
  shift everything after it, the joined video holds the game's last frame for that moment, so
  audio stays in sync (checked with FFmpeg: a 4 s part handing over at 5 s gives a 9 s file with
  the next part starting at 5.0 s).
- The output size is set once at the start (from the screen), so a windowed game is scaled to fit
  rather than changing the file's size part-way.
- League of Legends, VALORANT and a few others are recognised by program name, because their
  install folder may not be readable while they run.

## 2026-11-01: Instant replay is now called Echoes (0.9.1)
- The feature is **Echoes**; saving one is **Save an Echo**; a saved clip is **an Echo** (tagged
  "Echo" under Saved this session). Hotkeys: "Save an Echo", "Save an Echo: last 30 seconds" etc.
  Status: "Echoes: keeping the last 2 minutes" / "waiting for a game". Tray: "Save an Echo".
  Internally (code, settings keys, temp folders) it is still "replay"; nothing changes on disk.
- Calmer panel (top left): the title with a glowing icon button (blue when on), one line saying
  what it's doing, "Keeps the last [length]", and "Save an Echo" with its hotkey inside the button.
  Disk use moved to the length's tooltip.
- "Also when no game is open" moved to Settings → Games as "Only keep Echoes while a game is
  running" (the opposite way round, so it says what it limits); same setting underneath.
- First-run setup gets a fourth step introducing Echoes ("Never miss a moment"), with a switch
  (on) and the length.
- 0.9.2: while Echoes restarts for a settings change, its status line and panel turn amber
  ("Restarting with your new settings"), and the icon stays on. Do not disturb is no longer in the
  status panel (its button is always visible); it may come back in a small always-on-top remote.
- 0.9.3: first-run setup is a short tour, 7 steps with progress dashes: Welcome (save folder),
  Video, Sound, What to record (Follow / a monitor / Select Application), Game detection (switch,
  on for new installs), Echoes (switch and length), You're all set (the main hotkeys, the tray,
  Do not disturb, and a Start with Windows switch). Every page fits the minimum window. Settings →
  Games wording simplified (no bracketed asides; "The game I'm playing" explained plainly).

## 2026-11-01: PC freeze while recording Warframe; Lightshot taken for a game (0.9.4)
- Report: Warframe recorded by game detection (realtime graphics priority, GPU scheduling off),
  an Echo saved, the editor opened, and about a minute later the whole PC froze. The log just stops,
  so the cause can't be proven, but realtime graphics priority can let the capture crowd out
  everything else, including the desktop. Encore now asks for **high** only (never realtime). To
  confirm in testing that games stay smooth with high.
- The log also showed realtime priority being granted without administrator rights, so 0.7.1's
  refusal was only the timing problem fixed in 0.7.2.
- Lightshot (a screenshot tool) was detected as a game: it covers the screen while you pick an
  area. Screenshot and screen tools are now on the not-a-game list, and a window that only looks
  like a game because it fills the screen must stay 4 seconds before it counts.
- The "Couldn't switch…" warning came from those Lightshot switches. Failed switches now wait
  before trying that target again (10 s, doubling, up to 2 minutes), so a window that can't be
  captured can't set off a stream of FFmpeg starts; the warning is plain ("Couldn't start
  recording Warframe, so Encore carried on…") and shown once; FFmpeg's full reason goes to the log.
- A recording made while Echoes is on picks its file name when saved (it could, rarely, take the
  name of an Echo saved in the same second and overwrite it).

## 2026-11-01: Making "What to record" obvious, and a games-only choice (0.9.5)
- Worry: someone who just wants Echoes may think they have to set up a monitor. So: a line under
  "What to record" ("Used for recordings and Echoes. Leave it on Follow…"), Follow labelled
  "Recommended", and the Echoes panel says what it's keeping ("…the last 2 minutes of Warframe").
- For people who only want games recorded: the game detection bar has "Until a game opens:
  Record my screen / Wait for a game". Wait for a game sets `replay_on_desktop` off (Echoes waits)
  and `game_end` to stop (a recording ends with its game); the screens dim and the line above them
  says they're only used if you press record with no game open. Also offered in setup ("Only
  record games"). The Settings → Games options stay as the fine-grained version.
- Background on the earlier freeze (for later): the PC had the same kind of crash before Encore
  existed: 27/10 LiveKernelEvent 117 and 141, then BlueScreen 116 (VIDEO_TDR_FAILURE, insufficient
  resources) on the RTX 3060 Ti. Suggested a clean driver install, removing any GPU overclock,
  checking temperatures and power cables. If a freeze happens on 0.9.4+ with no hardware error
  logged, look at Encore again.

## 2026-11-01: Desktop Capture (0.9.6)
- "Until a game opens: Record my screen / Wait for a game" was a mode hidden inside the game bar.
  It's now **Desktop Capture**, an icon switch heading the screens section (where "What to
  record" was). "Manual mode" was considered and dropped: you can still press record by hand with
  it off. Setting `desktop_capture` (reads the old `replay_on_desktop`).
- Off (games only, needs game detection): screens and Select Application dimmed and disabled;
  Echoes waits for a game; a recording always ends with its game; record is greyed out with no
  game open ("Start a game to record (or turn on Desktop Capture)"), and the engine refuses it
  too; status "Desktop Capture: Off · waiting for a game".
- Game detection off forces Desktop Capture on (both off would record nothing), in the engine
  and the switch's tooltip.
- The Settings option "Only keep Echoes while a game is running" is gone (same thing); "when the
  game closes" now only applies with Desktop Capture on. Setup's switch is "Desktop Capture".
- Every glowing icon switch now sits on a faint tile (a tinted one when on), so it reads as a
  button: audio, Echoes, Desktop Capture, game detection, Do not disturb.
