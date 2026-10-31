// The only file in the interface that knows which shell it runs in (see ARCHITECTURE.md).
// Swapping Tauri for Electron means rewriting this file only.
(function () {
  const T = window.__TAURI__;
  const call = (cmd, args) => T.core.invoke('engine_call', { cmd, args: args ?? null });

  window.bridge = {
    startRecording: (source) => call('start_recording', source),
    stopRecording: () => call('stop_recording'),
    startReplay: () => call('start_replay'),
    stopReplay: () => call('stop_replay'),
    saveReplay: () => call('save_replay'),
    getStatus: () => call('get_status'),
    getSettings: () => call('get_settings'),
    setSettings: (patch) => call('set_settings', patch),
    getCapabilities: () => call('get_capabilities'),
    listAudioApps: () => call('list_audio_apps'),
    listAudioInputs: () => call('list_audio_inputs'),
    listMonitors: () => call('list_monitors'),
    listWindows: () => call('list_windows'),
    openFolder: (path) => T.core.invoke('open_folder', { path }),
    revealFile: (path) => T.core.invoke('reveal_in_folder', { path }),
    getUpdate: () => T.core.invoke('get_update'),
    installUpdate: () => T.core.invoke('install_update'),
    onUpdateReady: (callback) => T.event.listen('update-ready', (e) => callback(e.payload || {})),
    onEvent: (callback) =>
      T.event.listen('engine-event', (e) => callback(e.payload.event, e.payload.data || {})),
  };
})();
