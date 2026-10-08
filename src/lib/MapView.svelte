<script>
  // The world as a grid of squares. "You" are a 3x3 block; connecting draws a one-cell path to
  // the server's country and the block walks it, switching the path off behind itself.
  import { onMount, tick } from 'svelte';
  import world from '../assets/world.json';
  import { app, select, serverById, toast, save } from './store.svelte.js';
  import { countryName } from './flags.js';
  import Flag from './Flag.svelte';
  import { count } from './format.js';

  let { paused = false } = $props();

  const { cols, rows, countries, anchors } = world;
  const cells = Uint8Array.from(atob(world.cells), (c) => c.charCodeAt(0));
  const land = [];
  for (let i = 0; i < cells.length; i++) if (cells[i]) land.push(i);
  const indexOf = Object.fromEntries(countries.map((c, i) => [c, i + 1]));
  const alias = { EU: 'DE', UK: 'GB' };

  const mercator = (lat) => Math.log(Math.tan(Math.PI / 4 + (lat * Math.PI) / 360));
  const yTop = mercator(world.latTop);
  const cellRad = (2 * Math.PI) / cols;

  function cellAt(lat, lon) {
    const col = ((Math.floor(((lon + 180) / 360) * cols) % cols) + cols) % cols;
    const clamped = Math.max(world.latBottom + 0.01, Math.min(world.latTop - 0.01, lat));
    const row = Math.max(0, Math.min(rows - 1, Math.floor((yTop - mercator(clamped)) / cellRad)));
    return [col, row];
  }

  function latLon(col, row) {
    const y = yTop - (row + 0.5) * cellRad;
    return [((2 * Math.atan(Math.exp(y)) - Math.PI / 2) * 180) / Math.PI, ((col + 0.5) / cols) * 360 - 180];
  }

  const anchorOf = (cc) => anchors[alias[cc] ?? cc] ?? null;

  // ---- what the map shows, derived from the app state
  const home = $derived(app.settings?.home ? cellAt(app.settings.home.lat, app.settings.home.lon) : null);
  const serverCountries = $derived(new Set(app.servers.map((s) => indexOf[alias[s.cc] ?? s.cc]).filter(Boolean)));
  const selectedCountry = $derived.by(() => {
    const id = app.status.server ?? (app.settings?.selected !== 'auto' ? app.settings?.selected : null);
    const cc = id ? serverById(id)?.cc : null;
    return cc ? (indexOf[alias[cc] ?? cc] ?? 0) : 0;
  });

  // ---- canvas plumbing
  let wrap = $state();
  let canvas = $state();
  let tipEl = $state();
  let tip = $state(null);
  let ctx;
  let base;
  let geo = { cell: 5, sq: 4, ox: 0, oy: 0, W: 0, H: 0, dpr: 1, mapW: 0, mapH: 0 };
  // On a small screen the whole world would come out as specks, so the cells keep a size worth
  // looking at and the window shows a part of the map: a camera that follows the block.
  let cam = null; // where the map's corner is right now, in device pixels
  let drag = null; // a finger moving the map
  let nudge = [0, 0]; // how far the user has dragged away from where the camera would sit
  let touch = false;
  let tipTimer;
  let colors = null;
  let glow = null;
  let hovered = 0;
  let frame = 0;
  let dirty = true;

  const rgb = (c, a = 1) => `rgba(${c[0]},${c[1]},${c[2]},${a})`;
  const mix = (a, b, t) => [0, 1, 2].map((i) => Math.round(a[i] + (b[i] - a[i]) * t));

  function parseColor(value) {
    const probe = document.createElement('canvas').getContext('2d');
    probe.fillStyle = value.trim();
    const hex = probe.fillStyle;
    return [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16));
  }

  function readColors() {
    const style = getComputedStyle(document.documentElement);
    const light = document.documentElement.dataset.theme === 'light';
    const accent = parseColor(style.getPropertyValue('--accent'));
    const bg = parseColor(style.getPropertyValue('--bg'));
    const ink = light ? mix(accent, [6, 40, 22], 0.45) : accent;
    colors = {
      light,
      accent: ink,
      bright: light ? mix(ink, [0, 0, 0], 0.25) : mix(accent, [255, 255, 255], 0.35),
      danger: parseColor(style.getPropertyValue('--danger')),
      land: rgb(mix(bg, ink, light ? 0.26 : 0.16)),
      served: rgb(mix(bg, ink, light ? 0.5 : 0.36)),
      chosen: rgb(mix(bg, ink, light ? 0.78 : 0.62)),
    };
    const g = document.createElement('canvas');
    g.width = g.height = 96;
    const gc = g.getContext('2d');
    const grad = gc.createRadialGradient(48, 48, 0, 48, 48, 48);
    grad.addColorStop(0, rgb(ink, light ? 0.22 : 0.42));
    grad.addColorStop(0.45, rgb(ink, light ? 0.07 : 0.13));
    grad.addColorStop(1, rgb(ink, 0));
    gc.fillStyle = grad;
    gc.fillRect(0, 0, 96, 96);
    glow = g;
  }

  function layout() {
    if (!wrap || !canvas) return;
    const dpr = window.devicePixelRatio || 1;
    const W = Math.round(wrap.clientWidth * dpr);
    const H = Math.round(wrap.clientHeight * dpr);
    if (!W || !H) return;
    canvas.width = W;
    canvas.height = H;
    const fit = Math.max(3, Math.floor(Math.min((W * 1.0) / cols, (H * 0.9) / rows)));
    const cell = Math.max(fit, wrap.clientWidth <= 720 ? Math.round(5 * dpr) : 0);
    const gap = cell >= 8 ? 2 : 1;
    geo = { cell, sq: cell - gap, W, H, dpr, mapW: cols * cell, mapH: rows * cell, ox: 0, oy: 0 };
    cam = null;
    aim();
    ctx = canvas.getContext('2d');
    drawBase();
  }

  /** The cell the camera keeps in the middle when the map does not fit. */
  function focus() {
    if (phase !== 'idle' && blockCell) return blockCell;
    return home ?? [cols * 0.54, rows * 0.34];
  }

  /** Moves the camera towards its place; true while it is still on the way. */
  function aim() {
    const { cell, W, H, mapW, mapH } = geo;
    const [fc, fr] = focus();
    const place = (view, map, at, drift, shift) => {
      if (map <= view) return (view - map) / 2 - shift;
      return Math.max(view - map, Math.min(0, view / 2 - (at + 0.5) * cell + drift));
    };
    const tx = place(W, mapW, fc, nudge[0], 0);
    const ty = place(H, mapH, fr, nudge[1], H * 0.035);
    if (!cam || drag) cam = [tx, ty];
    const [dx, dy] = [tx - cam[0], ty - cam[1]];
    const moving = Math.abs(dx) > 0.5 || Math.abs(dy) > 0.5;
    cam = moving ? [cam[0] + dx * 0.16, cam[1] + dy * 0.16] : [tx, ty];
    geo.ox = Math.round(cam[0]);
    geo.oy = Math.round(cam[1]);
    return moving;
  }

  function drawBase() {
    if (!colors || !geo.W) return;
    base = base ?? document.createElement('canvas');
    base.width = geo.mapW;
    base.height = geo.mapH;
    const b = base.getContext('2d');
    const { cell, sq } = geo;
    for (const pass of [0, 1, 2]) {
      b.fillStyle = [colors.land, colors.served, colors.chosen][pass];
      for (const i of land) {
        const c = cells[i];
        const level = c === selectedCountry ? 2 : serverCountries.has(c) ? 1 : 0;
        if (level === pass) b.fillRect((i % cols) * cell, Math.floor(i / cols) * cell, sq, sq);
      }
    }
    dirty = true;
  }

  // ---- the animation: one small state machine driven by the connection status
  let phase = 'idle'; // idle | spin | draw | travel | on | fail | back | leave
  let path = [];
  let t0 = 0;
  let head = 0;
  let confirmedAt = 0;
  let confirmedFrom = 0;
  let blockCell = null;
  let rings = [];
  let twinkles = [];
  let lastTwinkle = 0;
  let lastPulse = 0;
  let seen = 'off';

  function makePath(a, b) {
    const dist = Math.hypot(b[0] - a[0], b[1] - a[1]);
    const lift = Math.max(2, dist * 0.22);
    const cx = (a[0] + b[0]) / 2;
    const cy = (a[1] + b[1]) / 2 - lift;
    const out = [];
    const steps = Math.ceil(dist * 3) + 2;
    for (let i = 0; i <= steps; i++) {
      const t = i / steps;
      const x = Math.round((1 - t) ** 2 * a[0] + 2 * (1 - t) * t * cx + t * t * b[0]);
      const y = Math.max(1, Math.round((1 - t) ** 2 * a[1] + 2 * (1 - t) * t * cy + t * t * b[1]));
      const last = out[out.length - 1];
      if (!last || last[0] !== x || last[1] !== y) out.push([x, y]);
    }
    return out;
  }

  function ring(cell, max, dur, alpha, color = 'accent') {
    rings.push({ cell, max, dur, alpha, color, t0: performance.now() });
  }

  function startTravel(from, to, confirmed) {
    path = makePath(from, to);
    head = 0;
    t0 = performance.now();
    confirmedAt = 0;
    confirmedFrom = 0;
    phase = path.length > 2 ? 'draw' : 'on';
    blockCell = phase === 'on' ? to : from;
    if (confirmed) pendingConfirm = true;
  }

  let pendingConfirm = false;

  function onStatus(state, serverId, error, exit) {
    const now = performance.now();
    if (state !== seen) nudge = [0, 0];
    if (state === 'connecting' && seen !== 'connecting') {
      rings = [];
      pendingConfirm = false;
      const dest = anchorOf(serverById(serverId)?.cc ?? '');
      if (home && dest && (dest[0] !== home[0] || dest[1] !== home[1])) startTravel(home, dest, false);
      else {
        phase = 'spin';
        t0 = now;
        blockCell = home;
      }
    } else if (state === 'on' && seen !== 'on') {
      if (phase === 'draw' || phase === 'travel') pendingConfirm = true;
      else if (phase === 'spin') {
        phase = 'on';
        if (blockCell) ring(blockCell, 6, 700, 0.7);
      } else {
        // already connected when the window opened: no journey to show
        const dest = anchorOf(serverById(serverId)?.cc ?? '');
        blockCell = dest ?? home;
        phase = 'on';
      }
    } else if (state === 'off' && seen !== 'off') {
      pendingConfirm = false;
      t0 = now;
      if (seen === 'connecting') phase = error ? 'fail' : 'back';
      else phase = 'leave';
      if (phase !== 'fail' && phase !== 'back' && blockCell) ring(blockCell, 5, 420, 0.5);
    }
    // A server whose country was unknown: once the exit address is located, go there.
    if (state === 'on' && phase === 'on' && exit && home && blockCell && blockCell[0] === home[0] && blockCell[1] === home[1]) {
      const dest = cellAt(exit.lat, exit.lon);
      if (Math.hypot(dest[0] - home[0], dest[1] - home[1]) > 3) startTravel(home, dest, true);
    }
    seen = state;
    dirty = true;
  }

  const ease = (t) => 1 - (1 - t) ** 3;

  function step(now) {
    if (phase === 'draw') {
      // already confirmed while the path is still being drawn: do not make the user wait for the show
      const drawMs = pendingConfirm ? 170 : Math.max(260, Math.min(520, path.length * 6));
      if (now - t0 >= drawMs) {
        phase = 'travel';
        t0 = now;
      }
    }
    if (phase === 'travel') {
      if (pendingConfirm && !confirmedAt) {
        confirmedAt = now;
        confirmedFrom = 0.82 * (1 - Math.exp(-(now - t0) / 620));
      }
      let p;
      if (confirmedAt) {
        const dur = 170 + (1 - confirmedFrom) * 330;
        p = confirmedFrom + (1 - confirmedFrom) * ease(Math.min(1, (now - confirmedAt) / dur));
      } else {
        // creep towards the server but never arrive before the tunnel is confirmed
        p = 0.82 * (1 - Math.exp(-(now - t0) / 620));
      }
      head = p * (path.length - 1);
      blockCell = path[Math.round(head)];
      if (p >= 0.999) {
        phase = 'on';
        blockCell = path[path.length - 1];
        pendingConfirm = false;
        ring(blockCell, 4, 520, 0.9);
        ring(blockCell, 9, 900, 0.55);
        path = [];
      }
    } else if (phase === 'fail') {
      const t = now - t0;
      if (t > 520 && path.length) {
        const back = 1 - ease(Math.min(1, (t - 520) / 380));
        blockCell = path[Math.round(Math.min(head, path.length - 1) * back)];
      }
      if (t > 900) finishReturn();
    } else if (phase === 'back') {
      const t = (now - t0) / 280;
      if (path.length) blockCell = path[Math.round(Math.min(head, path.length - 1) * (1 - ease(Math.min(1, t))))];
      if (t >= 1) finishReturn();
    } else if (phase === 'leave') {
      if (now - t0 > 520) finishReturn();
    }
  }

  function finishReturn() {
    phase = 'idle';
    path = [];
    head = 0;
    blockCell = home;
    if (home) ring(home, 4, 460, 0.6);
  }

  function square(c, r) {
    ctx.fillRect(geo.ox + c * geo.cell, geo.oy + r * geo.cell, geo.sq, geo.sq);
  }

  function drawBlock(cell, alpha, breath, tone = 'accent') {
    if (!cell || alpha <= 0) return;
    const [c, r] = cell;
    const { cell: size, ox, oy, sq } = geo;
    if (glow && tone === 'accent') {
      const span = size * 11;
      ctx.globalAlpha = alpha * (0.75 + breath * 0.25);
      ctx.drawImage(glow, ox + c * size + sq / 2 - span / 2, oy + r * size + sq / 2 - span / 2, span, span);
      ctx.globalAlpha = 1;
    }
    for (let dy = -1; dy <= 1; dy++) {
      for (let dx = -1; dx <= 1; dx++) {
        const centre = dx === 0 && dy === 0;
        const level = centre ? 1 : dx === 0 || dy === 0 ? 0.5 + breath * 0.14 : 0.27 + breath * 0.1;
        ctx.fillStyle = rgb(centre ? (tone === 'accent' ? colors.bright : colors[tone]) : colors[tone], level * alpha);
        square(c + dx, r + dy);
      }
    }
  }

  function draw(now) {
    const full = app.settings?.effects !== 'lite';
    ctx.clearRect(0, 0, geo.W, geo.H);
    if (base) ctx.drawImage(base, geo.ox, geo.oy);

    if (full && phase !== 'fail') {
      // a slow diagonal sweep of light, and the odd cell that flickers
      const cycle = 9000;
      const span = cols + rows * 0.5 + 30;
      const pos = ((now % cycle) / 5200) * span - 15;
      if (pos < span) {
        for (const i of land) {
          const d = Math.abs((i % cols) + Math.floor(i / cols) * 0.5 - pos);
          if (d < 7) {
            ctx.fillStyle = rgb(colors.accent, (1 - d / 7) ** 2 * (colors.light ? 0.3 : 0.2));
            square(i % cols, Math.floor(i / cols));
          }
        }
      }
      if (now - lastTwinkle > 150) {
        lastTwinkle = now;
        twinkles.push({ i: land[(Math.random() * land.length) | 0], t0: now, dur: 900 + Math.random() * 900 });
      }
      twinkles = twinkles.filter((t) => now - t.t0 < t.dur);
      for (const t of twinkles) {
        const k = Math.sin(((now - t.t0) / t.dur) * Math.PI);
        ctx.fillStyle = rgb(colors.accent, k * (colors.light ? 0.5 : 0.38));
        square(t.i % cols, Math.floor(t.i / cols));
      }
    }

    if (hovered) {
      ctx.fillStyle = rgb(colors.accent, serverCountries.has(hovered) ? 0.55 : 0.22);
      for (const i of land) if (cells[i] === hovered) square(i % cols, Math.floor(i / cols));
    }

    // the path: revealed cell by cell, then switched off behind the block
    if (path.length && (phase === 'draw' || phase === 'travel' || phase === 'fail')) {
      const drawMs = pendingConfirm ? 170 : Math.max(260, Math.min(520, path.length * 6));
      const revealed = phase === 'draw' ? Math.floor(Math.min(1, (now - t0) / drawMs) * path.length) : path.length;
      const from = phase === 'draw' ? 0 : Math.round(head) + 2;
      const tone = phase === 'fail' ? colors.danger : colors.accent;
      const blink = phase === 'fail' ? 0.35 + 0.65 * Math.abs(Math.cos((now - t0) / 85)) : 1;
      for (let i = from; i < revealed; i++) {
        const fresh = phase === 'draw' ? Math.max(0, 1 - (revealed - i) / 10) : 0;
        ctx.fillStyle = rgb(fresh > 0 ? mix(tone, [255, 255, 255], fresh * 0.6) : tone, (0.78 + fresh * 0.22) * blink);
        square(path[i][0], path[i][1]);
      }
      const end = path[path.length - 1];
      if (phase !== 'fail' && revealed >= path.length) {
        ctx.fillStyle = rgb(colors.bright, 0.55 + 0.45 * Math.abs(Math.sin(now / 160)));
        square(end[0], end[1]);
      }
    }

    // where you started, while the block is away
    const away = home && blockCell && (blockCell[0] !== home[0] || blockCell[1] !== home[1]);
    if (away) {
      ctx.fillStyle = rgb(colors.accent, 0.3);
      for (let dy = -1; dy <= 1; dy++) for (let dx = -1; dx <= 1; dx++) if (dx || dy) square(home[0] + dx, home[1] + dy);
    }

    rings = rings.filter((r) => now - r.t0 < r.dur);
    for (const r of rings) {
      const t = (now - r.t0) / r.dur;
      const radius = Math.round(1 + ease(t) * r.max);
      ctx.fillStyle = rgb(colors[r.color], (1 - t) * r.alpha * (colors.light ? 1 : 0.8));
      const [c, y] = r.cell;
      for (let k = -radius; k <= radius; k++) {
        square(c + k, y - radius);
        square(c + k, y + radius);
        if (k > -radius && k < radius) {
          square(c - radius, y + k);
          square(c + radius, y + k);
        }
      }
    }

    const breath = full ? 0.5 + 0.5 * Math.sin(now / 520) : 0.5;
    if (phase === 'leave') {
      const t = (now - t0) / 520;
      if (t < 0.42) drawBlock(blockCell, 1 - t / 0.42, breath);
      else drawBlock(home, (t - 0.42) / 0.58, breath);
    } else if (phase === 'spin') {
      drawBlock(blockCell, 1, breath);
      if (blockCell) {
        const orbit = [[-2, -2], [0, -2], [2, -2], [2, 0], [2, 2], [0, 2], [-2, 2], [-2, 0]];
        const at = Math.floor((now - t0) / 90) % 8;
        for (let k = 0; k < 3; k++) {
          const [dx, dy] = orbit[(at - k + 8) % 8];
          ctx.fillStyle = rgb(colors.accent, 0.85 - k * 0.28);
          square(blockCell[0] + dx, blockCell[1] + dy);
        }
      }
    } else {
      drawBlock(blockCell ?? home, 1, breath, phase === 'fail' ? 'danger' : 'accent');
    }

    if (phase === 'on' && blockCell) {
      // traffic shows as rings leaving the block: the more is flowing, the faster they come
      const flow = app.traffic.up + app.traffic.down;
      const gap = Math.max(300, 1500 - Math.log10(flow + 1) * 190);
      if (flow > 2048 && now - lastPulse > gap) {
        lastPulse = now;
        ring(blockCell, 3 + Math.min(4, Math.floor(Math.log10(flow + 1) - 3)), 900, 0.45);
      }
    }
  }

  function loop(now) {
    frame = 0;
    if (!ctx || !colors || paused || document.hidden) return;
    step(now);
    const panning = aim();
    draw(now);
    dirty = false;
    const moving = phase !== 'idle' && phase !== 'on';
    if (app.settings?.effects !== 'lite' || moving || panning || rings.length) frame = requestAnimationFrame(loop);
  }

  function wake() {
    if (!frame && !paused) frame = requestAnimationFrame(loop);
  }

  // ---- pointer: hover a country, click to pick its best server (or to place "you")
  function cellFromEvent(e) {
    const rect = canvas.getBoundingClientRect();
    const col = Math.floor(((e.clientX - rect.left) * geo.dpr - geo.ox) / geo.cell);
    const row = Math.floor(((e.clientY - rect.top) * geo.dpr - geo.oy) / geo.cell);
    return col >= 0 && col < cols && row >= 0 && row < rows ? [col, row] : null;
  }

  function serversIn(index) {
    return app.servers.filter((s) => indexOf[alias[s.cc] ?? s.cc] === index);
  }

  function countryTip(index) {
    const list = serversIn(index);
    const pings = list.map((s) => s.ping).filter((p) => p > 0);
    return { cc: countries[index - 1], index, n: list.length, best: pings.length ? Math.min(...pings) : null };
  }

  function placeTip(e) {
    if (!tipEl) return;
    const rect = wrap.getBoundingClientRect();
    const x = Math.max(8, Math.min(e.clientX - rect.left + 16, rect.width - (touch ? 250 : 220)));
    const y = Math.min(e.clientY - rect.top + 18, rect.height - 56);
    tipEl.style.transform = `translate(${Math.round(x)}px, ${Math.round(y)}px)`;
  }

  function onDown(e) {
    touch = e.pointerType === 'touch';
    if (geo.mapW <= geo.W && geo.mapH <= geo.H) return;
    drag = { x: e.clientX, y: e.clientY, from: [...nudge], camFrom: cam ? [...cam] : [0, 0], moved: false };
    try {
      canvas.setPointerCapture(e.pointerId);
    } catch {}
  }

  function onUp() {
    // the click that follows a drag is not a click on a country
    if (drag?.moved) setTimeout(() => (drag = null), 0);
    else drag = null;
  }

  function onMove(e) {
    if (drag) {
      const [dx, dy] = [(e.clientX - drag.x) * geo.dpr, (e.clientY - drag.y) * geo.dpr];
      if (!drag.moved && Math.hypot(dx, dy) < 8 * geo.dpr) return;
      if (!drag.moved) {
        // from here the map goes where the finger goes: count the drift from where it stands now
        const [fc, fr] = focus();
        drag.centre = [geo.W / 2 - (fc + 0.5) * geo.cell, geo.H / 2 - (fr + 0.5) * geo.cell];
        drag.base = [drag.camFrom[0] - drag.centre[0], drag.camFrom[1] - drag.centre[1]];
        drag.moved = true;
        tip = null;
      }
      // no further than the edge of the map, so that dragging back answers at once
      const held = (view, map, centre, drift) => (map > view ? Math.max(view - map, Math.min(0, centre + drift)) - centre : 0);
      nudge = [held(geo.W, geo.mapW, drag.centre[0], drag.base[0] + dx), held(geo.H, geo.mapH, drag.centre[1], drag.base[1] + dy)];
      wake();
      return;
    }
    if (touch) return;
    const cell = cellFromEvent(e);
    const index = cell ? cells[cell[1] * cols + cell[0]] : 0;
    if (index !== hovered) {
      hovered = index;
      tip = index ? countryTip(index) : null;
      canvas.style.cursor = app.pickHome ? 'crosshair' : index && serverCountries.has(index) ? 'pointer' : 'default';
      wake();
    }
    placeTip(e);
  }

  function onLeave() {
    if (touch) return;
    hovered = 0;
    tip = null;
    wake();
  }

  function pick(index) {
    const list = serversIn(index);
    if (!list.length) return;
    const reachable = list.filter((s) => s.ping > 0).sort((a, b) => a.ping - b.ping);
    const server = reachable[0] ?? list[0];
    select(server.id);
    toast(`Выбран сервер: ${server.name}`);
  }

  function onClick(e) {
    if (drag?.moved) return;
    const cell = cellFromEvent(e);
    if (!cell) return;
    const index = cells[cell[1] * cols + cell[0]];
    if (app.pickHome) {
      const [lat, lon] = latLon(cell[0], cell[1]);
      app.settings.home = { lat, lon, cc: index ? countries[index - 1] : '', ip: '', manual: true };
      app.pickHome = false;
      save();
      toast('Местоположение задано', 'ok');
      return;
    }
    if (!touch) return pick(index);
    // A finger lands on the map by accident far too easily to change the server with it.
    // A tap only says what is there; choosing takes the button in the note.
    clearTimeout(tipTimer);
    hovered = index;
    tip = index ? countryTip(index) : null;
    wake();
    if (tip) {
      tick().then(() => placeTip(e));
      tipTimer = setTimeout(() => {
        tip = null;
        hovered = 0;
        wake();
      }, 3200);
    }
  }

  function pickFromTip() {
    clearTimeout(tipTimer);
    const index = tip?.index;
    tip = null;
    hovered = 0;
    wake();
    if (index) pick(index);
  }

  onMount(() => {
    readColors();
    layout();
    const ro = new ResizeObserver(() => {
      layout();
      wake();
    });
    ro.observe(wrap);
    const onVisible = () => wake();
    document.addEventListener('visibilitychange', onVisible);
    return () => {
      ro.disconnect();
      document.removeEventListener('visibilitychange', onVisible);
      cancelAnimationFrame(frame);
    };
  });

  $effect(() => {
    // theme or accent changed: repaint with the new colours once the stylesheet has caught up
    app.settings?.theme;
    app.settings?.accent;
    requestAnimationFrame(() => {
      if (!canvas) return;
      readColors();
      drawBase();
      wake();
    });
  });

  $effect(() => {
    serverCountries;
    selectedCountry;
    if (canvas && colors) {
      drawBase();
      wake();
    }
  });

  $effect(() => {
    const s = app.status;
    onStatus(s.state, s.server, s.error, s.exit);
    wake();
  });

  $effect(() => {
    if (home && phase === 'idle') blockCell = home;
    app.settings?.effects;
    app.pickHome;
    if (!paused) wake();
  });
</script>

<div class="map" bind:this={wrap} class:pick={app.pickHome}>
  <canvas bind:this={canvas} onpointerdown={onDown} onpointerup={onUp} onpointercancel={onUp} onpointermove={onMove}
          onpointerleave={onLeave} onclick={onClick}></canvas>

  {#if tip && !app.pickHome}
    <div class="tip" class:touch bind:this={tipEl}>
      <Flag cc={tip.cc} cell={2} />
      <div>
        <b>{countryName(tip.cc)}</b>
        <span class="mono">
          {#if tip.n}{count(tip.n, 'сервер', 'сервера', 'серверов')}{#if tip.best} · {tip.best} мс{/if}{:else}нет серверов{/if}
        </span>
      </div>
      {#if touch && tip.n}<button class="btn" onclick={pickFromTip}>Выбрать</button>{/if}
    </div>
  {/if}

  {#if app.pickHome}
    <div class="banner">
      <span>Укажи на карте, где ты находишься</span>
      <button class="btn" onclick={() => (app.pickHome = false)}>Отмена</button>
    </div>
  {:else if app.ready && !home}
    <div class="banner quiet">
      <span>Местоположение неизвестно</span>
      <button class="btn" onclick={() => (app.pickHome = true)}>Указать на карте</button>
    </div>
  {/if}
</div>

<style>
  .map {
    position: absolute;
    inset: 0;
    overflow: hidden;
  }
  canvas {
    width: 100%;
    height: 100%;
    display: block;
    touch-action: none;
  }
  .tip.touch {
    pointer-events: auto;
    padding-right: 8px;
  }
  .tip .btn {
    height: 30px;
    margin-left: 6px;
  }
  .tip {
    position: absolute;
    left: 0;
    top: 0;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 12px 8px 9px;
    background: var(--surface-2);
    border: 1px solid var(--line-2);
    border-radius: var(--r);
    box-shadow: var(--shadow);
    pointer-events: none;
    white-space: nowrap;
    animation: fade-in var(--t-fast);
    will-change: transform;
  }
  .tip div {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .tip b {
    font-weight: 600;
  }
  .tip span {
    font-size: 11px;
    color: var(--text-2);
  }
  .banner {
    position: absolute;
    left: 50%;
    top: 64px;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 8px 8px 8px 14px;
    background: var(--surface-2);
    border: 1px solid var(--accent-line);
    border-radius: var(--r);
    box-shadow: var(--shadow);
    animation: fade-in var(--t) var(--ease);
  }
  .banner.quiet {
    border-color: var(--line-2);
    color: var(--text-2);
  }
  .banner {
    max-width: calc(100% - 24px);
    white-space: nowrap;
  }
  .banner span {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .banner .btn {
    height: 28px;
    flex: none;
  }
  :global([data-platform='android']) .banner {
    top: 12px;
  }
</style>
