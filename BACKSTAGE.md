# Encore's files, for Backstage

What Encore writes into every clip it saves, so Backstage can read it. Encore's side of the
format is in ARCHITECTURE.md ("File contract"); this page is the short version, kept up to date
whenever a tag is added.

## Reading the tags

They're file-level MP4 tags. Read them with ffprobe:

```
ffprobe -v error -show_entries format_tags -of json "clip.mp4"
```

## The tags (from Encore 0.9.36)

| Tag | Value | Example |
|---|---|---|
| `comment` | The audio layout version (unchanged) | `rec-format:2` |
| `encore_kind` | `replay` or `recording` | `replay` |
| `encore_source` | What was captured: `game`, `screen` or `app` (Select Application) | `game` |
| `encore_game` | The game's name, when the clip is of a game | `Valorant` |
| `encore_game_exe` | The game's program, when the clip is of a game. Stays the same when the display name changes, so it's the steadier way to group | `VALORANT-Win64-Shipping.exe` |
| `encore_id` | Unique to this clip (a version 4 UUID). Follows the clip through renames and moves | `0f8c2b9e-41d3-4a7e-9c1b-5e2d7f9a3c10` |
| `encore_version` | The Encore version that wrote the file | `0.9.36` |
| `creation_time` | When the clip starts, UTC (the standard MP4 field) | `2026-11-04T08:07:00.250000Z` |

`encore_game` is there for a game Encore recorded, and for a screen recording saved while a
game was open (the clips Encore already files in that game's folder; `encore_source` is
`screen` for those). A game recorded through Select Application is `app` with no game, because
Encore only knows it's a game when game detection found it.

## When a tag is missing

- **Files from before 0.9.36** have only `comment`. Show them under Recordings.
- **A file recovered after a crash** has only `encore_id` and `encore_version`: Encore can't tell
  what it was. Show it under Recordings too.
- Any other tag can be missing when Encore didn't know it. Treat missing as unknown, never as
  an error.

## Rules Encore keeps

1. Every tag of Encore's own starts with `encore_`.
2. Values are plain text on one line, at most 200 characters (game names are cleaned of line
   breaks and odd characters).
3. A tag's meaning never changes. New information gets a new tag.
4. Encore may add tags in any release: ignore any you don't know.
5. `rec-format` only goes up if the file's layout changes (tracks, their order or names), and
   that's always announced first. Adding tags doesn't change it.

## Files being saved (from Encore 0.9.38)

Encore writes each clip in a hidden folder, `.saving`, at the top of its recordings folder
(`<Encore folder>\.saving\<game>\<name>.mp4`), and moves it into its game's folder only
once it's complete. A clip in a game's folder is always whole: no empty or half-written files.
Skip `.saving` and everything in it. It can hold a file for a moment, or one left by a save
that never finished (Encore removes those when it next starts).

## Silent app tracks (from Encore 0.9.38)

A replay leaves out an app's track if every sample in the saved moment is exactly zero. No
threshold, so one non-zero sample keeps the track. Full mix and Desktop audio don't change,
since they're plain sums. The one exception: if apps were the only sound (no Full System Audio,
no microphone) and all were silent, they're kept, so the file still has audio. Clips saved
before 0.9.38 can still have silent app tracks.

## Kept for later (the names are taken)

- `encore_markers`: moments marked with a hotkey while playing.
- `encore_monitor`: which monitor a screen recording was of.
