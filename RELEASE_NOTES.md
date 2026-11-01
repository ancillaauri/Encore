## Encore 0.7.4 (beta), test build 12: save-length hotkeys

### What's new
- **Save just the bit you want.** New hotkeys for instant replay (Settings → Hotkeys):
  - **Ctrl+Alt+1**: save the last 30 seconds
  - **Ctrl+Alt+2**: save the last minute
  - **Ctrl+Alt+3**: save a custom length you choose (in seconds or minutes, up to 20 minutes)
  - **Ctrl+Alt+S** still saves the whole replay length.
  A save can't be longer than the replay length set in the left panel; if the custom length is
  longer, Settings says so and it saves the whole replay instead. Each hotkey can be changed or
  cleared.
- Turning on "Start Encore as administrator" while Encore already has admin rights now confirms it
  (it used to say nothing).

### Install
Update from inside Encore (≡ → Help → Update to 0.7.4). Encore restarts with admin rights
automatically if "Start Encore as administrator" is on.

### Test checklist
1. Turn instant replay on, play for 2 minutes, then press Ctrl+Alt+1, Ctrl+Alt+2 and Ctrl+Alt+3:
   three files of about 30 seconds, 1 minute and 90 seconds.
2. Set the custom length to 5 minutes with a 2-minute replay: Settings should warn, and Ctrl+Alt+3
   should save 2 minutes.
3. Hotkeys inside a League match (with Encore running as administrator).
4. After updating: Encore should reopen with admin rights and no prompt.

### What to send back
Worked / didn't work for each step, and the log if something failed.
