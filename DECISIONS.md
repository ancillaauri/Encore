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

## Open items
- Confirm zero-copy GPU capture works on real NVIDIA/AMD/Intel cards (fallback path exists if not).
- Test exclusive-fullscreen games for black capture; define the fallback.
- Default hotkeys: pick ones that do not clash with NVIDIA ShadowPlay (Alt+F9 / Alt+F10).
- Real-world sync accuracy of the progress-report method: confirm with a clap test (batch 2).
- Switching the default output device mid-recording is not handled yet.
- Installer is not code-signed: Windows SmartScreen will warn on first run.

## Performance log
Record idle RAM, CPU while recording, and game FPS (with and without recording) after batches 1, 2 and 3.

| Date | Batch | Game / test | Idle RAM | CPU recording | FPS off / on | Notes |
|---|---|---|---|---|---|---|
