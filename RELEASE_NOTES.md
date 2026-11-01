## Encore 0.8.1 (beta), test build 17: Follow gets its own tile

### What's changed
- **Follow** is now its own choice under "What to record". Pick it to record whichever screen you
  click on (the screen in use shows "In use"). Pick a monitor to always record that one; picking
  it again no longer switches back to following.
- **One app** is now called **Select Application**.
- The picker stays on one line in small windows (the tiles shrink).
- This sets things up for game detection, which is next.

### Install
Update from inside Encore (≡ → Help → Update to 0.8.1).

### Test checklist
1. Follow is selected by default (unless you had a monitor pinned). Click a window on your other
   monitor: "In use" moves to that monitor.
2. Click a monitor: it lights up and is always recorded. Click Follow to go back.
3. Select Application → pick a window. Then click Follow or a monitor to go back to screens.
4. Make the window small: everything stays on one line.
