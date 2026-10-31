## Encore 0.1.0, test build 1: screen recording (video only)

### What's in this build
- Record your whole monitor, or a single window (alt-tabbing never shows your desktop)
- Choose the monitor: primary, the one your mouse is on, or a specific monitor by name
- Encoding on your graphics card (NVIDIA, AMD or Intel), H.264 / HEVC / AV1
- Frame rate, resolution and quality settings
- Recordings are written in 2-second pieces, so a crash loses seconds, not the recording.
  Anything interrupted is recovered the next time Encore opens.

### Not in this build yet
Audio, hotkeys, tray icon, instant replay, game detection. These come in the next test builds.

### Install
Download `Encore_0.1.0_x64-setup.exe` below and run it. Windows will probably say
"Windows protected your PC" because the installer isn't code-signed: click **More info**, then **Run anyway**.

### Test checklist (about 10 minutes)
1. **Start-up.** Open Encore. The top-right should say "Encoding on your NVIDIA/AMD/Intel graphics card".
   If it says anything else, take a screenshot.
2. **Desktop recording.** Leave "Desktop" selected, press the big button, move some windows around for
   about 30 seconds, press it again. You should see "Saved …". Click **Show in folder** and play the file:
   right monitor, smooth, correct length?
3. **Game window.** Open a game in windowed or borderless mode. In Encore choose **One window**, pick the
   game, record 1–2 minutes of play, and alt-tab out once in the middle. The file should show only the
   game, including during the alt-tab.
4. **Performance.** Note the game's FPS with and without recording (in-game counter or overlay). While
   recording, open Task Manager and note CPU and memory for Encore, encore-engine and ffmpeg.
5. **Closing mid-recording.** Start a recording, then close Encore's window. Reopen Encore and check the
   recording was saved.
6. **Second monitor** (if you have one). Pick it under Monitor, record 10 seconds, check the file.
7. **HEVC.** Switch Format to HEVC, record 10 seconds, check the file plays.

### What to send back
For each step: worked / didn't work, plus any message Encore showed (the text can be selected and copied).
If something failed, also send the log file: `%LOCALAPPDATA%\Encore\logs\engine.log`
(paste that path into the File Explorer address bar).
