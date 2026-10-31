## Encore 0.2.1, test build 2: audio and new icon

### What's new
- **New icon**: a glowing E over a waveform, in the same style as the companion clip editor.
- **Sound.** Every recording now has separate audio tracks:
  - **Full mix**: everything you hear (the track players use by default)
  - **Microphone**
  - **One track per app** that plays sound while you record (your game, Spotify, your browser,
    Discord...), named after the app. Apps that start partway through still get a full-length track.
- New **Audio** settings: switch each of those on or off, and pick which microphone.
- The tracks being recorded are listed under the timer while you record.
- Audio is lined up with the video using the same clock Windows uses for both.

### Not in this build yet
Hotkeys, tray icon, instant replay, game detection, switching monitor mid-recording.

### Known limits
- Browsers are one track: YouTube in Chrome is the "Google Chrome" track (all tabs together).
- Discord is one track (not one per person).
- More than 8 apps playing sound: the extra ones are only in the Full mix.
- Changing your speakers/headphones mid-recording isn't handled yet.
- Saving takes a little longer than before, because the audio tracks are encoded when you stop.

### Install
Download `Encore_0.2.1_x64-setup.exe` below and run it (it updates earlier versions in place). Windows may say
"Windows protected your PC": click **More info**, then **Run anyway**.

### Test checklist (about 15 minutes)
1. **Settings.** Open Encore. Under **Audio**, check the microphone list shows your mic.
2. **Sync test.** Record the desktop for about 20 seconds while doing two things:
   - Play a YouTube video with obvious sharp sounds you can see happen (someone clapping, a drum hit,
     a door slamming).
   - Open Notepad and type a few words, so your microphone picks up the key clicks.
   Stop, play the file and check: does the YouTube sound land exactly on its picture, and do the
   key clicks line up with the letters appearing? If anything is off, say roughly by how much and
   which way (sound early or late).
3. **Separate tracks.** Play music in Spotify (or any music app), have a YouTube video open in your
   browser, and play your game. Record 1–2 minutes. While recording, the track list under the timer
   should grow as each app makes sound. Send the file to me or open it in a player that lists audio
   tracks (VLC: Audio → Audio Track) and check each track only has that app's sound.
4. **Microphone.** In the same recording, talk for a few seconds. The Microphone track should have
   only your voice.
5. **Switches.** Turn off "Separate track for each app", record 10 seconds: only Full mix and
   Microphone should be in the file. Then turn it back on.
6. **Still from test build 1** (not done yet): record a game window and alt-tab; note game FPS with
   and without recording; close Encore mid-recording and check the recording was saved on reopening.

### What to send back
For each step: worked / didn't work, plus any message Encore showed. If something failed, also send
`%LOCALAPPDATA%\Encore\logs\engine.log` (paste that path into the File Explorer address bar).
A short test recording file is also very useful: I can measure sync and check each track from it.
