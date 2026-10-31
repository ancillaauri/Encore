// Encore interface. Talks only to window.bridge (bridge.js).
(function () {
  const $ = (id) => document.getElementById(id);
  const RING = 603.2; // circumference of the ring (r = 96)
  const VENDOR = { nvidia: 'NVIDIA', amd: 'AMD', intel: 'Intel' };
  const CODEC = { h264: 'H.264', hevc: 'HEVC', av1: 'AV1' };

  const state = {
    settings: null,
    caps: null,
    monitors: [],
    windows: [],
    inputs: [],
    source: localStore('encore.source') === 'window' ? 'window' : 'desktop',
    phase: 'idle', // idle | starting | recording | saving
    recording: null,
    recent: [],
    engineGone: false,
  };

  function localStore(key, value) {
    try {
      if (value === undefined) return localStorage.getItem(key);
      localStorage.setItem(key, value);
    } catch (e) { /* storage unavailable: not important */ }
    return null;
  }

  // ---------- messages ----------
  function toast(message, kind = 'info', ms = 6000, action) {
    const el = document.createElement('div');
    el.className = `toast ${kind}`;
    el.textContent = message;
    if (action) {
      const b = document.createElement('button');
      b.className = 'btn sm';
      b.textContent = action.label;
      b.onclick = action.run;
      el.appendChild(document.createElement('br'));
      el.appendChild(b);
    }
    $('toasts').appendChild(el);
    if (ms) setTimeout(() => el.remove(), ms);
    el.addEventListener('click', (e) => { if (e.target === el) el.remove(); });
  }

  // ---------- formatting ----------
  function clock(ms) {
    const s = Math.floor(ms / 1000);
    const h = Math.floor(s / 3600), m = Math.floor((s % 3600) / 60), r = s % 60;
    return h ? `${h}:${String(m).padStart(2, '0')}:${String(r).padStart(2, '0')}` : `${m}:${String(r).padStart(2, '0')}`;
  }
  function size(bytes) {
    if (bytes >= 1e9) return `${(bytes / 1e9).toFixed(2)} GB`;
    return `${Math.max(0.1, bytes / 1e6).toFixed(1)} MB`;
  }
  function fileName(path) { return path.split(/[\\/]/).pop(); }

  // ---------- what will be / is being recorded ----------
  function chosenMonitor() {
    const want = state.settings ? state.settings.monitor : 'primary';
    if (want.startsWith('id:')) return state.monitors.find((m) => `id:${m.id}` === want);
    return state.monitors.find((m) => m.primary) || state.monitors[0];
  }

  function describeTarget() {
    if (state.source === 'window') {
      const w = state.windows.find((x) => String(x.handle) === $('window').value);
      return w ? `Records the window “${w.title}”` : 'Choose a window to record';
    }
    const want = state.settings ? state.settings.monitor : 'primary';
    if (want === 'mouse') return 'Records the monitor your mouse is on';
    const m = chosenMonitor();
    return m ? `Records ${m.name}${m.primary ? ' (primary)' : ''}` : 'Records your primary monitor';
  }

  function renderStage() {
    const body = document.body;
    body.classList.toggle('recording', state.phase === 'recording');
    body.classList.toggle('saving', state.phase === 'saving');
    const rec = $('record');
    rec.disabled = state.phase === 'starting' || state.phase === 'saving' || state.engineGone;
    const label = { idle: 'Start recording', starting: 'Starting…', recording: 'Stop and save', saving: 'Saving…' }[state.phase];
    $('record-label').textContent = label;
    rec.setAttribute('aria-label', label);

    const tracks = state.phase === 'recording' && state.recording ? state.recording.audio_tracks || [] : [];
    $('tracks').hidden = !tracks.length;
    $('tracks').textContent = tracks.length ? `Audio tracks: ${tracks.join(', ')}` : '';

    if (state.phase === 'recording' && state.recording) {
      const r = state.recording;
      $('what').textContent = `Recording ${r.source.replace(/^Desktop: /, '').replace(/^Window: /, '')}`;
      $('detail').textContent = `${r.width}×${r.height}, ${r.fps} fps, ${encoderLabel(r.encoder)}${r.pipeline === 'download' ? ' (copy path)' : ''}`;
    } else if (state.phase === 'saving') {
      $('what').textContent = 'Saving your recording';
      $('detail').textContent = 'Joining the pieces into one file.';
    } else {
      $('what').textContent = describeTarget();
      $('detail').textContent = state.settings ? `Saved to ${state.settings.output_dir}` : '';
      $('timer').textContent = '0:00';
      $('ring-sweep').style.strokeDashoffset = RING;
    }
    // Source and settings are locked while recording or saving.
    const locked = state.phase !== 'idle';
    document.querySelectorAll('.seg button').forEach((b) => (b.disabled = locked));
    $('refresh-windows').disabled = locked;
  }

  function encoderLabel(name) {
    const e = state.caps && state.caps.encoders.find((x) => x.name === name);
    return e ? `${VENDOR[e.vendor]} ${CODEC[e.codec]}` : name;
  }

  function tick() {
    if (state.phase !== 'recording' || !state.recording) return;
    const ms = Date.now() - state.recording.started_unix_ms;
    $('timer').textContent = clock(ms);
    // The ring sweeps once per minute.
    const frac = (ms % 60000) / 60000;
    $('ring-sweep').style.strokeDashoffset = RING * (1 - frac);
  }
  setInterval(tick, 250);

  // ---------- settings panel ----------
  function renderSource() {
    document.querySelectorAll('.seg button').forEach((b) => {
      b.setAttribute('aria-checked', String(b.dataset.source === state.source));
    });
    $('monitor-row').hidden = state.source !== 'desktop';
    $('window-row').hidden = state.source !== 'window';
    $('window-hint').hidden = state.source !== 'window';
  }

  function renderMonitors() {
    const sel = $('monitor');
    sel.textContent = '';
    const add = (value, text) => { const o = document.createElement('option'); o.value = value; o.textContent = text; sel.appendChild(o); };
    add('primary', 'Primary monitor');
    add('mouse', 'Monitor with the mouse');
    for (const m of state.monitors) add(`id:${m.id}`, `${m.name} (${m.width}×${m.height})${m.primary ? ', primary' : ''}`);
    const want = state.settings ? state.settings.monitor : 'primary';
    if (want.startsWith('id:') && !state.monitors.some((m) => `id:${m.id}` === want)) add(want, 'Saved monitor (not connected)');
    sel.value = want;
  }

  function renderWindows() {
    const sel = $('window');
    const keep = sel.value;
    sel.textContent = '';
    if (!state.windows.length) {
      const o = document.createElement('option');
      o.value = '';
      o.textContent = 'No windows found';
      sel.appendChild(o);
      return;
    }
    for (const w of state.windows) {
      const o = document.createElement('option');
      o.value = String(w.handle);
      o.textContent = w.exe ? `${w.title} (${w.exe})` : w.title;
      sel.appendChild(o);
    }
    if (state.windows.some((w) => String(w.handle) === keep)) sel.value = keep;
  }

  function renderSettings() {
    const s = state.settings;
    if (!s) return;
    $('codec').value = s.codec;
    $('fps').value = String(s.fps);
    $('resolution').value = s.resolution;
    $('quality').value = s.quality;
    $('cursor').checked = !!s.capture_cursor;
    $('output').value = s.output_dir;
    $('audio-full').checked = !!s.audio_full_mix;
    $('audio-mic').checked = !!s.audio_mic;
    $('audio-apps').checked = !!s.audio_app_tracks;
    renderInputs();
    renderMonitors();
    renderCodecs();
  }

  function renderInputs() {
    const sel = $('mic-device');
    sel.textContent = '';
    const add = (value, text) => { const o = document.createElement('option'); o.value = value; o.textContent = text; sel.appendChild(o); };
    const def = state.inputs.find((d) => d.default);
    add('default', def ? `Windows default (${def.name})` : 'Windows default');
    for (const d of state.inputs) add(d.id, d.name);
    const want = state.settings ? state.settings.mic_device : 'default';
    if (want !== 'default' && !state.inputs.some((d) => d.id === want)) add(want, 'Saved microphone (not connected)');
    sel.value = want;
    $('mic-row').hidden = !(state.settings && state.settings.audio_mic);
  }

  async function refreshInputs() {
    try { state.inputs = await bridge.listAudioInputs(); } catch (e) { state.inputs = []; }
    renderInputs();
  }

  function renderCodecs() {
    if (!state.caps || !state.caps.probed) return;
    for (const o of $('codec').options) {
      const ok = state.caps.encoders.some((e) => e.codec === o.value);
      o.disabled = !ok;
      o.textContent = o.textContent.replace(/ – not supported by your graphics card$/, '') + (ok ? '' : ' – not supported by your graphics card');
    }
  }

  function renderEngineLine() {
    const line = $('engine-line');
    line.classList.remove('bad');
    if (state.engineGone) {
      line.textContent = 'The recorder stopped. Close and reopen Encore.';
      line.classList.add('bad');
      return;
    }
    const c = state.caps;
    if (!c || !c.probed) { line.textContent = 'Checking your graphics card…'; return; }
    if (!c.ffmpeg_found) { line.textContent = 'FFmpeg is missing. Reinstall Encore.'; line.classList.add('bad'); return; }
    if (!c.capture_supported) { line.textContent = 'Screen capture is unavailable in this build.'; line.classList.add('bad'); return; }
    if (!c.encoders.length) { line.textContent = 'No graphics card encoder found. Try updating your graphics driver.'; line.classList.add('bad'); return; }
    const vendors = [...new Set(c.encoders.map((e) => VENDOR[e.vendor]))];
    line.textContent = `Encoding on your ${vendors.join(' and ')} graphics card`;
  }

  async function saveSetting(key, value) {
    try {
      state.settings = await bridge.setSettings({ [key]: value });
      if (key === 'audio_mic') renderInputs();
      renderStage();
    } catch (e) {
      toast(String(e), 'error');
      renderSettings();
    }
  }

  async function refreshWindows() {
    try {
      state.windows = await bridge.listWindows();
    } catch (e) { state.windows = []; }
    renderWindows();
    renderStage();
  }

  async function refreshMonitors() {
    try { state.monitors = await bridge.listMonitors(); } catch (e) { state.monitors = []; }
    renderMonitors();
    renderStage();
  }

  // ---------- record / stop ----------
  async function toggleRecording() {
    if (state.phase === 'idle') {
      let source = { source: 'desktop' };
      if (state.source === 'window') {
        const handle = Number($('window').value);
        if (!handle) { toast('Choose a window to record first.', 'warning'); return; }
        source = { source: 'window', handle };
      }
      state.phase = 'starting';
      renderStage();
      try {
        state.recording = await bridge.startRecording(source);
        state.phase = 'recording';
      } catch (e) {
        state.phase = 'idle';
        toast(String(e), 'error', 15000);
      }
      renderStage();
      tick();
    } else if (state.phase === 'recording') {
      state.phase = 'saving';
      renderStage();
      try {
        await bridge.stopRecording();
      } catch (e) {
        state.phase = 'idle';
        toast(String(e), 'error');
        renderStage();
      }
      // The saved / error event finishes the job.
    }
  }

  function addRecent(d) {
    state.recent.unshift(d);
    const ul = $('recent');
    const li = document.createElement('li');
    const name = document.createElement('span');
    name.className = 'file';
    name.textContent = fileName(d.path);
    name.title = d.path;
    if (d.recovered) {
      const t = document.createElement('span');
      t.className = 'tag';
      t.textContent = 'recovered';
      name.appendChild(t);
    }
    const meta = document.createElement('span');
    meta.className = 'meta';
    meta.textContent = d.duration_ms ? `${clock(d.duration_ms)}, ${size(d.size_bytes)}` : size(d.size_bytes);
    const btn = document.createElement('button');
    btn.className = 'btn sm';
    btn.textContent = 'Show in folder';
    btn.onclick = () => bridge.revealFile(d.path).catch((e) => toast(String(e), 'error'));
    li.append(name, meta, btn);
    ul.prepend(li);
    $('recent-wrap').hidden = false;
  }

  // ---------- engine events ----------
  bridge.onEvent((event, data) => {
    switch (event) {
      case 'capabilities':
        state.caps = data;
        renderEngineLine();
        renderCodecs();
        break;
      case 'recording_started':
        state.recording = data;
        state.phase = 'recording';
        renderStage();
        break;
      case 'recording_saved':
        state.phase = 'idle';
        state.recording = null;
        addRecent(data);
        toast(data.recovered ? `Recovered an interrupted recording: ${fileName(data.path)}` : `Saved ${fileName(data.path)}`, 'ok', 5000);
        renderStage();
        break;
      case 'status':
        if (data.state === 'saving' && state.phase === 'recording') state.phase = 'saving';
        if (data.state === 'idle' && state.phase === 'saving') state.phase = 'idle';
        renderStage();
        break;
      case 'audio_app_added':
        if (state.recording) {
          const list = state.recording.audio_tracks || (state.recording.audio_tracks = []);
          if (!list.includes(data.title)) list.push(data.title);
          renderStage();
        }
        break;
      case 'warning':
        toast(data.message, 'warning', 12000);
        break;
      case 'error':
        if (state.phase === 'saving') state.phase = 'idle';
        toast(data.message, 'error', 20000);
        renderStage();
        break;
      case 'engine_exited':
        state.engineGone = true;
        state.phase = 'idle';
        renderEngineLine();
        renderStage();
        break;
    }
  });

  // ---------- wiring ----------
  $('record').addEventListener('click', toggleRecording);
  document.querySelectorAll('.seg button').forEach((b) =>
    b.addEventListener('click', () => {
      state.source = b.dataset.source;
      localStore('encore.source', state.source);
      renderSource();
      if (state.source === 'window') refreshWindows();
      renderStage();
    })
  );
  $('refresh-windows').addEventListener('click', refreshWindows);
  $('window').addEventListener('change', renderStage);
  $('monitor').addEventListener('change', (e) => saveSetting('monitor', e.target.value));
  $('mic-device').addEventListener('change', (e) => saveSetting('mic_device', e.target.value));
  document.querySelectorAll('[data-setting]').forEach((el) => {
    if (el.id === 'output') {
      el.addEventListener('change', () => saveSetting('output_dir', el.value.trim()));
    } else if (el.type === 'checkbox') {
      el.addEventListener('change', () => saveSetting(el.dataset.setting, el.checked));
    } else {
      el.addEventListener('change', () =>
        saveSetting(el.dataset.setting, 'number' in el.dataset ? Number(el.value) : el.value)
      );
    }
  });
  $('open-output').addEventListener('click', () => {
    if (state.settings) bridge.openFolder(state.settings.output_dir).catch((e) => toast(String(e), 'error'));
  });
  window.addEventListener('focus', () => { if (state.phase === 'idle') { refreshMonitors(); refreshInputs(); } });

  // ---------- updates ----------
  function showUpdate(info) {
    if (!info) return;
    const b = $('update');
    b.hidden = false;
    b.disabled = false;
    b.textContent = 'Update ready';
    b.title = `Install Encore ${info.version} and restart`;
  }
  bridge.onUpdateReady(showUpdate);
  $('update').addEventListener('click', async () => {
    if (state.phase !== 'idle') {
      toast('Stop the recording first, then install the update.', 'warning');
      return;
    }
    const b = $('update');
    b.disabled = true;
    b.textContent = 'Installing…';
    try {
      await bridge.installUpdate(); // Encore closes and reopens on the new version
    } catch (e) {
      b.disabled = false;
      b.textContent = 'Update ready';
      toast(String(e), 'error', 15000);
    }
  });

  // ---------- start-up ----------
  async function init() {
    renderSource();
    renderStage();
    try {
      state.settings = await bridge.getSettings();
      renderSettings();
      const status = await bridge.getStatus();
      if (status.state === 'recording') {
        state.recording = { ...status.recording, source: status.recording.label, fps: '' };
        state.phase = 'recording';
      }
    } catch (e) {
      toast(`Could not reach the recorder: ${e}`, 'error', 0);
    }
    await refreshMonitors();
    await refreshInputs();
    if (state.source === 'window') await refreshWindows();
    // Capabilities arrive as an event, but may have been sent before we were listening.
    for (let i = 0; i < 40 && !(state.caps && state.caps.probed); i++) {
      try { state.caps = await bridge.getCapabilities(); } catch (e) { /* engine still starting */ }
      renderEngineLine();
      if (state.caps && state.caps.probed) break;
      await new Promise((r) => setTimeout(r, 500));
    }
    renderCodecs();
    renderStage();
    try { showUpdate(await bridge.getUpdate()); } catch (e) { /* no update info yet */ }
  }
  init();
})();
