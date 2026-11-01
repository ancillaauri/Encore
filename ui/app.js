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
    source: 'desktop', // 'desktop' (Follow or a monitor) or 'window' (Select Application)
    windowHandle: 0,
    windowTitle: '',
    focusMonitor: null, // id of the monitor last clicked on (what "follow" records)
    replay: { on: false }, // instant replay: { on, seconds, label, waiting }
    games: { on: false, games: [], active: null, locked: null, paused: false }, // from the engine
    replayBusy: false,
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
    const inRow = !!sel.closest('.setrow, .srow');
    if (!inRow) btn.classList.add('full');
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
    })), inRow);
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
  // The (i) next to an option explains it; clicking it shouldn't flip the switch it sits beside.
  document.querySelectorAll('.info').forEach((i) => {
    i.addEventListener('click', (e) => { e.preventDefault(); e.stopPropagation(); showTip(i); });
    i.addEventListener('focus', () => showTip(i));
    i.addEventListener('blur', hideTip);
  });
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

  // ---------- where recordings go ----------
  // The Encore folder inside the chosen folder (the engine does the same: paths::encore_root).
  function encoreRoot(dir) {
    const d = (dir ?? (state.settings ? state.settings.output_dir : '')).replace(/[\\/]+$/, '');
    if (!d) return '';
    const last = d.split(/[\\/]/).pop();
    return last.toLowerCase() === 'encore' ? d : `${d}\\Encore`;
  }
  function renderOutputNote() {
    const root = encoreRoot();
    if (root) $('output-note').textContent = `Encore makes a folder called Encore here, with a folder inside for Desktop and each game or app: ${root}\\Desktop, ${root}\\Warframe…`;
    renderWorkDir();
  }
  // Working folder: where recordings are written while they're being made, and where instant
  // replay keeps its last few minutes. Empty = Encore's own folder (AppData on C:).
  function renderWorkDir() {
    const s = state.settings;
    if (!s) return;
    const custom = (s.work_dir || '').trim();
    $('work-dir').value = custom ? `${custom.replace(/\\$/, '')}\\Encore temp` : 'Default (on the C: drive)';
    $('default-work').disabled = !custom;
    const replay = sizeText(mbPerMinute(s.quality) * (s.replay_seconds + 6) / 60);
    $('work-note').textContent = `Where recordings are written while they\u2019re being made, and where Echoes keeps its last few minutes (about ${replay} at your settings). Choose a drive with plenty of free space; an SSD is best. Encore makes an \u201cEncore temp\u201d folder inside.`;
  }
  async function setWorkDir(dir) {
    if (state.phase === 'recording') { toast('Stop the recording first, then change the working folder.', 'warning', 6000); return; }
    await saveSetting('work_dir', dir);
  }
  $('browse-work').addEventListener('click', async () => {
    try {
      const dir = await bridge.pickFolder(state.settings && state.settings.work_dir ? state.settings.work_dir : '');
      if (dir) await setWorkDir(dir);
    } catch (e) { toast(String(e), 'error'); }
  });
  $('default-work').addEventListener('click', () => setWorkDir(''));

  // ---------- what will be / is being recorded ----------
  const following = () => !state.settings || state.settings.monitor === 'follow';
  const pinnedId = () => (state.settings && state.settings.monitor.startsWith('id:') ? state.settings.monitor.slice(3) : null);
  // Windows' own number for a monitor ("\\.\DISPLAY2" is 2).
  const displayNo = (m) => { const n = /(\d+)\s*$/.exec(m.device || ''); return n ? n[1] : '?'; };
  const monitorById = (id) => state.monitors.find((m) => m.id === id);

  // The monitor that would be recorded now: the pinned one, or the one being used.
  function targetMonitor() {
    const pin = pinnedId();
    if (pin) return monitorById(pin);
    return monitorById(state.focusMonitor) || state.monitors.find((m) => m.primary) || state.monitors[0];
  }

  function describeTarget() {
    if (state.source === 'window') return state.windowTitle ? `Records ${state.windowTitle}` : 'Choose an app to record';
    const m = targetMonitor();
    if (pinnedId()) return m ? `Records ${m.name}` : 'Records the chosen monitor (not connected)';
    return "Records the screen you're using";
  }

  // Monitors are drawn at most this wide; narrower when the window is (see fitPicks).
  const MON_MAX = 210;
  let monBase = MON_MAX;

  function renderPicks() {
    const box = $('mons');
    box.textContent = '';
    const recording = state.phase !== 'idle';
    const recId = state.recording && state.recording.monitor;
    const pin = pinnedId();
    const shown = targetMonitor();
    // Left to right, top to bottom, as in Windows' display settings; sized relative to each other.
    const mons = [...state.monitors].sort((a, b) => a.x - b.x || a.y - b.y);
    const maxW = Math.max(1, ...mons.map((m) => m.width));
    for (const m of mons) {
      const w = Math.round(monBase * Math.max(0.6, m.width / maxW));
      const hgt = Math.round(w * (m.height / Math.max(1, m.width)));
      const on = state.source === 'desktop' && pin === m.id;
      // Following: the screen in use gets a lighter highlight (the Follow tile is the choice).
      const cur = state.source === 'desktop' && !pin && shown && shown.id === m.id;
      const b = h('button', `pick${on ? ' on' : ''}${cur ? ' cur' : ''}${recording && recId === m.id ? ' rec' : ''}`);
      b.type = 'button';
      b.setAttribute('role', 'radio');
      b.setAttribute('aria-checked', String(on));
      b.disabled = recording;
      const mon = h('span', 'mon', displayNo(m));
      mon.style.width = `${w}px`;
      mon.style.height = `${Math.max(40, Math.min(170, hgt))}px`;
      b.append(mon, h('span', 'neck'), h('span', 'beam'), h('span', 'pn', m.name));
      if (recording && recId === m.id) {
        const live = h('span', 'live');
        live.append(h('i'), document.createTextNode('Recording'));
        b.append(live);
      } else {
        b.append(h('span', 'ps', cur ? 'In use' : `${m.width}×${m.height}${m.primary ? ' · Primary' : ''}`));
      }
      b.title = `Always record ${m.name} (${m.width}×${m.height}${m.primary ? ', primary' : ''})`;
      b.onclick = () => pickMonitor(m);
      box.append(b);
    }
    const fol = $('pick-follow');
    const folOn = state.source === 'desktop' && !pin;
    fol.classList.toggle('on', folOn);
    fol.setAttribute('aria-checked', String(folOn));
    fol.disabled = recording;
    const app = $('pick-app');
    app.classList.toggle('on', state.source === 'window');
    app.setAttribute('aria-checked', String(state.source === 'window'));
    app.disabled = recording;
    $('pick-app-sub').textContent = state.source === 'window' && state.windowTitle ? state.windowTitle : 'Choose';
    // One line explaining what happens.
    let note = '';
    if (recording && state.recording && state.recording.following) note = 'Follows the screen you click on. Click a window on another monitor to move the recording there.';
    else if (recording) note = '';
    else if (state.source === 'window' && state.settings && state.settings.game_detection) note = 'Only this window is recorded, even when you alt-tab. Game detection is paused until you pick Follow or a monitor.';
    else if (state.source === 'window') note = 'Only this window is recorded, even when you alt-tab. Pick Follow or a monitor to record a screen instead.';
    else if (gameActive()) note = `${state.phase === 'recording' ? 'Recording' : 'Records'} ${gameActive().name}. The highlighted screen is used again when it closes.`;
    else if (gameWaiting()) note = 'Game detection is on: the green screen is recorded until a game opens.';
    else if (gamesOnly()) note = '';
    else if (pin) note = 'Always records this monitor. Pick Follow to record whichever screen you click on.';
    else note = 'Records the screen you click on and moves with you. Pick a monitor to always record that one.';
    $('pick-note').textContent = note;
    const s = state.settings;
    $('pick-sub').innerHTML = gamesOnly()
      ? 'Encore only records games. Turn on Desktop Capture to record your screen, or keep Echoes, when no game is open.'
      : s && s.game_detection
        ? 'When no game is open, Encore records the screen you choose below (recordings and Echoes). Games take over when they start.'
        : 'Encore records the screen you choose below, for recordings and Echoes. <strong>Follow</strong> is recommended.';
    const dc = !s || !!s.desktop_capture;
    $('dc-on').checked = dc;
    $('dc-on').disabled = !!(s && !s.game_detection) || state.phase !== 'idle';
    $('dc-btn').dataset.tip = s && !s.game_detection
      ? 'Desktop Capture: On. Needed while game detection is off (otherwise nothing could be recorded).'
      : dc ? 'Desktop Capture: On. Click to only record games.' : 'Desktop Capture: Off. Click to also record your screen when no game is open.';
    $('picks').classList.toggle('unused', gamesOnly());
    $('picks').querySelectorAll('button').forEach((b) => { if (gamesOnly()) b.disabled = true; });
  }

  // Keeps the picker on one line: shrinks the monitors until everything fits the stage.
  function fitPicks() {
    const row = $('picks');
    const stage = document.querySelector('.stage');
    // Too wide (the row never wraps, it overflows) or too tall for the stage.
    const tooBig = () => row.scrollWidth > row.clientWidth + 1
      || $('pick-follow').getBoundingClientRect().left < row.getBoundingClientRect().left - 1
      || $('pick-app').getBoundingClientRect().right > row.getBoundingClientRect().right + 1
      || (stage && stage.scrollHeight > stage.clientHeight + 1);
    // Larger windows get larger monitors, up to MON_MAX.
    monBase = stage && stage.clientHeight > 820 ? MON_MAX : 150;
    row.classList.remove('compact', 'tight');
    renderPicks();
    // Follow and Select Application shrink first, so the monitors stay the biggest things here.
    for (const cls of ['compact', 'tight']) {
      if (!tooBig()) return;
      row.classList.add(cls);
    }
    while (monBase > 100 && tooBig()) {
      monBase -= 10;
      renderPicks();
    }
  }
  try { new ResizeObserver(() => fitPicks()).observe(document.querySelector('.stage')); } catch (e) { /* older engine */ }

  // Picking a monitor always records that one; Follow records the screen you click on.
  async function pickScreen(value) {
    if (state.phase !== 'idle') return;
    const wasWindow = state.source === 'window';
    state.source = 'desktop';
    if (state.settings && state.settings.monitor !== value) await saveSetting('monitor', value);
    else if (wasWindow) restartReplay();
    renderStage();
  }
  const pickMonitor = (m) => pickScreen(`id:${m.id}`);
  $('pick-follow').addEventListener('click', () => pickScreen('follow'));

  async function pickApp() {
    if (state.phase !== 'idle') return;
    try { state.windows = await bridge.listWindows(); } catch (e) { state.windows = []; }
    const items = state.windows.length
      ? state.windows.map((w) => ({
        text: w.exe ? `${w.title} (${w.exe})` : w.title,
        check: state.source === 'window' && state.windowHandle === w.handle,
        run: () => {
          state.source = 'window';
          state.windowHandle = w.handle;
          state.windowTitle = w.title;
          renderStage();
          restartReplay();
        },
      }))
      : [{ text: 'No windows found', disabled: true, run: () => {} }];
    const r = $('pick-app').getBoundingClientRect();
    const m = openMenu(r.left, r.bottom + 6, items);
    m.dataset.for = 'pick-app';
  }

  function renderStage() {
    const body = document.body;
    body.classList.toggle('recording', state.phase === 'recording');
    body.classList.toggle('saving', state.phase === 'saving');
    const rec = $('record');
    const noGame = state.phase === 'idle' && gamesOnly() && !gameActive();
    rec.disabled = state.phase === 'starting' || state.phase === 'saving' || state.engineGone || noGame;
    const label = { idle: 'Start recording', starting: 'Starting…', recording: 'Stop and save', saving: 'Saving…' }[state.phase];
    const key = state.settings && state.settings.hotkey_record ? state.settings.hotkey_record.replace(/\+/g, ' + ') : '';
    $('record-label').textContent = noGame ? 'Start a game to record (or turn on Desktop Capture)'
      : key && (state.phase === 'idle' || state.phase === 'recording') ? `${label} \u00b7 ${key}` : label;
    rec.setAttribute('aria-label', label);

    const tracks = state.phase === 'recording' && state.recording ? state.recording.audio_tracks || [] : [];
    $('tracks').hidden = !tracks.length;
    $('tracks').textContent = tracks.length ? `Audio tracks: ${tracks.join(', ')}` : '';

    if (state.phase === 'recording' && state.recording) {
      const r = state.recording;
      const m = r.monitor && monitorById(r.monitor);
      $('what').textContent = m ? `Recording ${m.name}` : `Recording ${r.source.replace(/^Desktop: /, '').replace(/^Window: /, '')}`;
      $('detail').textContent = `${r.width}×${r.height}, ${r.fps} fps, ${encoderLabel(r.encoder)}${r.pipeline === 'download' ? ' (copy path)' : ''}`;
    } else if (state.phase === 'saving') {
      $('what').textContent = 'Saving your recording';
      $('detail').textContent = 'Joining the pieces into one file.';
    } else {
      $('what').textContent = describeTarget();
      $('detail').textContent = state.settings
        ? (state.replay.on ? `Echoes is on \u00b7 saved to ${encoreRoot()}` : `Saved to ${encoreRoot()}`)
        : '';
      $('timer').textContent = '0:00';
      $('ring-sweep').style.strokeDashoffset = RING;
    }
    renderStatus();
    // Settings that change the file are locked while recording or saving.
    const locked = state.phase !== 'idle';
    $('lock-note').hidden = !locked;
    document.querySelectorAll('#quick select, #quick input, #quick-audio input').forEach((el) => (el.disabled = locked));
    fitPicks();
    syncTray();
    renderReplay();
  }

  // The status panel beside the record button: one line per feature, with a coloured light.
  function renderStatus() {
    const list = $('status');
    if (!list) return;
    const s = state.settings;
    const rows = [];
    const add = (kind, title, value) => rows.push({ kind, title, value });
    // Recorder
    if (state.phase === 'recording' && state.recording) {
      const r = state.recording;
      add('rec', 'Recording', [`${r.width}\u00d7${r.height}`, r.fps ? `${r.fps} fps` : '', encoderLabel(r.encoder)].filter(Boolean).join(' \u00b7 '));
    } else if (state.phase === 'saving') add('acc', 'Saving', 'Joining the pieces into one file');
    else if (state.phase === 'starting') add('acc', 'Starting', 'Getting the capture ready');
    else add('', 'Standing by', 'Ready when you are');
    // What's captured
    const game = gameActive();
    const waiting = gameWaiting();
    if (state.phase === 'recording' && state.recording && state.recording.game) {
      add('acc', 'Recording game', state.recording.game);
    } else if (state.phase === 'recording' && state.recording) {
      const r = state.recording;
      const m = r.monitor && monitorById(r.monitor);
      const name = m ? m.name : r.source.replace(/^Desktop: /, '').replace(/^Window: /, '').replace(/^Game: /, '');
      add(waiting ? 'good' : 'acc', r.following ? 'Following you' : (m ? 'Locked to screen' : 'Application'), name);
    } else if (game && state.source === 'desktop') {
      // The game detection line below says which game.
    } else if (gamesOnly()) {
      add('', 'Desktop Capture', 'Off \u00b7 waiting for a game');
    } else if (state.source === 'window') {
      add(state.windowTitle ? 'acc' : '', 'Application', state.windowTitle || 'Choose one below');
    } else if (pinnedId()) {
      const m = targetMonitor();
      add(waiting ? 'good' : 'acc', 'Locked to screen', m ? m.name : 'Not connected: the primary is used');
    } else {
      const m = targetMonitor();
      add(waiting ? 'good' : 'acc', 'Following you', m ? `On ${m.name}` : 'The screen you click');
    }
    // Game detection
    if (s && s.game_detection) {
      const g = state.games || {};
      if (state.source === 'window' || g.paused) add('', 'Game detection', 'Paused while recording an application');
      else if (game && g.locked === game.pid) add('acc', 'Locked to', game.name);
      else if (game) add('acc', 'Tracking', game.name);
      else add('good', 'Game detection', gamesOnly() ? 'Waiting for a game' : 'Scanning for games');
    }
    // Instant replay
    if (state.replayRestarting) add('warn', 'Echoes', 'Restarting with your new settings');
    else if (state.replay && state.replay.on && state.replay.waiting) add('', 'Echoes', 'Waiting for a game');
    else if (state.replay && state.replay.on) add('good', 'Echoes', `Keeping the last ${replayLength()}`);
    else add('', 'Echoes', 'Off');
    // Audio tracks while recording
    const tracks = state.phase === 'recording' && state.recording ? state.recording.audio_tracks || [] : [];
    if (tracks.length) add('', `${tracks.length} audio track${tracks.length === 1 ? '' : 's'}`, tracks.join(', '));
    // Where files go
    if (s) add('', 'Saving to', encoreRoot());
    list.textContent = '';
    for (const r of rows) {
      const li = h('li', r.kind);
      li.append(h('span', 'sd'), h('span', 'st', r.title), h('span', 'sv', r.value));
      li.title = `${r.title}: ${r.value}`;
      list.append(li);
    }
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

  // ---------- settings (side panel and Settings window share the same controls) ----------
  function renderSettings() {
    const s = state.settings;
    if (!s) return;
    document.querySelectorAll('[data-setting]').forEach((el) => {
      const v = s[el.dataset.setting];
      if (el.type === 'checkbox') el.checked = !!v;
      else if (v !== undefined) el.value = String(v);
    });
    renderInputs();
    renderCodecs();
    renderOutputNote();
    renderHotkeys();
    renderReplay();
    renderSounds();
    renderToggles();
    renderGames();
    renderGameSettings();
    if (!$('setup').hidden) renderSetup();
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
    // The card that will actually be used (same order as the engine: NVIDIA, AMD, Intel).
    const codec = state.settings ? state.settings.codec : 'h264';
    const used = ['nvidia', 'amd', 'intel'].find((v) => c.encoders.some((e) => e.vendor === v && e.codec === codec))
      || ['nvidia', 'amd', 'intel'].find((v) => c.encoders.some((e) => e.vendor === v));
    line.textContent = `Encoding on your ${VENDOR[used]} graphics card`;
  }

  async function saveSetting(key, value) {
    try {
      state.settings = await bridge.setSettings({ [key]: value });
      if (key === 'audio_mic') renderInputs();
      if (key === 'codec') renderEngineLine();
      if (CAPTURE_KEYS.includes(key) || key === 'desktop_capture' || (key === 'game_detection' && state.replay.waiting)) restartReplay();
      renderSettings();
      renderStage();
      renderUpdate();
    } catch (e) {
      toast(String(e), 'error');
      renderSettings();
    }
  }

  async function refreshMonitors() {
    try { state.monitors = await bridge.listMonitors(); } catch (e) { state.monitors = []; }
    renderStage();
  }

  // ---------- record / stop ----------
  async function toggleRecording() {
    if (state.phase === 'idle') {
      const source = currentSource();
      if (!source) return;
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

  // "Desktop 2026.10.31 - 21.32.04.mp4" shows as "Desktop 21.32.04" when it's from today.
  function shortName(path) {
    const n = fileName(path).replace(/\.mp4$/i, '');
    const t = new Date();
    const today = `${t.getFullYear()}.${String(t.getMonth() + 1).padStart(2, '0')}.${String(t.getDate()).padStart(2, '0')} - `;
    return n.replace(today, '');
  }

  function addRecent(d) {
    state.recent.unshift(d);
    const li = h('li');
    li.title = `${d.path}\nClick to show in folder`;
    const thumb = h('span', 'thumb');
    thumb.innerHTML = '<svg viewBox="0 0 24 24" aria-hidden="true"><rect x="3.5" y="5.5" width="17" height="13" rx="2"/><path d="M10 9.5v5l4.5-2.5z" fill="currentColor" stroke="none"/></svg>';
    const text = h('div', 'rtext');
    const name = h('div', 'rname', shortName(d.path));
    if (d.recovered) name.append(h('span', 'tag', 'recovered'));
    if (d.replay) name.append(h('span', 'tag replay-live', 'Echo'));
    text.append(name, h('div', 'rmeta', d.duration_ms ? `${clock(d.duration_ms)} · ${size(d.size_bytes)}` : size(d.size_bytes)));
    li.append(thumb, text);
    li.onclick = () => bridge.revealFile(d.path).catch((e) => toast(String(e), 'error'));
    $('recent').prepend(li);
    $('recent-empty').hidden = true;
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
      case 'replay_status':
        state.replay = data || { on: false };
        renderReplay();
        renderStage();
        break;
      case 'replay_saving':
        toast(`Saving the last ${data.seconds ? lengthText(data.seconds) : replayLength()}…`, 'info', 3000);
        break;
      case 'replay_saved': {
        addRecent({ ...data, replay: true });
        const len = data.duration_ms ? ` (${lengthText(Math.round(data.duration_ms / 1000))})` : '';
        toast(`Saved an Echo${len}: ${fileName(data.path)}`, 'ok', 5000);
        break;
      }
      case 'games':
        state.games = data;
        renderGames();
        renderStage();
        break;
      case 'game_switched':
        if (state.recording) state.recording.game = data.name || null;
        renderStage();
        break;
      case 'focus_monitor':
        state.focusMonitor = data.id || state.focusMonitor;
        if (state.phase === 'idle') renderStage();
        break;
      case 'monitor_switched':
        if (state.recording) state.recording.monitor = data.monitor;
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
  $('pick-app').addEventListener('click', (e) => {
    e.stopPropagation();
    const m = $('ctxmenu');
    if (!m.hidden && m.dataset.for === 'pick-app') { hideMenu(); return; }
    pickApp();
  });
  const openOutput = () => { if (state.settings) bridge.openFolder(encoreRoot()).catch((e) => toast(String(e), 'error')); };
  $('open-folder').addEventListener('click', openOutput);
  $('rail-folder').addEventListener('click', openOutput);
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
    if (state.settings) bridge.openFolder(encoreRoot()).catch((e) => toast(String(e), 'error'));
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
      toast(String(e), 'error', 10000);
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
        { text: 'Open Recordings Folder', run: () => { if (state.settings) bridge.openFolder(encoreRoot()).catch((e) => toast(String(e), 'error')); } },
        '-',
        ...(state.elevated ? [] : [{ text: 'Restart as Administrator', run: () => bridge.restartAsAdmin().catch((e) => toast(String(e), 'error')) }]),
        { text: 'Exit', run: () => bridge.quitApp() },
      ] },
      { text: 'View', sub: [
        { text: 'What to Record', hint: 'Ctrl+B', check: !collapsed, run: () => setSideCollapsed(!collapsed) },
      ] },
      { text: 'Help', sub: [
        u.status === 'none'
          ? { text: 'Check for Updates...', run: checkForUpdates }
          : { text: `${u.status === 'ready' ? 'Restart to Update' : 'Update to'} ${u.version}...`, run: installUpdate },
        { text: 'Run Setup Again...', run: openSetup },
        '-',
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
    if (id === 'settings') { renderSettings(); renderUpdate(); refreshInputs(); refreshAdmin(); refreshAutostart(); }
    if (id === 'changelog') renderChangelog();
    if (id === 'about') $('about-title').textContent = state.version ? `Encore ${state.version}` : 'Encore';
    if (document.activeElement && document.activeElement.blur) document.activeElement.blur();
  }
  function closeDialog(ov) {
    if (ov.hidden) return false;
    if (ov.id === 'setup') finishSetup(); // finished or skipped: don't show it again
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
    if (e.key === 'Enter' && !$('setup').hidden) { if (!e.target.closest('button')) { e.preventDefault(); $('setup-next').click(); } return; }
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

  // ---------- instant replay ----------
  // What to capture, from the picker: Follow / a monitor, or the application chosen in Select Application.
  function currentSource() {
    if (state.source !== 'window') return { source: 'desktop' };
    if (!state.windowHandle) { toast('Choose an app to record first.', 'warning'); return null; }
    return { source: 'window', handle: state.windowHandle };
  }
  const REPLAY_NAMES = { 30: '30 seconds', 60: 'minute', 120: '2 minutes', 300: '5 minutes', 600: '10 minutes', 900: '15 minutes', 1200: '20 minutes' };
  function replayLength() {
    const n = state.settings ? state.settings.replay_seconds : 120;
    return n === 60 ? 'minute' : REPLAY_NAMES[n] || `${n} seconds`;
  }
  // What Echoes is keeping: the game, the window, or the screen.
  function echoTarget() {
    const g = gameActive();
    if (g && state.source === 'desktop') return g.name;
    if (state.source === 'window') return state.windowTitle || '';
    const m = targetMonitor();
    return m ? m.name : '';
  }
  function renderReplay() {
    const s = state.settings;
    if (!s) return;
    const on = !!state.replay.on;
    $('replay-on').checked = on || !!state.replayRestarting;
    $('replay-on').disabled = state.replayBusy || (state.phase !== 'idle' && !on);
    $('save-replay').disabled = !on || state.replayBusy;
    const key = s.hotkey_save_replay ? s.hotkey_save_replay.replace(/\+/g, ' + ') : '';
    const disk = sizeText(mbPerMinute(s.quality) * (s.replay_seconds + 6) / 60);
    $('save-key').textContent = key;
    $('echo-btn').dataset.tip = on
      ? `Echoes: On. Keeping the last ${replayLength()}. Click to turn off.`
      : 'Echoes: Off. Click to keep the last few minutes, so you can save great moments after they happen.';
    $('q-replay').closest('.hrow').dataset.tip = `Uses about ${disk} of disk space while Echoes is on (at your video settings).`;
    $('replay-sec').classList.toggle('lit', on);
    $('replay-sec').classList.toggle('restarting', !!state.replayRestarting);
    $('qs-replay-sub').textContent = state.replayRestarting ? 'Restarting with your new settings\u2026'
      : state.replay.waiting ? 'Waiting for a game'
      : on ? `On \u00b7 keeping the last ${replayLength()}${echoTarget() ? ` of ${echoTarget()}` : ''}`
      : 'Off \u00b7 save great moments after they happen';
    renderStatus();
  }
  async function setReplay(on, remember = true) {
    if (state.replayBusy) return;
    state.replayBusy = true;
    renderReplay();
    try {
      if (on) {
        const src = currentSource();
        if (!src) throw new Error('');
        const st = await bridge.startReplay(src);
        state.replay = (st && st.replay) || { on: true };
      } else {
        const st = await bridge.stopReplay();
        state.replay = (st && st.replay) || { on: false };
      }
      if (remember) await saveSetting('replay_on', on);
    } catch (e) {
      if (String(e)) toast(String(e).replace(/^Error: ?/, ''), 'error', 10000);
      state.replay = { on: false };
    }
    state.replayBusy = false;
    renderReplay();
    renderStage();
  }
  // Settings that change what's captured restart the buffer (it starts again from now).
  const CAPTURE_KEYS = ['work_dir', 'codec', 'fps', 'resolution', 'quality', 'capture_cursor', 'monitor', 'audio_full_mix', 'audio_mic', 'mic_device', 'audio_app_tracks'];
  let replayRestart = null;
  // Shown in amber while Echoes restarts for a settings change (it starts again from now).
  function restartReplay() {
    if (!state.replay.on || state.phase !== 'idle') return;
    clearTimeout(replayRestart);
    state.replayRestarting = true;
    renderReplay();
    replayRestart = setTimeout(async () => {
      try {
        await setReplay(false, false);
        await setReplay(true, false);
      } finally {
        state.replayRestarting = false;
        renderReplay();
      }
    }, 400);
  }
  async function saveReplay(seconds) {
    if (!state.replay.on) { toast('Turn on Echoes first (top left).', 'warning', 5000); return; }
    try {
      await bridge.saveReplay(seconds);
      playCue('save');
    } catch (e) { toast(String(e), 'error', 10000); }
  }
  $('replay-on').addEventListener('change', (e) => setReplay(e.target.checked));
  $('save-replay').addEventListener('click', () => saveReplay());

  // ---------- tray, hotkeys, sounds ----------
  // Sounds are made here (no sound files): each is a few short tones or a burst of noise.
  // Volume 0–100; 50 is the old fixed level.
  let audioCtx = null;
  const SOUNDS = {
    rise: [{ f: 660, t: 0 }, { f: 880, t: 0.11 }],
    fall: [{ f: 880, t: 0 }, { f: 587, t: 0.11 }],
    double: [{ f: 988, t: 0 }, { f: 988, t: 0.11 }],
    pop: [{ f: 420, to: 950, t: 0, d: 0.07 }],
    bell: [{ f: 1318, t: 0, d: 0.7, wave: 'triangle' }, { f: 2636, t: 0, d: 0.4, g: 0.35 }],
    blip: [{ f: 1250, t: 0, d: 0.05, wave: 'square', g: 0.35 }],
    click: [{ noise: true, t: 0, d: 0.018, hp: 2500 }],
    shutter: [{ noise: true, t: 0, d: 0.05, hp: 1200 }, { noise: true, t: 0.09, d: 0.07, hp: 800, g: 0.8 }],
    none: [],
  };
  function playSound(name, volume) {
    const parts = SOUNDS[name] || [];
    if (!parts.length || !volume) return;
    try {
      audioCtx = audioCtx || new AudioContext();
      const now = audioCtx.currentTime + 0.01;
      const peak = 0.24 * (volume / 100);
      for (const p of parts) {
        const t = now + p.t;
        const d = p.d || 0.1;
        const g = audioCtx.createGain();
        const level = peak * (p.g || 1);
        g.gain.setValueAtTime(0, t);
        g.gain.linearRampToValueAtTime(level, t + 0.006);
        g.gain.exponentialRampToValueAtTime(0.0001, t + d);
        let src;
        if (p.noise) {
          const len = Math.ceil(audioCtx.sampleRate * d);
          const buf = audioCtx.createBuffer(1, len, audioCtx.sampleRate);
          const data = buf.getChannelData(0);
          for (let i = 0; i < len; i++) data[i] = Math.random() * 2 - 1;
          src = audioCtx.createBufferSource();
          src.buffer = buf;
          const hp = audioCtx.createBiquadFilter();
          hp.type = 'highpass';
          hp.frequency.value = p.hp || 1000;
          src.connect(hp).connect(g);
        } else {
          src = audioCtx.createOscillator();
          src.type = p.wave || 'sine';
          src.frequency.setValueAtTime(p.f, t);
          if (p.to) src.frequency.exponentialRampToValueAtTime(p.to, t + d);
          src.connect(g);
        }
        g.connect(audioCtx.destination);
        src.start(t);
        src.stop(t + d + 0.02);
      }
    } catch (e) { /* no sound device: not important */ }
  }
  // kind: start | stop | save
  function playCue(kind) {
    const s = state.settings;
    if (!s || !s.sounds) return;
    playSound(s[`sound_${kind}`] || 'none', s[`volume_${kind}`] ?? 50);
  }
  document.querySelectorAll('[data-preview]').forEach((b) => b.addEventListener('click', () => {
    const s = state.settings;
    if (!s) return;
    const k = b.dataset.preview;
    playSound(s[`sound_${k}`] || 'none', s[`volume_${k}`] ?? 50);
  }));
  // Hear the new volume when the slider is let go, and the new sound when it's picked.
  document.querySelectorAll('#sound-rows [data-setting]').forEach((el) => el.addEventListener('change', () => {
    const row = el.closest('.soundrow');
    playSound(row.querySelector('select').value, Number(row.querySelector('.vol').value));
  }));
  function renderSounds() {
    const s = state.settings;
    if (!s) return;
    $('sound-rows').classList.toggle('off', !s.sounds);
    document.querySelectorAll('#sound-rows select, #sound-rows input, #sound-rows button').forEach((el) => (el.disabled = !s.sounds));
  }

  let trayRecording = false;
  function syncTray() {
    const rec = state.phase === 'recording';
    if (rec === trayRecording) return;
    trayRecording = rec;
    playCue(rec ? 'start' : 'stop');
    const label = rec && state.recording ? (state.recording.label || '') : '';
    bridge.setRecordingState(rec, label).catch(() => {});
    traySent = '';
    syncTrayGames();
  }

  // Hotkey settings: click the button, then press the keys.
  function hotkeyText(v) { return v ? v.replace(/\+/g, ' + ') : 'None'; }
  function renderHotkeys() {
    const s = state.settings;
    if (!s) return;
    document.querySelectorAll('[data-hotkey]').forEach((b) => {
      if (b.classList.contains('listening')) return;
      const v = s[b.dataset.hotkey] || '';
      b.textContent = hotkeyText(v);
      b.classList.toggle('empty', !v);
    });
    document.querySelectorAll('[data-close-to-tray]').forEach((o) =>
      o.setAttribute('aria-checked', String((o.dataset.closeToTray === 'true') === !!s.close_to_tray)));
    renderCustomLength();
  }
  // Custom save length: shown in minutes when it's a whole number of minutes, else in seconds.
  function lengthText(sec) {
    if (sec % 60 === 0) return sec === 60 ? '1 minute' : `${sec / 60} minutes`;
    if (sec === 1) return '1 second';
    return sec < 60 ? `${sec} seconds` : `${Math.floor(sec / 60)} min ${sec % 60} s`;
  }
  function renderCustomLength() {
    const s = state.settings;
    if (!s) return;
    const sec = s.replay_custom_seconds || 90;
    const box = $('custom-len');
    if (document.activeElement !== box) {
      const unit = sec % 60 === 0 && sec >= 60 ? 60 : 1;
      $('custom-unit').value = String(unit);
      box.value = String(sec / unit);
    }
    const max = s.replay_seconds;
    $('custom-len-note').textContent = sec > max
      ? `Longer than Echoes keeps, so it saves ${lengthText(max)}`
      : `Saves the last ${lengthText(sec)}`;
    $('custom-len-note').classList.toggle('warn', sec > max);
  }
  async function saveCustomLength() {
    const unit = Number($('custom-unit').value) || 1;
    let sec = Math.round(Number($('custom-len').value) * unit);
    if (!Number.isFinite(sec) || sec < 5) sec = 5;
    if (sec > 1200) sec = 1200;
    if (state.settings && sec !== state.settings.replay_custom_seconds) await saveSetting('replay_custom_seconds', sec);
    $('custom-len').blur();
    renderCustomLength();
  }
  $('custom-len').addEventListener('change', saveCustomLength);
  $('custom-len').addEventListener('keydown', (e) => { if (e.key === 'Enter') { e.preventDefault(); saveCustomLength(); } });
  $('custom-unit').addEventListener('change', saveCustomLength);
  const KEY_NAMES = { ' ': 'Space', ArrowUp: 'Up', ArrowDown: 'Down', ArrowLeft: 'Left', ArrowRight: 'Right', Escape: 'Esc' };
  function comboFrom(e) {
    const mods = [];
    if (e.ctrlKey) mods.push('Ctrl');
    if (e.altKey) mods.push('Alt');
    if (e.shiftKey) mods.push('Shift');
    if (e.metaKey) mods.push('Super');
    let key = null;
    if (/^Key[A-Z]$/.test(e.code)) key = e.code.slice(3);
    else if (/^Digit\d$/.test(e.code)) key = e.code.slice(5);
    else if (/^F\d{1,2}$/.test(e.code)) key = e.code;
    else if (/^Numpad\d$/.test(e.code)) key = e.code;
    else if (['Home', 'End', 'Insert', 'Delete', 'PageUp', 'PageDown', 'Pause', 'ScrollLock'].includes(e.code)) key = e.code;
    if (!key) return null; // a modifier on its own, or a key we don't support yet
    // Plain letters and numbers would fire while typing: they need a modifier. F-keys etc. don't.
    if (!mods.length && /^[A-Z0-9]$/.test(key)) return { error: 'Add Ctrl, Alt or Shift to that key, so it doesn\u2019t go off while you type.' };
    return { combo: [...mods, key].join('+') };
  }
  async function saveHotkey(setting, combo) {
    await saveSetting(setting, combo);
    try {
      const problems = await bridge.applyShellSettings();
      for (const p of problems || []) toast(p, 'error', 12000);
    } catch (e) { toast(String(e), 'error'); }
    renderHotkeys();
  }
  let listening = null;
  function stopListening() {
    if (!listening) return;
    listening.classList.remove('listening');
    listening = null;
    renderHotkeys();
  }
  document.querySelectorAll('[data-hotkey]').forEach((b) => b.addEventListener('click', () => {
    if (listening === b) { stopListening(); return; }
    stopListening();
    listening = b;
    b.classList.add('listening');
    b.textContent = 'Press the keys\u2026';
  }));
  document.addEventListener('keydown', (e) => {
    if (!listening) return;
    e.preventDefault();
    e.stopPropagation();
    if (e.key === 'Escape') { stopListening(); return; }
    const r = comboFrom(e);
    if (!r) return; // still holding modifiers
    if (r.error) { toast(r.error, 'warning', 5000); return; }
    const b = listening;
    stopListening();
    saveHotkey(b.dataset.hotkey, r.combo);
  }, true);
  window.addEventListener('blur', stopListening);
  document.addEventListener('pointerdown', (e) => { if (listening && e.target !== listening) stopListening(); }, true);
  $('hotkey-record-clear').addEventListener('click', () => { stopListening(); saveHotkey('hotkey_record', ''); });
  document.querySelectorAll('[data-hotkey-clear]').forEach((b) =>
    b.addEventListener('click', () => { stopListening(); saveHotkey(b.dataset.hotkeyClear, ''); }));
  document.querySelectorAll('[data-close-to-tray]').forEach((o) => o.addEventListener('click', async () => {
    await saveSetting('close_to_tray', o.dataset.closeToTray === 'true');
    bridge.applyShellSettings().catch(() => {});
    renderHotkeys();
  }));
  bridge.onHotkey((action) => {
    if (listening) return; // choosing a new hotkey in Settings: the keys are for that
    if (action === 'save_replay') { saveReplay(); return; }
    if (action === 'save_replay_30') { saveReplay(30); return; }
    if (action === 'save_replay_60') { saveReplay(60); return; }
    if (action === 'save_replay_custom') { saveReplay(state.settings ? state.settings.replay_custom_seconds : 90); return; }
    if (action === 'lock_game' || action === 'next_game') { gameHotkey(action); return; }
    if (action !== 'toggle_record') return;
    if (state.phase === 'idle' || state.phase === 'recording') toggleRecording();
  });

  // ---------- first-run setup ----------
  const BPP = { ultra: 0.30, high: 0.24, medium: 0.18, low: 0.10 }; // same as the engine (ffmpeg::bitrate_kbps)
  const CODEC_FACTOR = { h264: 1, hevc: 0.7, av1: 0.6 };
  // Output size for the main monitor at a resolution setting (never upscaled), like the engine.
  function outputSize(res) {
    const m = state.monitors.find((x) => x.primary) || state.monitors[0] || { width: 1920, height: 1080 };
    const target = { native: m.height, '1440p': 1440, '1080p': 1080, '720p': 720 }[res] || m.height;
    const h = Math.min(m.height, target);
    const w = Math.round((m.width * h) / m.height);
    return { w: w & ~1, h: h & ~1 };
  }
  function mbPerMinute(quality) {
    const s = state.settings;
    if (!s) return 0;
    const { w, h } = outputSize(s.resolution);
    let kbps = (w * h * s.fps * BPP[quality] * (CODEC_FACTOR[s.codec] || 1)) / 1000;
    kbps = Math.min(150000, Math.max(2000, kbps)) + 600; // plus audio
    return (kbps * 60) / 8 / 1000;
  }
  function sizeText(mb) {
    const r = Math.round(mb / 10) * 10;
    return r >= 1000 ? `${(r / 1000).toFixed(1)} GB` : `${r} MB`;
  }

  let setupStep = 1;
  function renderSetup() {
    const s = state.settings;
    if (!s) return;
    const steps = [...document.querySelectorAll('#setup .sstep')];
    const total = steps.length;
    steps.forEach((el) => (el.hidden = Number(el.dataset.step) !== setupStep));
    $('setup-step').textContent = `Step ${setupStep} of ${total}`;
    $('setup-title').textContent = steps[setupStep - 1].dataset.title;
    $('setup-dots').innerHTML = steps.map((_, i) => `<i class="${i + 1 === setupStep ? 'on' : i + 1 < setupStep ? 'done' : ''}"></i>`).join('');
    $('setup-back').disabled = setupStep === 1;
    $('setup-next').textContent = setupStep === total ? 'Start using Encore' : 'Next';
    const k = (v) => (v ? v.replace(/\+/g, ' + ') : 'not set');
    $('tour-key-record').textContent = k(s.hotkey_record);
    $('tour-key-echo').textContent = k(s.hotkey_save_replay);
    if (s.hotkey_save_replay) $('setup-echo-key').textContent = s.hotkey_save_replay.replace(/\+/g, ' + ');
    $('setup-folder').value = s.output_dir;
    const root = encoreRoot();
    $('setup-folder-note').textContent = `Encore makes a folder called Encore here, with a folder inside for Desktop and each game or app: ${root}\\Desktop, ${root}\\Warframe…`;
    const { w, h } = outputSize(s.resolution);
    document.querySelectorAll('#setup [data-quality]').forEach((b) => b.setAttribute('aria-checked', String(b.dataset.quality === s.quality)));
    document.querySelectorAll('#setup [data-size]').forEach((el) => {
      const q = el.dataset.size;
      const what = { ultra: 'Best quality, biggest files', high: 'Great quality, a fifth smaller', medium: 'Good quality, smaller files', low: 'Smallest files' }[q];
      el.textContent = `${what}. About ${sizeText(mbPerMinute(q))} per minute at ${h}p ${s.fps} fps.`;
    });
  }
  function openSetup() {
    setupStep = 1;
    refreshAdmin();
    const s0 = state.settings;
    $('setup-games').checked = !s0 || !s0.setup_done || !!s0.game_detection;
    $('setup-desktop').checked = !s0 || !s0.setup_done || !!s0.desktop_capture;
    $('setup-echo').checked = !s0 || !s0.setup_done || !!s0.replay_on;
    bridge.autostartStatus().then((on) => { $('setup-autostart').checked = !!on; }).catch(() => {});
    renderSetup();
    openDialog('setup');
  }
  // ---------- start as administrator ----------
  async function refreshAdmin() {
    try {
      const a = await bridge.adminStatus();
      state.elevated = !!a.elevated;
      state.startAsAdmin = !!a.start_as_admin;
    } catch { /* not available (e.g. in a browser preview) */ }
    $('start-admin').checked = !!state.startAsAdmin;
    $('setup-admin').checked = state.startAsAdmin || !state.settings || !state.settings.setup_done;
  }
  function busyNow() { return state.phase !== 'idle'; }
  async function setStartAsAdmin(on, { restart = 'ask' } = {}) {
    const box = $('start-admin');
    box.disabled = true;
    try {
      await bridge.setStartAsAdmin(on);
      state.startAsAdmin = on;
      if (on && !state.elevated) {
        if (restart === 'now' && !busyNow()) { await bridge.restartAsAdmin(); return; }
        toast('Done. Encore will start as administrator from now on. Restart it to start now.', 'ok', 12000,
          { label: 'Restart now', run: () => { if (busyNow()) toast('Stop recording first, then restart.', 'info'); else bridge.restartAsAdmin().catch((e) => toast(String(e), 'error')); } });
      } else if (on) {
        toast('Done. Encore will start as administrator from now on, with no prompt.', 'ok', 6000);
      } else {
        toast('Encore will start normally from now on.', 'ok');
      }
    } catch (e) {
      toast(String(e), 'error');
    } finally {
      box.disabled = false;
      box.checked = !!state.startAsAdmin;
    }
  }
  $('start-admin').addEventListener('change', (e) => setStartAsAdmin(e.target.checked));

  // ---------- Do not disturb and game detection buttons (click to turn on/off; the icon glows) ----------
  function renderToggles() {
    const s = state.settings;
    if (!s) return;
    // Hovering an icon switch says its state in words, so nobody has to rely on the colours.
    document.querySelectorAll('.arow .aic').forEach((lab) => {
      const box = lab.querySelector('input');
      const on = box.checked;
      lab.dataset.tip = `${box.getAttribute('aria-label')}: ${on ? 'On' : 'Off'}. Click to turn ${on ? 'off' : 'on'}.`;
    });
    $('dnd').removeAttribute('title');
    $('dnd').dataset.tip = s.do_not_disturb ? 'Do not disturb: On. Encore\u2019s notifications are paused. Click to turn off.' : 'Do not disturb: Off. Click to pause Encore\u2019s notifications.';
    $('rail-dnd').dataset.railTip = s.do_not_disturb ? 'Do not disturb: On' : 'Do not disturb: Off';
    $('game-on').removeAttribute('title');
    $('game-on').dataset.tip = s.game_detection ? 'Game detection: On. Click to turn off.' : 'Game detection: Off. Click to record games on their own as they open.';
    const dnd = !!s.do_not_disturb;
    for (const id of ['dnd', 'rail-dnd']) $(id).setAttribute('aria-pressed', String(dnd));
    $('dnd-sub').textContent = dnd ? 'On: notifications paused' : 'Off';
    const game = !!s.game_detection;
    $('game-on').setAttribute('aria-pressed', String(game));
    $('gamebar').classList.toggle('on', game);
  }
  // After a click the tip under the mouse shows the new state.
  function refreshTipFor(el) { setTimeout(() => { if (el.matches(':hover')) { tip.el = el; showTip(el, el.dataset.tip); } }, 80); }
  document.querySelectorAll('.arow .aic input').forEach((box) => box.addEventListener('change', () => setTimeout(() => { renderToggles(); refreshTipFor(box.closest('.aic')); }, 0)));
  for (const id of ['dnd', 'rail-dnd']) $(id).addEventListener('click', async () => {
    if (!state.settings) return;
    await saveSetting('do_not_disturb', !state.settings.do_not_disturb);
    refreshTipFor($(id));
  });
  $('game-on').addEventListener('click', async () => {
    if (!state.settings) return;
    await saveSetting('game_detection', !state.settings.game_detection);
    refreshTipFor($('game-on'));
  });

  // ---------- game detection ----------
  const LOCK_SVG = '<svg class="lock" viewBox="0 0 24 24" aria-hidden="true"><rect x="5" y="11" width="14" height="9" rx="2"/><path d="M8 11V8a4 4 0 0 1 8 0v3"/></svg>';
  function gameActive() {
    const g = state.games || {};
    if (!state.settings || !state.settings.game_detection) return null;
    return (g.games || []).find((x) => x.pid === g.active) || null;
  }
  // "Wait for a game": with game detection on, nothing is recorded on the desktop.
  function gamesOnly() {
    const s = state.settings;
    return !!(s && s.game_detection && !s.desktop_capture);
  }
  // Game detection is on but no game is open: the chosen screen is recorded meanwhile (green).
  function gameWaiting() {
    const s = state.settings;
    return !!(s && s.game_detection && !gamesOnly() && state.source === 'desktop' && !(state.games || {}).paused && !gameActive());
  }
  function renderGames() {
    const s = state.settings;
    if (!s) return;
    const g = state.games || {};
    const on = !!s.game_detection;
    const act = gameActive();
    const paused = on && (state.source === 'window' || g.paused);
    $('gb-sub').textContent = !on ? 'Off'
      : paused ? 'Paused while recording an application'
      : act ? (g.locked === act.pid ? `Locked to ${act.name}` : `Tracking ${act.name}`)
      : gamesOnly() ? 'Waiting for a game' : 'Scanning for games';
    const box = $('gb-games');
    box.textContent = '';
    if (on) {
      if (!(g.games || []).length) box.append(h('span', 'gb-empty', 'No games open'));
      for (const game of g.games || []) {
        const isActive = game.pid === g.active;
        const isLocked = game.pid === g.locked;
        const b = h('button', `gchip${isActive ? ' active' : ''}${isLocked ? ' locked' : ''}${isActive && state.phase === 'recording' ? ' rec' : ''}`);
        b.type = 'button';
        b.append(h('span', 'gd'), document.createTextNode(game.name));
        if (isLocked) b.insertAdjacentHTML('beforeend', LOCK_SVG);
        b.dataset.tip = isLocked
          ? `${game.name} is locked. Click to unlock (Encore goes back to the newest game).`
          : `Click to lock ${game.name}: Encore records it until it closes. Right-click for more.`;
        b.addEventListener('click', () => lockGame(isLocked ? null : game.pid));
        b.addEventListener('contextmenu', (e) => {
          e.preventDefault();
          openMenu(e.clientX, e.clientY, [
            { text: isLocked ? 'Unlock' : 'Record this game', run: () => lockGame(isLocked ? null : game.pid) },
            '-',
            { text: `Never a game (${game.exe})`, run: () => addGameToList('games_never', game.exe) },
          ]);
        });
        box.append(b);
      }
    }
    document.body.classList.toggle('gd-wait', gameWaiting());
    document.body.classList.toggle('gd-game', !!(on && !paused && act && state.source === 'desktop'));
    syncTrayGames();
  }
  async function lockGame(pid) {
    try {
      state.games = await bridge.lockGame(pid);
      renderGames();
      renderStage();
    } catch (e) { toast(String(e), 'warning'); }
  }
  async function gameHotkey(action) {
    if (!state.settings || !state.settings.game_detection) { toast('Turn on game detection first (the controller under the screens).', 'warning', 6000); return; }
    try {
      const r = action === 'next_game' ? await bridge.nextGame() : await bridge.lockFocusedGame();
      playCue('save');
      toast(`Recording ${r.locked} until it closes`, 'ok', 4000);
    } catch (e) { toast(String(e), 'warning', 7000); }
  }
  let traySent = '';
  function syncTrayGames() {
    const s = state.settings;
    const g = state.games || {};
    const on = !!(s && s.game_detection);
    const games = (g.games || []).map((x) => ({ pid: x.pid, name: x.name }));
    const rec = state.phase === 'recording';
    const sig = JSON.stringify([on, games, g.active, g.locked, rec]);
    if (sig === traySent) return;
    traySent = sig;
    bridge.setTrayGames(on, games, g.active, g.locked, rec).catch(() => {});
  }
  bridge.onTrayGame((id) => lockGame(id === 'unlock' ? null : Number(id)));

  // Settings → Games: always / never lists, and the two choices.
  function renderGameSettings() {
    const s = state.settings;
    if (!s) return;
    document.querySelectorAll('[data-game-pick]').forEach((o) => o.setAttribute('aria-checked', String(o.dataset.gamePick === s.game_pick)));
    document.querySelectorAll('[data-game-end]').forEach((o) => o.setAttribute('aria-checked', String(o.dataset.gameEnd === s.game_end)));
    for (const [key, id] of [['games_always', 'games-always'], ['games_never', 'games-never']]) {
      const box = $(id);
      box.textContent = '';
      const list = s[key] || [];
      if (!list.length) box.append(h('span', 'gb-empty', 'None'));
      for (const exe of list) {
        const chip = h('span', 'lchip', exe);
        const x = h('button', 'lx');
        x.type = 'button';
        x.setAttribute('aria-label', `Remove ${exe}`);
        x.textContent = '\u00d7';
        x.addEventListener('click', () => saveSetting(key, list.filter((e) => e !== exe)));
        chip.append(x);
        box.append(chip);
      }
    }
  }
  async function addGameToList(key, exe) {
    if (!state.settings || !exe) return;
    const other = key === 'games_always' ? 'games_never' : 'games_always';
    const has = (l) => l.some((e) => e.toLowerCase() === exe.toLowerCase());
    if (has(state.settings[other])) await saveSetting(other, state.settings[other].filter((e) => e.toLowerCase() !== exe.toLowerCase()));
    if (!has(state.settings[key])) await saveSetting(key, [...state.settings[key], exe]);
    toast(key === 'games_always' ? `${exe} will be treated as a game.` : `${exe} won\u2019t be treated as a game.`, 'ok');
  }
  async function pickAppFor(key, btn) {
    let wins = [];
    try { wins = await bridge.listWindows(); } catch { /* none */ }
    const items = wins.filter((w) => w.exe).map((w) => ({ text: `${w.title} (${w.exe})`, run: () => addGameToList(key, w.exe) }));
    const r = btn.getBoundingClientRect();
    openMenu(r.left, r.bottom + 6, items.length ? items : [{ text: 'No apps open', disabled: true, run: () => {} }]);
  }
  // Desktop Capture: off records games only (Echoes waits for a game; the engine enforces the rest).
  $('dc-on').addEventListener('change', async (e) => {
    await saveSetting('desktop_capture', e.target.checked);
    renderStage();
  });
  $('add-always-game').addEventListener('click', (e) => { e.stopPropagation(); pickAppFor('games_always', e.currentTarget); });
  $('add-never-game').addEventListener('click', (e) => { e.stopPropagation(); pickAppFor('games_never', e.currentTarget); });
  document.querySelectorAll('[data-game-pick]').forEach((o) => o.addEventListener('click', () => saveSetting('game_pick', o.dataset.gamePick)));
  document.querySelectorAll('[data-game-end]').forEach((o) => o.addEventListener('click', () => saveSetting('game_end', o.dataset.gameEnd)));

  // ---------- settings contents (click to jump; the current section is highlighted) ----------
  (function buildSettingsNav() {
    const scroller = $('settings-scroll');
    const nav = $('setnav');
    const sections = [...scroller.querySelectorAll('.setsec')];
    const links = sections.map((sec) => {
      const name = sec.querySelector('h3').textContent;
      sec.id = sec.id || `set-${name.toLowerCase().replace(/[^a-z]+/g, '-')}`;
      const b = document.createElement('button');
      b.type = 'button';
      b.textContent = name;
      b.addEventListener('click', () => {
        scroller.scrollTo({ top: sec.offsetTop - 4 });
        mark(sections.indexOf(sec));
        holdUntil = Date.now() + 600; // let the smooth scroll finish before following it
      });
      nav.appendChild(b);
      return b;
    });
    let holdUntil = 0;
    function mark(i) { links.forEach((l, j) => l.setAttribute('aria-current', String(i === j))); }
    function follow() {
      if (Date.now() < holdUntil) return;
      const top = scroller.scrollTop + 40;
      let current = 0;
      sections.forEach((sec, i) => { if (sec.offsetTop <= top) current = i; });
      // At the very bottom, the last section counts as current even if it's short.
      if (scroller.scrollTop + scroller.clientHeight >= scroller.scrollHeight - 2) current = sections.length - 1;
      mark(current);
    }
    scroller.addEventListener('scroll', follow, { passive: true });
    window.addEventListener('resize', follow);
    mark(0);
  })();

  // ---------- start with Windows ----------
  async function refreshAutostart() {
    try { $('autostart').checked = !!(await bridge.autostartStatus()); } catch { /* not available */ }
  }
  $('autostart').addEventListener('change', async (e) => {
    const on = e.target.checked;
    e.target.disabled = true;
    try {
      await bridge.setAutostart(on);
      toast(on ? 'Encore will start in the tray when Windows starts.' : 'Encore won\u2019t start with Windows any more.', 'ok');
    } catch (err) {
      toast(String(err), 'error');
    }
    e.target.disabled = false;
    refreshAutostart();
  });

  async function finishSetup() {
    if (state.settings && !state.settings.setup_done) await saveSetting('setup_done', true);
  }
  async function browseFolder() {
    try {
      const dir = await bridge.pickFolder(state.settings ? state.settings.output_dir : '');
      if (dir) await saveSetting('output_dir', dir);
    } catch (e) { toast(String(e), 'error'); }
    renderSetup();
  }
  $('setup-browse').addEventListener('click', browseFolder);
  $('browse-output').addEventListener('click', browseFolder);
  $('setup-back').addEventListener('click', () => { if (setupStep > 1) { setupStep--; renderSetup(); } });
  $('setup-next').addEventListener('click', async () => {
    if (setupStep < document.querySelectorAll('#setup .sstep').length) { setupStep++; renderSetup(); return; }
    const want = $('setup-admin').checked;
    const change = want !== !!state.startAsAdmin;
    const echoes = $('setup-echo').checked;
    const games = $('setup-games').checked;
    const desktop = !games || $('setup-desktop').checked;
    const autostart = $('setup-autostart').checked;
    closeDialog($('setup'));
    await finishSetup();
    if (state.settings && games !== !!state.settings.game_detection) await saveSetting('game_detection', games);
    if (state.settings && desktop !== !!state.settings.desktop_capture) await saveSetting('desktop_capture', desktop);
    try { if (autostart !== !!(await bridge.autostartStatus())) await bridge.setAutostart(autostart); } catch (e) { toast(String(e), 'error'); }
    if (echoes && !state.replay.on && state.phase === 'idle') await setReplay(true);
    else if (!echoes && state.replay.on) await setReplay(false);
    if (change) await setStartAsAdmin(want, { restart: 'now' });
  });
  $('setup-skip').addEventListener('click', () => closeDialog($('setup')));
  document.querySelectorAll('#setup [data-quality]').forEach((b) =>
    b.addEventListener('click', async () => { await saveSetting('quality', b.dataset.quality); renderSetup(); }));

  // ---------- start-up ----------
  async function init() {
    renderStage();
    try {
      state.settings = await bridge.getSettings();
      renderSettings();
      const status = await bridge.getStatus();
      state.focusMonitor = status.focus_monitor || null;
      state.elevated = !!status.elevated;
      await refreshAdmin();
      state.replay = status.replay || { on: false };
      if (status.games) state.games = status.games;
      if (status.state === 'recording') {
        state.recording = { ...status.recording, source: status.recording.label };
        state.phase = 'recording';
      }
    } catch (e) {
      toast(`Could not reach the recorder: ${e}`, 'error', 0);
    }
    await refreshMonitors();
    await refreshInputs();
    if (state.settings && !state.settings.setup_done && state.phase === 'idle') openSetup();
    // Instant replay was on last time: turn it back on.
    if (state.settings && state.settings.replay_on && !state.replay.on && state.phase === 'idle') setReplay(true, false);
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
