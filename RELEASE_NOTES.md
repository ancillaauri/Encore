## Encore 0.7.6 (beta), test build 14: hotkeys in League, right lengths in messages

### What's changed
- **Hotkeys in games like League of Legends.** Encore used Windows' standard hotkey system, which
  some games block while they're in front. It now watches the keys itself, the way other
  recorders do, so hotkeys work whatever is in front. One difference: the keys now also reach the
  game, so choose combinations the game doesn't use (the defaults should be fine in most games).
- If the same keys are set for two actions, Encore says so.
- **Replay messages show the right length.** Saving with the 30-second hotkey now says "Saving
  the last 30 seconds" and "Saved replay (30 seconds)", instead of the replay length.
- Every hotkey press is noted in the log, so if one ever seems to do nothing we can tell whether
  Encore saw it.

### Install
Update from inside Encore (≡ → Help → Update to 0.7.6).

### Test checklist
1. In a League match, with instant replay on: Ctrl+Alt+S, Ctrl+Alt+1, Ctrl+Alt+2. Each should
   beep and save. Also Ctrl+Alt+R to start and stop a recording.
2. Check the messages show the length you asked for.
3. In Settings → Hotkeys, set a new key: pressing it while choosing shouldn't save anything.
4. If a hotkey does nothing, send `%LOCALAPPDATA%\Encore\logs\engine.log` (look for
  "hotkey … pressed").
