## Encore 0.9.6 (beta), test build 27: Desktop Capture

### What's changed
- **Desktop Capture** heads the screens section, with its own glowing icon switch.
  - **On** (the default): Encore records your chosen screen when no game is open, as before.
  - **Off**: Encore only records games. The screens dim, Echoes waits for a game, a recording ends
    when its game closes, and the record button is greyed out until a game opens.
  - Off needs game detection on (otherwise nothing could be recorded), so turning game detection
    off turns Desktop Capture back on.
- The "Until a game opens" choice in the game bar is gone; Desktop Capture replaces it.
- **Icon switches look like buttons**: each glowing icon sits on a faint tile, tinted when on.

### Install
Update from inside Encore (≡ → Help → Update to 0.9.6).

### Test checklist
1. Game detection on, Desktop Capture off, Echoes on: status says "Desktop Capture: Off · waiting
   for a game", the record button is greyed out. Open Warframe: Echoes starts and record works.
   Close it: Echoes waits again.
2. Turn game detection off: Desktop Capture turns itself back on.
