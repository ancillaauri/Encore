## Encore 0.9.23 (beta), test build 44: your microphone, found again

### What's new
- **Your microphone is found by name** if Windows gives it a new id after a driver update or a
  different USB port, so it doesn't silently drop out of your recordings.
- **A warning when your microphone isn't plugged in**: a yellow icon next to Microphone in the
  sidebar, and a notification. Click the icon to use another microphone.
- When you plug it back in, Echoes pick it up again by themselves.

### Install
Update from inside Encore (Main Menu → Help → Update to 0.9.23).

## Encore 0.9.22 (beta), test build 43: small fixes

### Fixed
- With Desktop Capture off, the note under the screens no longer says the highlighted screen is
  used again when the game closes. It says Encore waits for the next game.
- Removed a line under Settings → Audio that repeated what Split Application Audio says.
- The **Echo** tag on saved clips is now purple.

### Install
Update from inside Encore (Main Menu → Help → Update to 0.9.22).

## Encore 0.9.21 (beta), test build 42: menu and sidebar tidy-up

### Changed
- The ≡ menu is now the **Main Menu**, with **Settings** at the top instead of under File.
- The left panel is the **Sidebar** (View → Sidebar, Ctrl+B), headed **Capture**.
- **Priority Capture can be hidden from the sidebar**: Settings → Advanced, or Main Menu → View.

### Install
Update from inside Encore (Main Menu → Help → Update to 0.9.21).

## Encore 0.9.20 (beta), test build 41: tidy up your clips

### What's new
- **Rename and delete clips** under Saved this session: right-click a clip, or use the small bin
  in its corner. A deleted clip dims with an **Undo** button and a 5-second timer; delete several
  at once, each with its own timer. Deleted clips go to the Recycle Bin.
- **"Saving an Echo…"** appears the moment you press the hotkey and turns into "Echo saved" when
  the file is ready.

### Fixed
- The side panel's scrollbar no longer runs over Priority Capture and Do not disturb.

### Install
Update from inside Encore (≡ → Help → Update to 0.9.20).

## Encore 0.9.19 (beta), test build 40: sharper video for the same space

### Changed
- **NVIDIA graphics cards: better picture for the same file size,** especially on Medium and Low.
  Encore now aims for a quality level, with each preset's bitrate as the most it can use, and
  spends more of it where your eyes notice (dark areas, edges, movement). Low no longer goes
  smeary between sharp moments.
- **Quality shows its bitrate:** each option shows what it uses at your resolution and frame rate
  (for example "Ultra: 66 Mbps"), with an (i) explaining it. Settings shows the size per minute,
  and changing frame rate or resolution tells you the new bitrate.

### Install
Update from inside Encore (≡ → Help → Update to 0.9.19).

## Encore 0.9.18 (beta), test build 39: Priority Capture made safe

### Changed
- **Priority Capture is always off when Encore starts,** including when it starts with Windows.
  Turn it on for the session when you need it.
- **It turns itself off** once no game has been detected for a minute, and if the capture hits an
  error while it's on. You're told when it does.
- **Switching it on or off no longer restarts Echoes,** so you keep what's already saved up.

### Install
Update from inside Encore (≡ → Help → Update to 0.9.18). This update also includes 0.9.16 and
0.9.17 (notification size, the Priority Capture switch, and a tidy-up).

## Encore 0.9.17 (beta), test build 38: Priority Capture

### What's new
- **Priority Capture,** at the bottom of the side panel above Do not disturb. It's the same as
  Settings → Advanced (now called Priority Capture there too): it puts the recording first on
  the graphics card, for games that put a heavy load on it or less powerful graphics cards.
  The (i) explains what it does.
- **The first time you turn it on,** Encore explains what it does and asks you to tick "I
  understand". After that it switches on and off straight away.

### Install
Update from inside Encore (≡ → Help → Update to 0.9.17).

## Encore 0.9.16 (beta), test build 37: notification size

### What's new
- **Settings → Notifications → Size:** Normal, 1.25×, 1.5×, 1.75× or 2×. Show a test uses the
  size you pick.

### Install
Update from inside Encore (≡ → Help → Update to 0.9.16).

## Encore 0.9.15 (beta), test build 36: notifications drawn by Encore

### Changed
- **Notifications are now drawn by Encore itself,** not by a web page. They look the same, but
  they no longer need the graphics card, so they appear even when a game is using all of it and
  Top graphics priority is on.

### Install
Update from inside Encore (≡ → Help → Update to 0.9.15).

## Encore 0.9.14 (beta), test build 35: notifications with top priority

### Fixed
- **Notifications didn't appear with Top graphics priority on.** They were drawn but the
  graphics card never got round to showing them while the game and the capture had it busy.
  Encore's own windows now get a higher place on the graphics card (above the game, still below
  the capture), so notifications appear, and their timers run smoothly.

### Install
Update from inside Encore (≡ → Help → Update to 0.9.14).

## Encore 0.9.13 (beta), test build 34: notifications stay on top

### Fixed
- **Notifications hidden behind the game.** While a notification is showing, Encore keeps its
  window on top, so a borderless game can't cover it. If something still covers it, Encore's log
  says which program.
- **The "Saving the last 1 minute…" message drew as a small circle** with squashed text. Plain
  messages now look right.

### Install
Update from inside Encore (≡ → Help → Update to 0.9.13).

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
