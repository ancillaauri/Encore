// The only file in the interface that knows which shell it runs in (see ARCHITECTURE.md).
// Swapping Tauri for Electron means rewriting this file only.
(function () {
  const T = window.__TAURI__;
  const call = (cmd, args) => T.core.invoke('engine_call', { cmd, args: args ?? null });

  window.bridge = {
    startRecording: (source) => call('start_recording', source),
    stopRecording: () => call('stop_recording'),
    startReplay: (source) => call('start_replay', source),
    stopReplay: () => call('stop_replay'),
    // seconds: how much to save (the whole replay length if left out)
    saveReplay: (seconds) => call('save_replay', seconds ? { seconds } : {}),
    getStatus: () => call('get_status'),
    getSettings: () => call('get_settings'),
    setSettings: (patch) => call('set_settings', patch),
    getCapabilities: () => call('get_capabilities'),
    reportDetails: () => call('report_details'),
    // A line in Encore's log (engine.log), for things only the interface knows about.
    note: (message) => call('note', { message }),
    listAudioInputs: () => call('list_audio_inputs'),
    listMonitors: () => call('list_monitors'),
    listWindows: () => call('list_windows'),
    // Game detection
    lockGame: (pid) => call('lock_game', { pid: pid ?? null }),
    lockFocusedGame: () => call('lock_focused_game'),
    nextGame: () => call('next_game'),
    setTrayGames: (on, games, active, locked, recording) => T.core.invoke('set_tray_games', { on, games, active: active ?? null, locked: locked ?? null, recording }),
    onTrayGame: (callback) => T.event.listen('tray-game', (e) => callback(e.payload)),
    // A notification over the game: device = a screen's Windows name, or null for the screen in use
    // (window = the game's window, whose screen is used; else the window in front);
    // pos = tl, tc, tr, ml, mr, bl, bc or br; item = {kind, title, text, seconds}
    showNotification: (device, window, pos, item) => T.core.invoke('show_notification', { device: device || null, window: window || null, pos, item }),
    openFolder: (path) => T.core.invoke('open_folder', { path }),
    revealFile: (path) => T.core.invoke('reveal_in_folder', { path }),
    // Saved clips: rename (returns {path}) or send to the Recycle Bin.
    renameClip: (path, name) => call('rename_clip', { path, name }),
    deleteClip: (path) => call('delete_clip', { path }),
    openLink: (url) => T.core.invoke('open_link', { url }),
    pickFolder: (start) => T.core.invoke('pick_folder', { start: start || null }),
    locateClipEditor: () => T.core.invoke('locate_clip_editor'),
    pickClipEditor: () => T.core.invoke('pick_clip_editor'),
    openInEditor: (exe, path) => T.core.invoke('open_in_editor', { exe, path }),
    openInPlayer: (path) => T.core.invoke('open_in_player', { path }),
    // Tray, hotkeys and quitting (the shell owns these)
    setRecordingState: (recording, label) => T.core.invoke('set_recording_state', { recording, label: label || null }),
    quitApp: () => T.core.invoke('quit_app'),
    restartAsAdmin: () => T.core.invoke('restart_as_admin'),
    // { elevated, start_as_admin }
    adminStatus: () => T.core.invoke('admin_status'),
    setStartAsAdmin: (on) => T.core.invoke('set_start_as_admin', { on }),
    autostartStatus: () => T.core.invoke('autostart_status'),
    setAutostart: (on) => T.core.invoke('set_autostart', { on }),
    applyShellSettings: () => T.core.invoke('apply_shell_settings'),
    onHotkey: (callback) => T.event.listen('hotkey', (e) => callback(e.payload)),
    appVersion: () => T.app.getVersion(),
    // Updates: { status: 'none' | 'available' | 'ready', version, current }
    getUpdate: () => T.core.invoke('get_update'),
    checkForUpdates: () => T.core.invoke('check_for_updates'),
    installUpdate: () => T.core.invoke('install_update'),
    onUpdateStatus: (callback) => T.event.listen('update-status', (e) => callback(e.payload || {})),
    onUpdateProgress: (callback) => T.event.listen('update-progress', (e) => callback(e.payload || {})),
    // The window (Encore draws its own title bar and window buttons)
    win: (() => {
      const w = () => T.window.getCurrentWindow();
      return {
        minimize: () => w().minimize(),
        toggleMaximize: () => w().toggleMaximize(),
        close: () => w().close(),
        isMaximized: () => w().isMaximized(),
        onResized: (callback) => w().onResized(() => callback()),
        onFocusChanged: (callback) => w().onFocusChanged((e) => callback(!!e.payload)),
        // Flashes Encore's taskbar button until it's clicked (1: critical).
        requestAttention: async () => w().requestUserAttention(1),
      };
    })(),
    onEvent: (callback) =>
      T.event.listen('engine-event', (e) => callback(e.payload.event, e.payload.data || {})),
  };
})();
