## Encore 0.9.0 (beta), test build 21: game detection

### What's new
- **Game detection.** Click the controller under the screens (it glows purple when on). Encore
  then records games on their own:
  - When a game opens, recording and instant replay move to it, with just the game's window (no
    desktop, no Alt+Tab, no pop-ups).
  - When it closes, they go back to the screen you chose. While no game is open that screen is
    highlighted **green**, so you know detection is still on.
  - League of Legends: only the match counts, not the client. Between matches it's your screen.
  - Files and folders are named after the game ("League of Legends", Steam's own game names), and
    a replay is named after whatever was on screen for most of it.
- **Several games open:** the newest one is recorded. To choose:
  - click a game in the bar under the screens to lock it (click again to unlock);
  - **Ctrl+Alt+G** locks the game you're in; **Ctrl+Alt+N** moves to the next open game;
  - or use the tray menu's **Game** list.
  A locked game stays until it closes.
- **Settings → Games:** newest game or the game you're playing; what a recording does when its
  game closes (keep recording the screen, or stop and save); instant replay only while a game is
  open; "Always a game" and "Never a game" lists (or right-click a game → Never a game).
- **New layout:** instant replay controls top left; status on the right; Select Application to the
  right of the monitors; Do not disturb at the bottom of the side panel (it does nothing yet:
  notifications come next); audio options are glowing icon buttons, and hovering any of them says
  whether it's on or off. **Full System Audio** and **Split Application Audio** (new names) each
  have an (i) explaining them.
- Minimum window size is now 1100×700, so nothing gets squashed.

### Install
Update from inside Encore (≡ → Help → Update to 0.9.0).

### Test checklist
1. Turn on game detection. With no game open, your screen tile should be green and the status
   should say "Scanning for games".
2. Start a League match: the bar shows League of Legends and the status "Tracking League of
   Legends". Record 30 seconds of the match, then stop: the file is named League of Legends and
   only shows the game.
3. Instant replay on, play a match, then go back to the client: the replay should switch to your
   screen at the end of the match, and back to the next match. Save a replay across the switch:
   picture and sound in sync.
4. Open a second game (e.g. Warframe): it takes over. Click League in the bar to lock it; Warframe
   no longer takes over. Ctrl+Alt+N moves between them; tray → Game shows the list.
5. Settings → Games → "Stop and save", record, then close the game: the recording should save.
6. Turn off "Also when no game is open" (under instant replay): with no game open, the status
   should say "Waiting for a game"; it starts when a game opens.
7. Anything detected as a game that isn't: right-click it → Never a game. Anything missed: Settings
   → Games → Always a game → Add….

### What to send back
Worked / didn't work for each step, and `%LOCALAPPDATA%\Encore\logs\engine.log` (it now lists
each game found and why).
