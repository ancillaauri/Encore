## Encore 0.9.12 (beta), test build 33: fewer false alarms, no double saves

### Changed
- **The "Recording is falling behind" warning** now only appears when it affects your Echoes:
  the capture is still behind after 30 seconds, or more than 10 seconds behind. A short slip
  while a game loads no longer sets it off. Every slip is still noted in Encore's log.
- **Pressing a save hotkey twice by accident** (within a second and a half) saves one Echo, not
  two identical ones.

### Install
Update from inside Encore (≡ → Help → Update to 0.9.12).

## Encore 0.9.11 (beta), test build 32: notifications you can see

### Fixed
- **Notifications showing on the wrong screen.** They now go to the screen your game is on (with
  "The screen you're using" picked in Settings → Notifications).
- **Notifications that never appeared.** The notification window was sometimes paused by Windows
  for a few seconds after showing, so the card never drew. It now always draws straight away.
- A notification arriving just as the last one faded could be hidden at once.

### Install
Update from inside Encore (≡ → Help → Update to 0.9.11).

## Encore 0.9.10 (beta), test build 31: top graphics priority

### What's new
- **Settings → Advanced → Top graphics priority.** For games that use all of your graphics card
  (Warframe, for one). It puts Encore's capture first on the graphics card so recordings and
  Echoes don't fall behind. Off by default, with a warning and a confirmation: on a PC with an
  unstable graphics driver it can black the screen for a moment or freeze the PC.
- **A warning when the recording falls behind.** If a game takes so much of the graphics card
  that Encore can't keep up, you're told once per game, with how to fix it.

### Install
Update from inside Encore (≡ → Help → Update to 0.9.10).

## Encore 0.9.9 (beta), test build 30: admin after updates, Echoes that fell behind

### Fixed
- **Encore lost administrator rights after every update.** With "Start Encore as administrator"
  on, an updated Encore now comes back as administrator. This update is the last one that
  arrives without it: start Encore again once afterwards, or use File → Restart as Administrator.
- **Echoes failing with "Nothing has been recorded for that moment yet".** When the game stalls
  and the capture falls behind, saving an Echo now saves the newest moments recorded instead of
  failing.
- Encore's log now notes when notifications appear and disappear, to track down the lag.

### Install
Update from inside Encore (≡ → Help → Update to 0.9.9).

## Encore 0.9.8 (beta), test build 29: notifications

### What's new
- **Notifications over your game.** A small pop-up when you save an Echo, start or save a
  recording, or Encore follows or locks a game, plus warnings. They never appear in your
  recordings or Echoes.
- **Two looks, chosen by where you put them.** The hologram card in the corners and at top and
  bottom centre, with a timer line on the side facing into the screen. The edge tab at the middle
  of the left or right edge, sliding in from the side.
- **Up to three at once**, stacked.
- **Settings → Notifications:** a monitor diagram to pick the spot, how long they stay (2 to 10
  seconds), which screen, which events, and a Show a test button.
- **Setup** has a Notifications page.
- They don't show while Encore's own window is in front, or with Do not disturb on. Games in
  exclusive fullscreen draw over them; borderless or windowed games show them.

### Install
Update from inside Encore (≡ → Help → Update to 0.9.8).

## Encore 0.9.7 (beta), test build 28: setup tidy-up

### What's changed
- Setup is down to 6 pages; the "What to record" page is gone (the main screen explains itself
  now).
- The game detection page is up to date with Desktop Capture, and its Desktop Capture switch can
  only be turned off with game detection on.
- Echoes' page now says **Shadow every play**.
- Setup shows your actual hotkeys everywhere (it said Ctrl+Alt+1, 2 and G whatever you'd set).
- The last page points to **Settings → Hotkeys**, with a button to go straight there.

### Install
Update from inside Encore (≡ → Help → Update to 0.9.7). ≡ → Help → Run Setup Again to see it.
