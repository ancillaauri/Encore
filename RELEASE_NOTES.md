## Encore 0.7.7 (beta), test build 15: Encore's sounds stay out of your recordings

### What's changed
- **Encore's own sounds aren't recorded.** The start, stop and save sounds used to end up in the
  desktop audio track. Desktop audio now records everything except Encore.
- **No more "Microsoft Edge WebView2" track.** That was Encore's own window; Split App Tracks now
  leaves it out.
- From 0.7.6: hotkeys work in games like League of Legends, and replay messages show the length
  you saved.

### Install
Update from inside Encore (≡ → Help → Update to 0.7.7).

### Test checklist
1. With sounds on, play some music, record 20 seconds (start and stop with Ctrl+Alt+R) and save a
   replay during it. In the file: the music is there, the beeps aren't, in Full mix and Desktop
   audio.
2. No "Microsoft Edge WebView2" track.
3. Everything else in desktop audio still there (game, Discord, browser).
4. If desktop audio is silent or missing something, send the log: look for "desktop audio without
   Encore's sounds not available".
