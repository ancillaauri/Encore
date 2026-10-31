## Encore 0.3.0 (beta), test build 3: new layout and settings

### What's new
- **New layout**, in the same style as the companion clip editor:
  - **≡ menu** in the top-left: Settings, Check for updates, Changelog, About Encore.
  - **What to record** panel on the left (Desktop or One window, and which monitor). Collapse it
    with the icon next to its title; it shrinks to two icons.
  - **Settings** now opens as a window over the app: Video, Audio, Saving and Updates.
- **Updates, your choice** (Settings → Updates):
  - Tell me when there's an update (default): nothing downloads until you click.
  - Download updates automatically: you still choose when to install.
  - Never check for updates (not recommended).
  - **Check for updates** works whichever you pick.
- **Changelog**: an empty placeholder for now. It starts at version 1.0; until then each test build's
  notes (like these) list what changed.
- Still in this build from 0.2: separate audio tracks (Full mix, Microphone, one per app) and the new icon.

### Install
Download `Encore_0.3.0_x64-setup.exe` below and run it (it updates earlier versions in place). Windows
may say "Windows protected your PC": click **More info**, then **Run anyway**.

### Test checklist
1. **Layout.** Open the ≡ menu and each item. Collapse and expand the left panel. Change a setting,
   close Encore, reopen it: is the setting kept?
2. **Updates.** In Settings → Updates, pick each option and press **Check for updates**. (Installing
   updates from inside the app needs the update-signing setup finished; until then the check may
   say it couldn't check, which is expected.)
3. **Audio (from 0.2, not tested yet).** Record about a minute with a game, Spotify, a YouTube video
   and you talking. While recording, the track list under the timer should grow. Send the file if
   you can: I'll measure the sync and check every track.
4. **Still outstanding from build 1:** a game window with an alt-tab; game FPS with and without
   recording; closing Encore mid-recording and checking it saved.

### What to send back
Worked / didn't work for each step, any message Encore showed, and if something failed,
`%LOCALAPPDATA%\Encore\logs\engine.log` (paste that path into File Explorer's address bar).
