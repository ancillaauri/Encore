## Encore 0.7.0 (beta), test build 8: instant replay

### What's new
- **Instant replay.** Turn it on in the left panel and Encore keeps the last few minutes of
  whatever is selected (the screen you're using, a monitor or an app). When something great
  happens, press **Ctrl+Alt+S** (or **Save replay** in the panel or the tray menu) and the last
  2 minutes are saved, with every audio track, while Encore keeps going.
  - Length: 30 seconds to 20 minutes. The panel shows how much disk space it uses (High quality,
    1440p at 120 fps: about 2 GB for 2 minutes). Older footage is deleted as it goes.
  - Saved replays appear under "Saved this session" with a *replay* tag, in the same folders as
    recordings.
  - Recording while replay is on uses the same capture, so there's never a second one slowing
    things down.
  - Changing what's recorded or the video settings restarts the buffer (it starts again from now).
  - If it was on when you closed Encore, it's on again next time.
  - The save hotkey can be changed in Settings → Hotkeys.

From 0.6.0 (if you haven't updated): the **tray icon**, **closing to the tray**, the **Ctrl+Alt+R**
hotkey to start and stop recording, start/stop **sounds**, and only one copy of Encore running.

### Install
Update from inside Encore (≡ → Help → Update to 0.7.0). Otherwise download
`Encore_0.7.0_x64-setup.exe` below and run it. Windows may say "Windows protected your PC": click
**More info**, then **Run anyway**.

### Test checklist
1. **Replay in a game.** Turn instant replay on, play for 3 minutes, press Ctrl+Alt+S. You should
   hear a short double beep, then "Saved replay" appears. The file should be about the last
   2 minutes, smooth, with all audio tracks in sync. Send it if you can.
2. **Save twice quickly** (two Ctrl+Alt+S a few seconds apart): both files saved.
3. **Record while replay is on:** Ctrl+Alt+R, play 30 seconds, Ctrl+Alt+R. The recording should be
   just that stretch.
4. **Switch monitors** with replay on (click a window on the other screen), then save: the
   replay should switch at the same moment.
5. **Turn replay off**, and quit with it on: no errors, and the temp folder
   `%LOCALAPPDATA%\Encore\temp` should be empty afterwards.
6. **From 0.6.0:** the tray, closing to the tray, the record hotkey in a game, sounds.
7. **Still waiting:** the game stutter test (normal vs run as administrator).

### What to send back
Worked / didn't work for each step, any message Encore showed, and if something failed,
`%LOCALAPPDATA%\Encore\logs\engine.log` (paste that path into File Explorer's address bar).
