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
    update: { status: 'none' },
    version: '',
  };

  function localStore(key, value) {
    try {
      if (value === undefined) return localStorage.getItem(key);
      localStorage.setItem(key, value);
    } catch (e) { /* storage unavailable: not important */ }
    return null;
  }

  // ---------- messages ----------
  function toast(message, kind = 'info', ms = 3500, action) {
    if (kind === 'error' && ms && ms < 7000) ms = 7000;
    const box = $('toasts');
    while (box.children.length >= 3) box.firstElementChild.remove();
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
    box.appendChild(el);
    requestAnimationFrame(() => el.classList.add('show'));
    const close = () => { el.classList.remove('show'); setTimeout(() => el.remove(), 200); };
    if (ms) setTimeout(close, ms);
    el.addEventListener('click', (e) => { if (e.target === el) close(); });
  }

  // ---------- icons ----------
  const ICO = {
    check: '<svg viewBox="0 0 24 24"><path d="M5.5 12.5l4.2 4.2 8.8-9.2"/></svg>',
    blank: '<svg viewBox="0 0 24 24"></svg>',
    back: '<svg viewBox="0 0 24 24"><path d="M14.5 6l-6 6 6 6"/></svg>',
    chev: '<svg viewBox="0 0 24 24"><path d="M7 10l5 5 5-5"/></svg>',
  };
  function icoSpan(name, cls) {
    const s = document.createElement('span');
    s.className = `ico ${cls || ''}`;
    s.innerHTML = ICO[name];
    return s;
  }
  function h(tag, cls, text) {
    const e = document.createElement(tag);
    if (cls) e.className = cls;
    if (text !== undefined) e.textContent = text;
    return e;
  }

  // ---------- menus: the app menu and dropdowns share one, like the editor ----------
  // items: { text, hint, check, disabled, run } or { text, sub: [items] } or '-'
  const MENU_BUTTONS = new Set();
  function fillMenu(m, items) {
    m.replaceChildren();
    const checks = items.some((it) => it && it.check !== undefined);
    for (const it of items) {
      if (it === '-') { m.append(h('div', 'msep')); continue; }
      const b = h('button');
      b.type = 'button';
      b.setAttribute('role', 'menuitem');
      if (checks) b.append(icoSpan(it.check ? 'check' : 'blank', 'mcheck'));
      b.append(h('span', '', it.text));
      if (it.hint) b.append(h('span', 'mhint', it.hint));
      if (it.disabled) b.disabled = true;
      if (it.sub) {
        b.append(icoSpan('back', 'msub'));
        b.classList.add('hassub');
        const show = () => { for (const x of m.querySelectorAll('.hassub')) x.classList.toggle('subopen', x === b); openSub(b, it.sub); };
        b.addEventListener('mouseenter', show);
        b.onclick = show;
      } else {
        if (m.id === 'ctxmenu') b.addEventListener('mouseenter', () => { closeSub(); for (const x of m.querySelectorAll('.hassub')) x.classList.remove('subopen'); });
        b.onclick = () => { hideMenu(); it.run(); };
      }
      m.append(b);
    }
  }
  function openMenu(x, y, items, alignRight) {
    closeSub();
    hideTip();
    const m = $('ctxmenu');
    m.dataset.for = '';
    fillMenu(m, items);
    m.hidden = false;
    const r = m.getBoundingClientRect();
    if (alignRight) x -= r.width;
    m.style.left = `${Math.max(8, Math.min(x, innerWidth - r.width - 8))}px`;
    m.style.top = `${Math.max(8, Math.min(y, innerHeight - r.height - 8))}px`;
    return m;
  }
  function openSub(btn, items) {
    let sm = $('ctxsub');
    if (!sm) { sm = h('div'); sm.id = 'ctxsub'; sm.setAttribute('role', 'menu'); document.body.append(sm); }
    fillMenu(sm, items);
    sm.hidden = false;
    const r = btn.getBoundingClientRect(), mr = btn.parentElement.getBoundingClientRect(), w = sm.offsetWidth, ht = sm.offsetHeight;
    const x = mr.right - 2 + w > innerWidth - 8 ? mr.left - w + 2 : mr.right - 2;
    sm.style.left = `${Math.max(8, x)}px`;
    sm.style.top = `${Math.max(8, Math.min(r.top - 5, innerHeight - ht - 8))}px`;
  }
  function closeSub() { const sm = $('ctxsub'); if (sm) sm.hidden = true; }
  function hideMenu() {
    $('ctxmenu').hidden = true;
    closeSub();
    for (const b of MENU_BUTTONS) { b.classList.remove('open'); b.setAttribute('aria-expanded', 'false'); }
  }
  // While a menu is open, a click outside it only closes it.
  document.addEventListener('pointerdown', (e) => {
    const m = $('ctxmenu'), sub = $('ctxsub');
    if (m.hidden || m.contains(e.target) || (sub && sub.contains(e.target))) return;
    hideMenu();
    if (e.button !== 0) return;
    e.stopPropagation();
    const eat = (ev) => { ev.stopPropagation(); ev.preventDefault(); };
    window.addEventListener('click', eat, { capture: true, once: true });
    setTimeout(() => window.removeEventListener('click', eat, true), 600);
  }, true);
  window.addEventListener('blur', hideMenu);
  // A button that opens a menu; clicking it again closes it.
  function menuButton(btn, build, alignRight) {
    MENU_BUTTONS.add(btn);
    btn.addEventListener('click', (e) => {
      e.stopPropagation();
      const m = $('ctxmenu');
      if (!m.hidden && m.dataset.for === btn.id) { hideMenu(); return; }
      const r = btn.getBoundingClientRect();
      openMenu(alignRight ? r.right : r.left, r.bottom + 6, build(), alignRight);
      m.dataset.for = btn.id;
      btn.classList.add('open');
      btn.setAttribute('aria-expanded', 'true');
    });
  }

  // Dropdowns: each <select class="dd"> stays the source of truth (code sets .value and fills its
  // options as before) and is shown as a button that opens the menu.
  function dropdown(sel) {
    const btn = h('button', 'sortbtn');
    btn.type = 'button';
    btn.id = `${sel.id}-dd`;
    btn.setAttribute('aria-haspopup', 'menu');
    const text = h('span', 'ddtext');
    btn.append(text, icoSpan('chev', 'chev'));
    if (!sel.closest('.setrow')) btn.classList.add('full');
    sel.after(btn);
    const sync = () => {
      const o = sel.options[sel.selectedIndex];
      text.textContent = o ? o.textContent : '';
      btn.disabled = sel.disabled || !sel.options.length;
    };
    const desc = Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, 'value');
    Object.defineProperty(sel, 'value', { get() { return desc.get.call(this); }, set(v) { desc.set.call(this, v); sync(); } });
    new MutationObserver(sync).observe(sel, { childList: true, subtree: true, characterData: true, attributes: true });
    menuButton(btn, () => [...sel.options].map((o) => ({
      text: o.textContent,
      check: o.selected,
      disabled: o.disabled,
      run: () => { if (sel.value === o.value) return; sel.value = o.value; sel.dispatchEvent(new Event('change')); },
    })), !!sel.closest('.setrow'));
    // the label still points at the button
    const lab = sel.closest('label');
    if (lab) lab.addEventListener('click', (e) => { if (e.target === lab || !btn.contains(e.target)) e.preventDefault(); });
    sync();
  }
  document.querySelectorAll('select.dd').forEach(dropdown);

  // ---------- tooltips: the app's own controls get the editor's style ----------
  const tip = { el: null, timer: 0, hideTimer: 0 };
  function showTip(el, text, pos) {
    const t = $('tip');
    clearTimeout(tip.hideTimer);
    t.textContent = text || el.dataset.tip;
    t.classList.toggle('rail', pos === 'right');
    t.hidden = false;
    const r = el.getBoundingClientRect(), w = t.offsetWidth, ht = t.offsetHeight;
    let x, y;
    if (pos === 'right') {
      const box = (el.querySelector('.gav') || el).getBoundingClientRect();
      x = box.right + 12; y = box.top + box.height / 2 - ht / 2;
    } else {
      x = r.left + r.width / 2 - w / 2; y = r.top - ht - 8;
      if (y < 4) y = r.bottom + 8; // no room above: show it below
    }
    t.style.left = `${Math.max(6, Math.min(x, innerWidth - w - 6))}px`;
    t.style.top = `${Math.max(4, Math.min(y, innerHeight - ht - 4))}px`;
    t.style.transition = 'none';
    t.classList.remove('on');
    void t.offsetWidth;
    t.style.transition = '';
    t.classList.add('on');
  }
  function hideTip() {
    clearTimeout(tip.timer);
    const t = $('tip');
    const wasRail = t.classList.contains('rail') && t.classList.contains('on');
    tip.el = null;
    t.classList.remove('on');
    clearTimeout(tip.hideTimer);
    if (wasRail) tip.hideTimer = setTimeout(() => { if (!tip.el) t.hidden = true; }, 110);
    else t.hidden = true;
  }
  document.addEventListener('mouseover', (e) => {
    if (!e.target.closest) return;
    // The window buttons keep Windows' own tooltip, as in the editor.
    const titled = e.target.closest('[title]');
    if (titled && titled.matches('button:not(.wc), .ibtn, .btn, .tb-btn')) {
      const tt = titled.getAttribute('title');
      titled.removeAttribute('title');
      if (tt) titled.dataset.tip = tt; else delete titled.dataset.tip;
    }
    const rail = document.body.classList.contains('side-collapsed') && e.target.closest('.side [data-rail-tip]');
    if (rail) {
      if (rail === tip.el) return;
      hideTip();
      tip.el = rail;
      showTip(rail, rail.dataset.railTip, 'right');
      return;
    }
    const el = e.target.closest('[data-tip]');
    if (el === tip.el) return;
    hideTip();
    if (!el) return;
    tip.el = el;
    tip.timer = setTimeout(() => { if (tip.el === el && el.isConnected) showTip(el); }, 250);
  });
  document.addEventListener('pointerdown', hideTip, true);
  window.addEventListener('blur', hideTip);

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
    document.querySelectorAll('.seg button, .railbtn').forEach((b) => (b.disabled = locked));
    $('refresh-windows').disabled = locked;
    renderSummary();
  }

  // One line under the source picker saying how recordings are made, with a link to Settings.
  function renderSummary() {
    const s = state.settings;
    if (!s) return;
    const res = { native: 'Full resolution', '1440p': '1440p', '1080p': '1080p', '720p': '720p' }[s.resolution];
    const audio = [s.audio_full_mix && 'desktop', s.audio_mic && 'mic', s.audio_app_tracks && 'each app'].filter(Boolean);
    $('summary').textContent =
      `${res}, ${s.fps} fps, ${CODEC[s.codec]}, ${s.quality} quality. ` +
      (audio.length ? `Audio: ${audio.join(', ')}.` : 'No audio.');
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
    document.querySelectorAll('.railbtn').forEach((b) => b.classList.toggle('on', b.dataset.source === state.source));
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
      renderUpdate();
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
  document.querySelectorAll('.seg button, .railbtn').forEach((b) =>
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
  function renderUpdate() {
    const u = state.update || { status: 'none' };
    const b = $('update');
    b.hidden = u.status === 'none';
    if (!b.disabled) $('update-text').textContent = u.status === 'ready' ? 'Update ready' : 'Update available';
    b.title = u.version ? `Install Encore ${u.version} and restart` : '';
    $('version-line').textContent = state.version ? `Version ${state.version} (beta)` : 'Beta';
    if (u.status !== 'none') $('update-status').textContent = `Version ${u.version} is available. Use Update at the top to install it.`;
    const mode = state.settings ? state.settings.update_mode : 'manual';
    document.querySelectorAll('[data-update-mode]').forEach((o) =>
      o.setAttribute('aria-checked', String(o.dataset.updateMode === mode)));
  }

  async function checkForUpdates() {
    const btn = $('check-updates');
    btn.disabled = true;
    $('update-status').textContent = 'Checking...';
    try {
      state.update = await bridge.checkForUpdates();
      $('update-status').textContent = state.update.status === 'none'
        ? `You're up to date.`
        : `Version ${state.update.version} is available. Use Update at the top to install it.`;
      if (state.update.status !== 'none') toast(`Encore ${state.update.version} is available.`, 'ok');
      else if (!$('settings').hidden) { /* status line is enough */ } else toast("You're up to date.", 'ok', 4000);
    } catch (e) {
      $('update-status').textContent = 'Could not check for updates.';
      toast(`Could not check for updates: ${e}`, 'error', 10000);
    }
    btn.disabled = false;
    renderUpdate();
  }

  async function installUpdate() {
    if (state.phase !== 'idle') {
      toast('Stop the recording first, then install the update.', 'warning');
      return;
    }
    const b = $('update');
    b.disabled = true;
    $('update-text').textContent = state.update.status === 'ready' ? 'Installing…' : 'Downloading…';
    try {
      await bridge.installUpdate(); // Encore closes and reopens on the new version
    } catch (e) {
      toast(String(e), 'error', 15000);
    }
    b.disabled = false;
    renderUpdate();
  }

  bridge.onUpdateStatus((u) => { state.update = u; renderUpdate(); });
  bridge.onUpdateProgress(({ done, total }) => {
    if (total) $('update-text').textContent = `Downloading ${Math.round((done / total) * 100)}%`;
    if (total && done >= total) $('update-text').textContent = 'Installing…';
  });
  $('update').addEventListener('click', installUpdate);
  $('check-updates').addEventListener('click', checkForUpdates);
  document.querySelectorAll('[data-update-mode]').forEach((o) =>
    o.addEventListener('click', async () => { await saveSetting('update_mode', o.dataset.updateMode); renderUpdate(); }));

  // ---------- app menu ----------
  menuButton($('menu-btn'), () => {
    const u = state.update || { status: 'none' };
    const collapsed = document.body.classList.contains('side-collapsed');
    return [
      { text: 'File', sub: [
        { text: 'Settings', hint: 'Ctrl+Comma', run: () => openDialog('settings') },
        { text: 'Open Recordings Folder', run: () => { if (state.settings) bridge.openFolder(state.settings.output_dir).catch((e) => toast(String(e), 'error')); } },
        '-',
        { text: 'Exit', hint: 'Alt+F4', run: () => bridge.win.close() },
      ] },
      { text: 'View', sub: [
        { text: 'What to Record', hint: 'Ctrl+B', check: !collapsed, run: () => setSideCollapsed(!collapsed) },
      ] },
      { text: 'Help', sub: [
        u.status === 'none'
          ? { text: 'Check for Updates...', run: checkForUpdates }
          : { text: `${u.status === 'ready' ? 'Restart to Update' : 'Update to'} ${u.version}...`, run: installUpdate },
        { text: 'Changelog', run: () => openDialog('changelog') },
        { text: 'About Encore', run: () => openDialog('about') },
      ] },
    ];
  });

  // ---------- dialogs: they fade in and out like the editor's ----------
  let lastFocus = null;
  function openDialog(id) {
    hideMenu();
    hideTip();
    const ov = $(id);
    if (!ov.hidden && ov.classList.contains('on')) return;
    lastFocus = document.activeElement;
    ov.hidden = false;
    requestAnimationFrame(() => ov.classList.add('on'));
    if (id === 'settings') { renderSettings(); renderUpdate(); refreshInputs(); }
    if (id === 'changelog') renderChangelog();
    if (id === 'about') $('about-title').textContent = state.version ? `Encore ${state.version}` : 'Encore';
    if (document.activeElement && document.activeElement.blur) document.activeElement.blur();
  }
  function closeDialog(ov) {
    if (ov.hidden) return false;
    ov.classList.remove('on');
    setTimeout(() => { if (!ov.classList.contains('on')) ov.hidden = true; }, 160);
    return true;
  }
  function closeTopDialog() {
    const open = [...document.querySelectorAll('.overlay, .modal-ov')].filter((o) => !o.hidden && o.classList.contains('on'));
    const top = open.find((o) => o.classList.contains('modal-ov')) || open[0];
    if (top && closeDialog(top) && lastFocus) lastFocus.focus({ preventScroll: true });
  }
  document.querySelectorAll('[data-open]').forEach((b) => b.addEventListener('click', () => openDialog(b.dataset.open)));
  document.querySelectorAll('[data-close]').forEach((b) => b.addEventListener('click', () => closeDialog(b.closest('.overlay, .modal-ov'))));
  document.querySelectorAll('.overlay, .modal-ov').forEach((ov) =>
    ov.addEventListener('pointerdown', (e) => { if (e.target === ov && e.button === 0) closeDialog(ov); }));
  document.querySelectorAll('[data-link]').forEach((b) =>
    b.addEventListener('click', () => bridge.openLink(b.dataset.link).catch((e) => toast(String(e), 'error'))));
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') {
      if (!$('ctxmenu').hidden) { hideMenu(); return; }
      closeTopDialog();
      return;
    }
    // small dialogs also close with Enter
    if (e.key === 'Enter' && [...document.querySelectorAll('.modal-ov')].some((o) => !o.hidden)) { e.preventDefault(); closeTopDialog(); return; }
    if (e.ctrlKey && !e.shiftKey && !e.altKey && e.key === ',') { e.preventDefault(); openDialog('settings'); }
    if (e.ctrlKey && !e.shiftKey && !e.altKey && (e.key === 'b' || e.key === 'B')) {
      e.preventDefault();
      setSideCollapsed(!document.body.classList.contains('side-collapsed'));
    }
  });

  function setSideCollapsed(collapsed) {
    document.body.classList.toggle('side-collapsed', collapsed);
    $('side-toggle').setAttribute('aria-expanded', String(!collapsed));
    const label = collapsed ? 'Open What to record' : 'Collapse What to record';
    $('side-toggle').removeAttribute('title');
    $('side-toggle').dataset.tip = label;
    $('side-toggle').setAttribute('aria-label', label);
    localStore('encore.side', collapsed ? 'collapsed' : 'open');
  }
  $('side-toggle').addEventListener('click', () => setSideCollapsed(!document.body.classList.contains('side-collapsed')));
  setSideCollapsed(localStore('encore.side') === 'collapsed');

  function renderChangelog() {
    const body = $('changelog-body');
    body.replaceChildren();
    body.classList.add('updnotes');
    const releases = window.ENCORE_CHANGELOG || [];
    if (!releases.length) {
      body.append(h('p', '', 'The changelog starts at version 1.0. Encore is in beta; until then, what changed in each test build is listed in its release notes on GitHub.'));
      const p = h('p');
      const a = h('button', 'linkbtn', 'Open the releases page');
      a.type = 'button';
      a.onclick = () => bridge.openLink('https://github.com/ancillaauri/Encore/releases').catch((e) => toast(String(e), 'error'));
      p.append(a);
      body.append(p);
      return;
    }
    for (const r of releases) {
      const head = h('h3', 'relhead', `Encore ${r.version}`);
      if (r.date) head.append(h('small', '', r.date));
      body.append(head);
      for (const [title, items] of Object.entries(r.sections || {})) {
        if (!items.length) continue;
        const ul = h('ul', 'notes');
        for (const it of items) ul.append(h('li', '', it));
        body.append(h('h4', '', title), ul);
      }
    }
  }


  // ---------- window buttons ----------
  $('wc-min').addEventListener('click', () => bridge.win.minimize());
  $('wc-max').addEventListener('click', () => bridge.win.toggleMaximize());
  $('wc-close').addEventListener('click', () => bridge.win.close());
  async function renderMaximized() {
    let max = false;
    try { max = await bridge.win.isMaximized(); } catch (e) { /* keep the default */ }
    const b = $('wc-max');
    b.innerHTML = max ? '&#xE923;' : '&#xE922;';
    b.setAttribute('aria-label', max ? 'Restore' : 'Maximise');
    b.title = max ? 'Restore' : 'Maximise';
  }
  try {
    bridge.win.onResized(renderMaximized);
    bridge.win.onFocusChanged((focused) => document.body.classList.toggle('inactive', !focused));
  } catch (e) { /* not running in the app */ }
  renderMaximized();

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
    try { state.version = await bridge.appVersion(); } catch (e) { state.version = ''; }
    try { state.update = await bridge.getUpdate(); } catch (e) { /* no update info yet */ }
    renderUpdate();
  }
  init();
})();
