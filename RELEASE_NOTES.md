## Encore 0.7.1 (beta), test build 9: the stutter fix, properly

### What's changed
- **Game stutter.** The fix from 0.5.2 never applied: Windows only lets Encore raise the
  recording's priority on the graphics card when Encore runs as administrator, and even then the
  permission has to be switched on first. Encore now switches it on.
- **Restart as Administrator.** ≡ → File → Restart as Administrator closes Encore and opens it
  again with admin rights (Windows asks first). It isn't shown when Encore already has them.
- **A warning when priority is refused,** once per run, instead of the recording quietly
  stuttering.
- The log now says whether Encore is running as administrator, and why priority was refused.

### Install
Update from inside Encore (≡ → Help → Update to 0.7.1). Otherwise download
`Encore_0.7.1_x64-setup.exe` below and run it. Windows may say "Windows protected your PC": click
**More info**, then **Run anyway**.

### Test checklist
1. After updating, choose ≡ → File → **Restart as Administrator** and say yes to Windows.
   Encore should reopen, and the menu item should be gone.
2. Record 20 seconds of uncapped Warframe (V-Sync off). It should be smooth.
3. Send the clip and `%LOCALAPPDATA%\Encore\logs\engine.log`. The line after
   "starting capture" should say **GPU priority realtime** or **high**.
4. Without admin (quit and open normally): starting a recording should show the warning once.

### What to send back
Worked / didn't work for each step, the clip, and the log.
