## Encore 0.6.0 (beta), test build 7: tray icon, hotkeys and sounds

### What's new
- **Hotkey to start and stop recording: Ctrl+Alt+R.** Works anywhere, including in games.
  Change it in Settings → Hotkeys: click the box, press the keys you want. It records whatever is
  selected in Encore (the screen you're using, a monitor or an app).
- **Tray icon** by the clock. Left-click opens Encore; right-click has Start/stop recording and
  Quit. It shows a red dot while recording.
- **Closing the window keeps Encore running in the tray**, so the hotkey keeps working. The first
  time, Windows tells you where it went. To really quit: the tray icon → Quit, or ≡ → File → Exit.
  Prefer the old way? Settings → General → "Quit Encore".
- **Sounds:** a short rising tone when recording starts and a falling one when it stops, so you
  know the hotkey worked without leaving your game. Turn off in Settings → General.
- **Only one Encore:** opening Encore while it's in the tray brings the window back instead of
  starting a second copy.
- The per-app audio feature is now called **Split App Tracks**.

Also in this version (from 0.5.x, if you haven't updated): setup on first start, your real Videos
folder, the Encore folder, graphics priority while recording, and following the screen you click.

### Install
Update from inside Encore (≡ → Help → Update to 0.6.0). Otherwise download
`Encore_0.6.0_x64-setup.exe` below and run it. Windows may say "Windows protected your PC": click
**More info**, then **Run anyway**.

### Test checklist
1. **Hotkey in a game.** Start a game, press Ctrl+Alt+R: you should hear the start tone. Play a bit,
   press it again: the stop tone, and the file is saved. Does the game notice the keys at all?
2. **Tray.** Close Encore's window: a notification, and the icon stays by the clock. Right-click it:
   Start recording, then Stop. Left-click: the window comes back.
3. **Change the hotkey** in Settings → Hotkeys. Try one another app already uses (e.g. one from
   Discord): Encore should say it couldn't set it up.
4. **Quit** from the tray, and from ≡ → File → Exit: Encore should close completely.
5. **Open Encore twice** (from the Start menu while it's in the tray): only one window.
6. **Still waiting:** the game stutter test (normal vs run as administrator) and following the
   screen you click on.

### What to send back
Worked / didn't work for each step, any message Encore showed, and if something failed,
`%LOCALAPPDATA%\Encore\logs\engine.log` (paste that path into File Explorer's address bar).
