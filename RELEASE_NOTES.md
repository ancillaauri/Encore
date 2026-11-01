## Encore 0.8.0 (beta), test build 16: Ultra, start with Windows, working folder

### What's new
- **Ultra quality.** What was High is now called **Ultra** (same quality, still the default), and
  there's a new **High** between Ultra and Medium: very close in quality, files about a fifth
  smaller. If you had High selected, you're now on Ultra, so nothing changes until you pick.
- **Start Encore when Windows starts** (Settings → General). Encore opens in the tray, so hotkeys
  and instant replay are ready straight away. Works with "Start Encore as administrator" too, with
  no prompt.
- **Working folder** (Settings → Saving). Where recordings are written while they're being made,
  and where instant replay keeps its last few minutes. It used to always be on C:; you can now
  pick a bigger or faster drive. Encore makes an "Encore temp" folder inside the one you choose.
- **Settings contents.** Settings has a list of sections on the left: click one to jump to it.
  The section you're looking at is highlighted.

### Install
Update from inside Encore (≡ → Help → Update to 0.8.0).

### Test checklist
1. Quality shows **Ultra** (left panel and Settings → Video). Try High: files should be a bit
   smaller and still look great.
2. Settings → General → turn on **Start Encore when Windows starts**, then restart the PC: Encore
   should be in the tray (no window), running as administrator, with instant replay on if it was
   on before.
3. Settings → Saving → **Working folder** → Browse… and pick a folder on another drive. Turn
   instant replay on: an "Encore temp" folder should appear there and fill up. Default puts it back.
4. Settings contents: click each section; scroll and check the highlight follows.
