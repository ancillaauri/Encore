## Encore 0.4.1 (beta), test build 4: mic in the main track, new title bar, matched design

**0.4.1:** recordings are now marked as the new track layout (`rec-format:2`), so the companion
clip editor can tell them apart from 0.3 recordings, whose first track has no mic. Nothing else
changed from 0.4.0.

### Changes since 0.3.1

- **The first audio track now includes your microphone.** "Full mix" is everything: the PC's
  sound plus your voice. That's the track players, Discord and VLC use, so a recording sounds
  right anywhere without editing.
- **New "Desktop audio" track**: everything the PC played, without your mic. Present when the mic
  is recorded.
- Track order in a file: Full mix, Microphone, Desktop audio, then one per app.
- **Encore's own title bar.** The Windows frame is gone; the menu, name and window buttons sit in
  one dark bar, like the companion clip editor. Drag it to move the window, double-click to
  maximise.
- **Matched to the companion clip editor**, piece by piece:
  - ≡ menu now has File, View and Help, with shortcuts: **Ctrl+,** opens Settings, **Ctrl+B**
    shows or hides the What to record panel.
  - Dropdowns, buttons, tooltips, messages, scrollbars, dialogs and their fade-ins all look and
    behave like the editor's.
  - The What to record panel is the editor's width; collapsed, its icons sit where the editor's do.
  - About and Changelog open as the editor's smaller dialogs.
- Settings → Audio: "Everything you hear" is now called **Desktop audio**.

### Install
Already on 0.3.1 or 0.4.0? Update from inside Encore (≡ → Help → Update to 0.4.1). Otherwise download
`Encore_0.4.1_x64-setup.exe` below and run it. Windows may say "Windows protected your PC": click
**More info**, then **Run anyway**.

**Taskbar still showing the old icon?** That's Windows' icon cache, not Encore. Press Win+R, type
`ie4uinit.exe -show` and press Enter. If it's still old, unpin Encore from the taskbar and pin it
again, or restart your PC.

### Test checklist
1. **Update from inside the app.** Does it close, install and reopen on 0.4.1 by itself?
2. **Title bar.** Drag the window by the bar, double-click it, use minimise / maximise / close.
   Resize from the edges. Snap it with Win+Left/Right.
3. **Audio.** Record about 30 seconds with music playing and you talking.
   - Play the file in VLC: your voice and the music should both be there.
   - In VLC (Audio → Audio Track) check Microphone, Desktop audio and the app tracks.
4. **Look.** Open everything (menu, each submenu, Settings, About, Changelog, the dropdowns,
   collapse the panel) and compare with the clip editor. Anything that doesn't match, screenshot it.
5. **Still outstanding:** a game window with an alt-tab; game FPS with and without recording;
   closing Encore mid-recording and checking it saved.

### What to send back
Worked / didn't work for each step, any message Encore showed, and if something failed,
`%LOCALAPPDATA%\Encore\logs\engine.log` (paste that path into File Explorer's address bar).
