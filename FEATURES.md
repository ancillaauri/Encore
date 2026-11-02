# FEATURES.md

Agreed feature backlog, slotted into the build steps. Status: [ ] not started, [~] built, awaiting hands-on test, [x] done and tested.
Out-of-scope items are listed at the bottom with the reason, so they are not re-proposed.

## Core (agreed 2026-10-31)

| Feature | Build step | Status |
|---|---|---|
| Manual recording (start/stop) | 3 | [x] |
| Capture modes: Game window / Desktop (toggle style TBC) | 3, 6 | [ ] |
| Monitor selection for Desktop mode: follow the screen you click on (default) or always one monitor | 3 | [x] 0.5 |
| "What's being recorded" indicator (tray + window) | 6 | [ ] |
| Codec choice: H.264 default, HEVC and AV1 optional | 3 | [~] |
| Quality settings: resolution (native/downscaled), frame rate up to 144, quality/bitrate presets | 3 | [~] |
| Full System Audio (all PC sound, minus Encore's own) + microphone tracks | 4 | [x] |
| Microphone choice (push-to-talk comes with hotkeys) | 4 | [~] |
| Split Application Audio: one audio track per app (cap 8; extras only in Full mix; always/never lists later) | 5 | [x] |
| Global hotkeys (start/stop recording; more with replay), tray icon, close to tray, sounds | 6 | [~] 0.6.0 |
| Game detection: games recorded on their own (window only), green fallback screen, lock/next game (click, tray, Ctrl+Alt+G / Ctrl+Alt+N), newest or focused game, always/never lists, names from Steam/library folders, saves named after the game | 6 | [x] 0.9 |
| Recording moves to the monitor you click on, mid-recording, with no gap | 6 | [x] 0.5 |
| Switch Game ↔ Desktop mid-recording (reuses the same mechanism) | 6 | [ ] |
| Pause / resume manual recording | 6 | [ ] |
| Screenshot hotkey | 6 | [ ] |
| "Saved" feedback: sound + Windows notification | 6 | [ ] |
| Echoes (instant replay) (rolling buffer, 30 s – 20 min, save hotkey Ctrl+Alt+S, tray item, recording while on shares the capture) | 7 | [~] 0.7.0 |
| Multiple save hotkeys (e.g. last 30 s / 2 min / full buffer) | 7 | [ ] |
| Auto-start replay when a game is detected, stop when it closes | 7 | [ ] |
| Storage: estimated disk use beside replay length [~] 0.7.0; max library size + auto-delete oldest, low-disk warning [ ] | 7 | [~] |
| Installer + updates (check, download and install from the app; user chooses the update mode) | 8 | [~] |
| Settings dialog, ≡ menu, side panel (quick settings, saved this session), About, changelog skeleton, first-run setup | 6 | [x] |
| Companion editor reads our per-app tracks (change on the editor side) | 9 | [ ] |

## Found in testing (0.3.0 desktop test, 2026-10-31)

| Issue | Finding | Status |
|---|---|---|
| Raw file in VLC has no mic | First track "Full mix" now includes the mic; new "Desktop audio" track has system sound without it (0.4.0). | [x] |
| Companion editor shows only "Game / Desktop" + "Microphone", audio doubled | Encore's file was correct (Full mix, Microphone, Spotify, Microsoft Edge). The editor doesn't read per-app tracks yet and lumps Full mix + app tracks together. Needs the "Companion editor change" in ARCHITECTURE.md (editor side). | [ ] |
| Mic track "had everything" in the editor | Measured: Encore's Microphone track contains only the voice (no correlation with any app audio). Cause is the editor's lumping/preview; fixed by the same editor change. | [ ] |
| Per-app split | Confirmed working: Spotify and Edge on separate tracks, fully separated; all tracks within 5 ms of each other, no drift over 63 s. | [x] |

## Found in testing (0.5.1, 2026-10-31)

| Issue | Finding | Status |
|---|---|---|
| Game recordings choppy while playing | Measured on three clips: while the game is in front, only 0–10 of every 60 frames are new (the rest repeat). Smooth in Steam and other windows, and smooth again when the game is in the background. The other tester's clip (Deadlock, same Encore) is smooth. Points to the game using all of the GPU so the capture gets starved, as seen with other recorders using the same Windows capture. Confirming with two tests (game FPS capped; Encore run as administrator), then: raise Encore's GPU priority while recording. Confirmed: Task Manager showed the game at 90.8 % GPU (93 % total). 0.5.2 raises FFmpeg's GPU priority (realtime with admin rights, else high) and CPU priority while recording. To verify in game. 0.7.1: the log showed priority was refused without admin rights, so the fix never applied. Encore now switches on the right it needs, logs why priority is refused, warns in the app, and can restart itself as administrator. 0.7.2 retries until FFmpeg has opened the card: smooth (60/60 new frames every second, Warframe at 120). 0.7.3: "Start Encore as administrator", approved once. | [x] |
| Save folder layout | The chosen folder gets an `Encore` folder made inside it, with a folder per category: e.g. `D:\Program Files\` → `D:\Program Files\Encore\Desktop\`, `...\Encore\Warframe\`. Say so next to the setting. | [x] 0.5.2 |
| File size with many audio tracks | Measured: all audio tracks together are about 0.7 Mbps of a 25–66 Mbps file (1–3 %). Size comes from the video bitrate. Options: smaller app tracks (silent ones already cost almost nothing), HEVC/AV1, custom bitrate with a size-per-minute readout. 0.5.2: extra tracks at 128 kbps. Bigger savings need the video options. | [~] |
| App track named "App 24720" | An app whose program details couldn't be read (likely a protected process) got a placeholder name. Fall back to its program file name, which is always readable. | [x] 0.5.2 |

## Match the editor exactly (design rule; next UI pass)

| Difference | Status |
|---|---|
| Custom title bar merged into the dark interface (own minimise/maximise/close, drag area), like the editor's | [x] 0.4.0 |
| Audit every screen side by side with the editor (0.4.0): font, icon stroke, scrollbars, tooltips, File/View/Help menu with shortcuts, dropdown buttons instead of Windows dropdowns, chips, buttons, dialogs and their fades, toasts, panel width and collapsed rail | [x] |
| Windows 11 snap layouts when hovering Maximise (the editor gets them from Windows; Encore draws its own buttons). Win+Z still works | [ ] |

## Later (agreed, after core works)

| Feature | Notes | Status |
|---|---|---|
| Bookmarks: hotkey marks a moment, stored as MP4 chapters | Needs the companion editor to display them; agree format together (step 9+) | [ ] |
| Full-session recording: auto-record whole match/session from game launch | Pairs with bookmarks | [ ] |
| Microphone noise suppression (optional) | Raw mic track kept untouched | [ ] |
| Custom bitrate option + expected size per minute beside Quality | Presets stay as the default | [ ] |
| Apps on a non-default sound device (e.g. Discord on a headset) get their own track | Today only apps on the default output are found | [ ] |
| Discord: your user ID via Discord's local connection (hello message only; no login, nothing private) | Purpose to confirm with the companion editor side; needs a free Discord developer app | [ ] |
| Lower suggested frame rate in setup for weaker graphics cards | Uses the PC specs collected for the performance log | [ ] |
| Per-app audio feature named **Split Application Audio** (UI and docs) | | [x] |
| Notifications over the game, never recorded: hologram card or edge tab by spot, 2 to 10 s timer, up to 3 stacked, size 1× to 2× | 0.9.8; drawn by Encore on the processor from 0.9.15; not over exclusive fullscreen games | [x] |
| Priority Capture: realtime graphics priority for the capture, switch in the side panel and Settings → Advanced, one-time "I understand" | 0.9.10 (as Top graphics priority), 0.9.17 | [x] |
| Warning when the capture falls behind (30 s, or more than 10 s behind) | 0.9.10, 0.9.12 | [x] |
| Main Menu (Settings at top level), Sidebar naming, Priority Capture hideable from the sidebar | 0.9.21 | [x] |
| Microphone found by name when its id changes; warning icon and menu when it isn't plugged in | 0.9.23 | [x] |
| Clicking a clip: folder, video player or Clip Editor (Locate / Browse) | 0.9.24 | [x] |

## Out of scope (decided, with reasons)

- HDR recording: not wanted (2026-10-31).
- Automatic kill/win clipping: needs per-game integrations or AI vision across many titles; too big for a two-person project.
- In-game overlay: needs injection into the game, which risks anti-cheat bans. Sound + Windows notification instead.
- Cloud hosting / share links / social feed: server costs, accounts, moderation; a different product.
- Built-in streaming (Twitch etc.): OBS covers it.
- Editing effects (slow motion, captions, music): the companion editor's job.
