# Encore

A lightweight game and screen recorder for Windows 11, made as a pair with the companion clip editor.

- **What to record:** your primary monitor (recommended), always another monitor, Follow (the screen you click on), or Select Application
  (a single app's window)
- **Replay:** Encore quietly keeps the last 30 seconds to 20 minutes, so you can save a great
  moment after it happens (Save a replay), or just the last 30 seconds, minute or a length you
  choose, with hotkeys that work in games
- **Split Application Audio:** a separate audio track for each app that makes sound (game, music,
  browser, voice chat), plus Full mix (everything, including your mic), Microphone and Desktop audio.
  Encore's own sounds are never recorded
- GPU encoding on NVIDIA, AMD and Intel (H.264, HEVC or AV1), with graphics priority so games
  that use the whole card don't make recordings stutter (needs "Start Encore as administrator"),
  and **Priority Capture** for games that put a heavy load on the card or less powerful cards
- Saves into an Encore folder wherever you choose, with a folder per game or app. Clips you
  save show in the sidebar with a picture: click to open them in the companion clip editor, your
  video player or their folder; rename them, or delete them with 5 seconds to undo
- Tray icon, start with Windows, start/stop/save sounds you can choose, and a status panel
- Finds your microphone again if Windows renames it, and warns you when it's unplugged
- In-app updates, on your terms (tell me, download automatically, or never)
- **Game detection:** games are recorded on their own as they open (just the game's window), and
  the screen you chose takes over when they close; lock a game when several are open. Turn
  Desktop Capture off to record games only
- **Notifications** over your game (never recorded): a card in the corners or an edge tab at the
  sides, with a timer, stacking up to three, in the size you choose

Status: beta (0.x). See [FEATURES.md](FEATURES.md) for what is built and what is planned.

## Install

Download the latest `Encore_x.y.z_x64-setup.exe` from
[Releases](https://github.com/ancillaauri/Encore/releases/latest) and run it.
The installer is not code-signed yet, so Windows may show "Windows protected your PC" the first
time: click **More info**, then **Run anyway**. After that, Encore updates itself from inside the
app (Main Menu → Help).

## Docs

- [DEVELOPMENT.md](DEVELOPMENT.md): how this project is worked on, and the hard rules
- [ARCHITECTURE.md](ARCHITECTURE.md): how the app is put together, and the recording file format
- [DESIGN.md](DESIGN.md): Encore's look (colours, glows, rules), being redesigned
- [DECISIONS.md](DECISIONS.md): what was decided and why, test results and the performance log
- [FEATURES.md](FEATURES.md): the feature backlog and progress
- [BUILDING.md](BUILDING.md): how builds and releases are produced
- [CHANGELOG.md](CHANGELOG.md): starts at version 1.0; until then see each release's notes

## Licence

MIT (see [LICENSE](LICENSE)). Third-party components are listed in
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
