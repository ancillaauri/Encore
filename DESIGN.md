# Encore's design

Encore's look, written down so anyone building next to it (the companion clip editor included)
can follow it. Status: redesigned on 2026-11-02 (purple accent, our own switch and slider, squarer
dialogs). This file changes first when the look changes; `ui/style.css` holds every value.

## Direction

Dark, quiet surfaces, with colour used as light: a soft neon glow on what's switched on or
being pointed at, nothing glowing for decoration. Away from the Discord look Encore started
with. The layout stays: custom title bar, Main Menu (≡), a 260 px sidebar on the left, the main
screen, dialogs. **The backgrounds stay as they are** (dark grey, black sidebar): the purple
replaces the old blue accent only, wherever it was used (links, main buttons, selection,
focus on text boxes, ticks), and doesn't spread to anything else.

Shapes are squarer than Discord's: dialogs 4 px corners with a 2 px purple line
along the top, buttons and text boxes 3–4 px, switches and sliders our own (being chosen:
chosen: switch A "Square slide", slider A "Bar handle").

Ideas kept for later: a diamond slider handle, if the look ever takes on more diamond shapes.

## Colours

| Token | Value | Use |
|---|---|---|
| `--bg` | `#0a090c` | Window background |
| `--side` | `#000000` | Sidebar |
| `--panel` / `--panel2` | `#1a191c` / `#242327` | Cards, dropdowns |
| `--line` / `--line2` | `#1f1e22` / `#2e2d31` | Dividers, outlines |
| `--text` / `--mute` / `--dim` | `#e2e1e4` / `#949398` / `#6b6a6f` | Text, secondary, faint |
| `--acc` | `#9B6BFF` (was #5865f2) | Purple: the accent. Main buttons, switches on, sliders, selection |
| `--acc-hover` | `#B08BFF` | Accent, hovered |
| `--acc-press` | `#7E52E6` | Accent, pressed; the hotkey half of Save an Echo |
| `--acc-text` | `#D9C9FF` | Accent-coloured text on dark (values, the BETA pill) |
| `--glow` | `rgba(155,107,255, .35–.95)` | Glow around anything in the accent |

Colours that keep their own meaning and are **not** replaced by the accent:

| Colour | Value | Use |
|---|---|---|
| Microphone on | `#6fdc8c` (glow `rgba(59,199,93,…)`) | Microphone icon switch, on |
| Microphone off / an icon switch off | `#ff7a7e` (glow `rgba(255,92,97,…)`) | Off state of icon switches |
| Record | `#f23f43` | The record button |
| Warning | `#f0b232` | Warnings, the microphone-missing icon |
| Good | `#23a55a` | "Echoes on" dot, success |
| Keyboard focus | `#00a8fc` | Focus rings (never removed) |

## Glow recipes

- **Icon switch, on:** stroke icon in a light tint of its colour, `filter: drop-shadow(0 0 2px
  <colour .95>) drop-shadow(0 0 6px <colour .5>)`, on a tile with a 10 % fill and a 1 px
  inner line at 22–28 %.
- **On/off switch (Square slide):** 40 × 20, 3 px corners, a dark track with a 1 px outline
  and a 16 × 14 block. Off: grey outline and block. On: the block slides right, purple with an
  8 px glow, outline purple at 60 %. Hovered: one step brighter in both states. Locked: 40 %.
- **Slider (Bar handle):** a 2 px line, purple where filled; the handle is an 8 × 22 bar in
  `--acc-hover` with a glow, growing to 10 × 24 and near-white while hovered or dragged (easy to
  grab). Marks under it where values matter (bitrate: every 5 Mbps, numbers at 5, 50, 100, 150).
- **Record button**: the red dot glows (`box-shadow: 0 0 16px rgba(242,63,67,.6)`) instead of flat red.
- **Main button:** accent fill, 10 px glow; hovered: `--acc-hover` and a 16 px glow; pressed:
  `--acc-press` and a 22 px glow.

## Rules

1. **Everything you can click brightens when hovered** (buttons, links, switches, tiles, menu
   items, cards), and every button looks a little more lit while pressed. No exceptions, now or
   later.
2. Glow means "on", "selected" or "pointed at". Off things don't glow (or only faintly, so a
   switch still reads as a switch).
3. Never colour alone: every state also has words (tooltips say "On"/"Off"), and colours that
   must be told apart differ in lightness too.
4. Text is in UK English and sentence case. Buttons say what they do ("Save an Echo",
   "Restart to update").

## Waiting for you (the sidebar's inbox)

Above Priority Capture and Do not disturb: small cards for anything that needs the person,
staying until dealt with: an update ready (Restart to update / Later; this replaces the update
button in the title bar), a reply to a problem report (Read / Mark as read), a microphone not
plugged in, a game that recorded black, recordings falling behind. Nothing waiting: the area
isn't there. Passing messages ("Settings saved") stay at the bottom of the window.

## Where to see it

The mock-ups: the "Encore redesign" design canvas (main screen, every control state,
Settings). Once built, the app itself is the reference: `ui/style.css` holds every value.
