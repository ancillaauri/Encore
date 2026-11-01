## Encore 0.7.2 (beta), test build 10: the stutter fix, take two

### What's changed
- **Game stutter.** Running as administrator in 0.7.1 still showed the warning. Windows was
  refusing because Encore asked for graphics priority a split second too early, before the
  recording had opened the graphics card. Encore now asks again until Windows accepts (it tries for
  up to 10 seconds).
- When Windows' "Hardware-accelerated GPU scheduling" is on, Encore asks for high priority rather
  than realtime, which can make the whole PC stutter in that mode.

### Install
Update from inside Encore (≡ → Help → Update to 0.7.2). Otherwise download
`Encore_0.7.2_x64-setup.exe` below and run it. Windows may say "Windows protected your PC": click
**More info**, then **Run anyway**.

### Test checklist
1. After updating, choose ≡ → File → **Restart as Administrator** and say yes to Windows.
2. Record 20 seconds of uncapped Warframe (V-Sync off). It should be smooth, and no warning
   should appear.
3. Send the clip and `%LOCALAPPDATA%\Encore\logs\engine.log`. Look for a line saying
   **GPU priority realtime** or **high**, "after … ms".
4. Without admin (quit and open normally): starting a recording should show the warning once,
   about 10 seconds in.

### What to send back
Worked / didn't work for each step, the clip, and the log.
