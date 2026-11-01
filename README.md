# Encore

A lightweight game and screen recorder for Windows 11, made as a pair with the companion clip editor.

- **What to record:** Follow (the screen you click on), always one monitor, or Select Application
  (a single app's window)
- **Instant replay:** keeps the last 30 seconds to 20 minutes; save it all, or just the last 30
  seconds, minute or a length you choose, with hotkeys that work in games
- **Split Application Audio:** a separate audio track for each app that makes sound (game, music,
  browser, voice chat), plus Full mix (everything, including your mic), Microphone and Desktop audio.
  Encore's own sounds are never recorded
- GPU encoding on NVIDIA, AMD and Intel (H.264, HEVC or AV1), with graphics priority so games
  that use the whole card don't make recordings stutter (needs "Start Encore as administrator")
- Saves into an Encore folder wherever you choose, with a folder per game or app
- Tray icon, start with Windows, start/stop/save sounds you can choose, and a status panel
- In-app updates, on your terms (tell me, download automatically, or never)
- **Game detection:** games are recorded on their own as they open (just the game's window), and
  the screen you chose takes over when they close; lock a game when several are open
- Coming next: Encore's own notifications

Status: beta (0.x). See [FEATURES.md](FEATURES.md) for what is built and what is planned.

## Install

Download the latest `Encore_x.y.z_x64-setup.exe` from
[Releases](https://github.com/ancillaauri/Encore/releases/latest) and run it.
The installer is not code-signed yet, so Windows may show "Windows protected your PC" the first
time: click **More info**, then **Run anyway**. After that, Encore updates itself from inside the
app (≡ → Help).

## Docs

- [DEVELOPMENT.md](DEVELOPMENT.md): how this project is worked on, and the hard rules
- [ARCHITECTURE.md](ARCHITECTURE.md): how the app is put together, and the recording file format
- [DECISIONS.md](DECISIONS.md): what was decided and why, test results and the performance log
- [FEATURES.md](FEATURES.md): the feature backlog and progress
- [BUILDING.md](BUILDING.md): how builds and releases are produced
- [CHANGELOG.md](CHANGELOG.md): starts at version 1.0; until then see each release's notes

## Licence

MIT (see [LICENSE](LICENSE)). Third-party components are listed in
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
