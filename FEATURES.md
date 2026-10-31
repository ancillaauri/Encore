# FEATURES.md

Agreed feature backlog, slotted into the build steps. Status: [ ] not started, [~] in progress, [x] done and tested.
Out-of-scope items are listed at the bottom with the reason, so they are not re-proposed.

## Core (agreed 2026-10-31)

| Feature | Build step | Status |
|---|---|---|
| Manual recording (start/stop) | 3 | [ ] |
| Capture modes: Game window / Desktop (toggle style TBC) | 3, 6 | [ ] |
| Monitor selection for Desktop mode (follow game / follow mouse / primary / named monitor) | 3 | [ ] |
| "What's being recorded" indicator (tray + window) | 6 | [ ] |
| Codec choice: H.264 default, HEVC and AV1 optional | 3 | [ ] |
| Quality settings: resolution (native/downscaled), frame rate up to 144, quality/bitrate presets | 3 | [ ] |
| Full desktop audio + microphone tracks | 4 | [ ] |
| Microphone choice + push-to-talk | 4 | [ ] |
| One audio track per app (cap 8, then "Other"; always/never lists) | 5 | [ ] |
| Global hotkeys, tray icon, settings screen | 6 | [ ] |
| Game detection (auto + always/never list + pick a window) | 6 | [ ] |
| Swap monitor / mode mid-recording (hotkey + tray) | 6 | [ ] |
| Pause / resume manual recording | 6 | [ ] |
| Screenshot hotkey | 6 | [ ] |
| "Saved" feedback: sound + Windows notification | 6 | [ ] |
| Instant replay (rolling buffer, configurable length) | 7 | [ ] |
| Multiple save hotkeys (e.g. last 30 s / 2 min / full buffer) | 7 | [ ] |
| Auto-start replay when a game is detected, stop when it closes | 7 | [ ] |
| Storage: estimated disk use beside replay length, max library size + auto-delete oldest, low-disk warning | 7 | [ ] |
| Installer + auto-updates | 8 | [ ] |
| Companion editor reads our per-app tracks (change on the editor side) | 9 | [ ] |

## Later (agreed, after core works)

| Feature | Notes | Status |
|---|---|---|
| Bookmarks: hotkey marks a moment, stored as MP4 chapters | Needs the companion editor to display them; agree format together (step 9+) | [ ] |
| Full-session recording: auto-record whole match/session from game launch | Pairs with bookmarks | [ ] |
| Microphone noise suppression (optional) | Raw mic track kept untouched | [ ] |

## Out of scope (decided, with reasons)

- HDR recording: not wanted (2026-10-31).
- Automatic kill/win clipping: needs per-game integrations or AI vision across many titles; too big for a two-person project.
- In-game overlay: needs injection into the game, which risks anti-cheat bans. Sound + Windows notification instead.
- Cloud hosting / share links / social feed: server costs, accounts, moderation; a different product.
- Built-in streaming (Twitch etc.): OBS covers it.
- Editing effects (slow motion, captions, music): the companion editor's job.
