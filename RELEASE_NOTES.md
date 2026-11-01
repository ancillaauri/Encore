## Encore 0.9.5 (beta), test build 26: clearer "What to record", and games only

### What's changed
- **"What to record" explains itself:** it's used for recordings *and* Echoes, and **Follow** is
  marked **Recommended**, so you can leave it alone.
- **The Echoes panel says what it's keeping**: "keeping the last 2 minutes of Warframe" (or your
  screen).
- **Only want games?** With game detection on, the bar under the screens now has
  **Until a game opens: Record my screen / Wait for a game**. "Wait for a game" means nothing is
  kept on the desktop: Echoes starts when a game opens, and a recording ends when its game closes.
  The screens dim, since they're only used if you press record with no game open.
- Setup's game detection page offers the same choice ("Only record games").

### Install
Update from inside Encore (≡ → Help → Update to 0.9.5).

### Test checklist
1. Game detection on → **Wait for a game**, Echoes on. On the desktop: status says "Waiting for a
   game" and Echoes waits. Open Warframe: Echoes starts ("keeping the last 2 minutes of Warframe").
   Close it: Echoes waits again.
2. Switch back to **Record my screen**: Echoes keeps your screen again (green).
