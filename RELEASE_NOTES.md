## Encore 0.5.4 (beta), test build 6: setup on first start, smoother game recordings, Encore folder

**0.5.4 fixes:**
- Choosing folders in the setup no longer leaves empty folders behind. Encore only checks it
  could save there; the Encore folder is made when you finish the setup (or on your first
  recording).
- The frame rate and resolution dropdowns in the setup now open (they were opening behind it).
- The top bar names the graphics card Encore actually uses, not every one it found.

### What's new
- **Setup on first start.** Three short steps: where to save recordings (with a Browse button),
  video quality (with an estimate of how big a minute of recording will be on your screen) and
  frame rate, then sound. Everyone sees it once, including people updating. Run it again any
  time from ≡ → Help → Run Setup Again.
- **Finds your real Videos folder.** Encore now asks Windows where your Videos folder is (it can
  be on another drive or in OneDrive) instead of assuming `C:\Users\<you>\Videos`, which made a
  new, empty folder for some people.
- **Browse button** next to "Save recordings to" in Settings.
- **Smoother recordings while a game uses the whole graphics card.** Tests showed that with the
  game in front, the capture got as few as 0–10 new frames a second (Task Manager: game at 90 %
  GPU). Encore now asks Windows to give its capture priority on the graphics card and processor
  while recording.
  - **Run Encore as administrator** for the strongest effect (right-click → Run as administrator):
    Windows only allows the top priority level with admin rights.
  - Capping the game's frame rate (or V-Sync) also leaves room for recording.
- **Encore folder:** whatever folder you choose to save to, Encore makes a folder called
  **Encore** inside it, with a folder for Desktop and each game or app:
  `D:\Clips\` → `D:\Clips\Encore\Desktop\`, `D:\Clips\Encore\Warframe\`. Settings shows the
  full path. If the folder you choose is already called Encore, it's used as it is. A folder
  Encore can't write to is refused with a message (e.g. `C:\Program Files` needs admin rights).
  Your current setting keeps working: recordings stay in `Videos\Encore`.
- **Slightly smaller files:** the extra audio tracks are now 128 kbps (the main Full mix track
  stays 192). Audio was only 1–3 % of a file; the video is what makes files big, and options for
  that (custom bitrate, size per minute) come later.
- **App names:** apps that wouldn't share their name (protected games) are now named after their
  program file instead of "App 24720".

### Install
Update from inside Encore (≡ → Help → Update to 0.5.4). Otherwise download
`Encore_0.5.4_x64-setup.exe` below and run it. Windows may say "Windows protected your PC": click
**More info**, then **Run anyway**.

### Test checklist
0. **Setup.** It should open by itself. Check the folder it suggests is your real Videos folder,
   try Browse, look at the size estimates, finish. Reopen Encore: it shouldn't show again.
1. **Game stutter.** Record 20 seconds of Warframe gameplay, uncapped, the normal way. Then run
   Encore as administrator and record 20 seconds again. Send both: I'll count the real frames.
2. **Encore folder.** In Settings → Saving, choose another folder (e.g. `D:\Clips`). Record: the
   file should land in `D:\Clips\Encore\Desktop\`. Try a folder like `C:\Program Files`: Encore
   should refuse it with a message.
3. **Monitor follow** (from 0.5, if not tested yet): record, click a window on the other monitor
   after 5 seconds, click back after 5 more, stop. Send the file.

### What to send back
Worked / didn't work for each step, any message Encore showed, and if something failed,
`%LOCALAPPDATA%\Encore\logs\engine.log` (paste that path into File Explorer's address bar).
The log now also says which graphics priority Encore got ("GPU priority realtime" or "high").
