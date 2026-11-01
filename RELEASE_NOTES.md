## Encore 0.7.3 (beta), test build 11: smooth game recordings, without the hassle

### What's new
- **Game stutter is fixed.** When a game uses all of your graphics card, Encore now gets priority
  for the recording, so clips stay smooth (tested: Warframe at 120 fps, every frame captured).
  This needs Encore to run as administrator.
- **Start Encore as administrator** (Settings → General). Turn it on and Windows asks for
  permission once. After that Encore always opens with admin rights, with no prompt. Turning it off
  asks once more and puts things back as they were.
  - New installs are offered it during setup ("Smooth game recordings").
  - If you ticked "Run this program as an administrator" in Encore's Properties for testing, untick
    it: the setting replaces it, and that box makes Windows ask every time.
- **Update checks** try again by themselves if GitHub can't be reached, and the message is plainer.

### Install
Update from inside Encore (≡ → Help → Update to 0.7.3). Otherwise download
`Encore_0.7.3_x64-setup.exe` below and run it. Windows may say "Windows protected your PC": click
**More info**, then **Run anyway**.

### Test checklist
1. Untick "Run this program as an administrator" in Encore's Properties (Compatibility tab).
2. Quit Encore from the tray and open it. ≡ → File should show **Restart as Administrator**.
3. Settings → General → turn on **Start Encore as administrator**. Windows asks once; say yes.
   Click **Restart now**: Encore should reopen with no prompt, and Restart as Administrator should
   be gone from ≡ → File.
4. Quit from the tray and open Encore again: no prompt, still elevated.
5. Record uncapped Warframe for 20 seconds: smooth, no warning.
6. Turn the setting off (Windows asks once). Quit and open: normal start, Restart as Administrator
   is back.
7. Say **No** to the Windows prompt once: Encore should say nothing was changed.

### What to send back
Worked / didn't work for each step, any message Encore showed, and the log
(`%LOCALAPPDATA%\Encore\logs\engine.log`) if something failed.
