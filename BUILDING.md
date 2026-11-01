# BUILDING.md

## How builds are made

Every push to `main` runs the Windows build in GitHub Actions (`.github/workflows/build.yml`):

1. Downloads an LGPL Windows build of FFmpeg 8.1 or newer and checks it has the `gfxcapture`
   source and the GPU encoders. The build fails if `gfxcapture` is missing.
2. Runs the engine's tests.
3. Builds the engine (`encore-engine.exe`) and the app, and packages the installer.
4. Runs the engine self-test on the build machine (diagnostic only: build machines have no GPU).
5. Uploads the installer as a build artifact.

A release is published by running the workflow manually with a `release_tag` (e.g. `v0.5.4`).
It is published as the latest release, with `latest.json`, which installed copies check for updates.
Release runs and push runs queue separately, so a push never cancels a release build.
After each release: download the assets, check the update signature against the public key, and
check the installer contains the app, the engine and FFmpeg.

## Update signing

Automatic updates only accept builds signed with Encore's update key. The private key lives only in
the repository secret `TAURI_SIGNING_PRIVATE_KEY` (Settings → Secrets and variables → Actions). It has
no password. The matching public key is `plugins.updater.pubkey` in `src-tauri/tauri.conf.json`.
If the private key is ever lost, generate a new pair (`npx tauri signer generate`), update both, and
reinstall Encore manually once, since old installs only trust the old key.

## Testing

Build machines can prove the app builds, not that it records. Each batch ends with a hands-on test
on a real Windows 11 gaming PC, using the checklist in the release notes.

## Building locally (optional)

Only needed for local development. On Windows 11:

1. Git for Windows, Node.js LTS, Visual Studio Build Tools ("Desktop development with C++"), Rust (rustup).
2. Put `ffmpeg.exe` (8.1+, with `gfxcapture`) and its licence in `src-tauri/resources/` as
   `ffmpeg.exe` and `FFMPEG-LICENSE.txt`.
3. Build the engine and copy it where the app expects it:
   ```
   cargo build --release -p encore-engine
   copy target\release\encore-engine.exe src-tauri\binaries\encore-engine-x86_64-pc-windows-msvc.exe
   ```
4. `npm ci` then `npx tauri dev` (run) or `npx tauri build` (installer).

The engine can be checked on its own: `encore-engine --selftest --ffmpeg path\to\ffmpeg.exe`.
