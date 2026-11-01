## Encore 0.9.4 (beta), test build 25: safety fixes

### What's changed
- **Possible cause of a PC freeze removed.** Encore was giving its recording the very highest
  graphics priority ("realtime"), which can crowd out the rest of the system. It now uses "high",
  which should still keep games smooth in recordings.
- **Screenshot tools aren't games.** Lightshot (and ShareX, Greenshot, Snagit and others) cover the
  screen for a moment and were being picked up by game detection. They're now ignored, and anything
  that only looks like a game because it fills the screen has to stay for a few seconds first.
- **"Couldn't switch what's being recorded"** is now a plain message naming what couldn't be
  recorded, shown once, and Encore waits before trying again.
- A recording made while Echoes is on can no longer replace an Echo saved in the same second.

### Install
Update from inside Encore (≡ → Help → Update to 0.9.4).

### Test checklist
1. Warframe with game detection on, uncapped: is it still smooth in the recording? (This checks
   that "high" priority is enough.)
2. Play for 10+ minutes with Echoes on, save a few Echoes: no freeze.
3. Take a Lightshot screenshot: Encore shouldn't switch to it.
