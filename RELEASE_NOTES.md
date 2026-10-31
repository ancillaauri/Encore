## Encore 0.5.0 (beta), test build 5: new layout, and recording follows the screen you click on

### What's new
- **Recording follows the screen you're using.** In Desktop mode, Encore records the monitor of
  the window you last clicked on. Click a window on your other monitor while recording and the
  recording moves there within about half a second, with no gap and the audio still in sync.
  Moving the mouse across without clicking doesn't move it, and clicking Encore itself doesn't
  count.
- **New layout:**
  - **Under the record button:** your monitors, drawn as holograms and laid out the way Windows
    has them. The one that will be recorded glows blue; while recording it glows red.
    - Click a monitor to **always** record that one (it shows "ALWAYS"). Click it again to go
      back to following the screen you use.
    - **One app:** pick a window from the list to record only that window.
    - **Game:** automatic game detection, coming in the next update.
  - **Left panel:** quick settings (quality, frame rate, resolution, desktop audio, mic, a
    track for each app) and **Saved this session**. Click a recording there to show it in its
    folder. All other settings are still under "All settings" or ≡ → File → Settings.
- The old "Monitor with the mouse" and "Primary monitor" choices are replaced by following the
  screen you use. If you had picked a specific monitor, it's still picked.

### Install
Update from inside Encore (≡ → Help → Update to 0.5.0). Otherwise download
`Encore_0.5.0_x64-setup.exe` below and run it. Windows may say "Windows protected your PC": click
**More info**, then **Run anyway**.

### Test checklist
1. **Follow.** Start recording with a window focused on one monitor. After about 5 seconds click
   a window on the other monitor, then after 5 more click back. Stop. In the file, the picture
   should switch at each click, with no freeze, no black frames and no jump in the sound. Send
   the file if you can: I'll measure the switch points.
2. **Not following.** While recording, move the mouse to the other monitor without clicking,
   and click Encore's own window. Neither should move the recording.
3. **Always one monitor.** Click a monitor in the picker so it says ALWAYS, record, click
   around on the other monitor: the recording stays put. Click it again to unpin.
4. **One app.** Pick an app, record, alt-tab away: only that window is recorded.
5. **Layout.** Quick settings change the same settings as the Settings window. Recordings appear
   under "Saved this session". Collapse the panel (Ctrl+B).
6. **Still outstanding:** game FPS with and without recording; closing Encore mid-recording.

### What to send back
Worked / didn't work for each step, any message Encore showed, and if something failed,
`%LOCALAPPDATA%\Encore\logs\engine.log` (paste that path into File Explorer's address bar).
