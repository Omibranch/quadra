import { invoke, listen } from './api.js';

export const app = $state({
  ready: false,
  settings: null,
  subs: [],
  servers: [],
  status: { state: 'off', server: null, since: 0, ms: null, error: null, exit: null, tun: false },
  hwid: '',
  admin: false,
  elevated: false,
  platform: 'windows',
  tiling: false,
  relaunchForTun: false,
  version: '',
  userAgent: '',
  traffic: { up: 0, down: 0, tup: 0, tdown: 0 },
  history: [],
  panel: null, // 'routes' | 'log' | 'settings'
  popup: null, // { kind, ... }
  menu: null, // context menu { x, y, items }
  toasts: [],
  query: '',
  pinging: false,
  busy: {},
  pickHome: false,
  now: Date.now(),
  logTick: 0,
});

/** Core output. Kept outside the reactive state: thousands of lines, read only by the log panel. */
export const logLines = [];

const HISTORY = 48;
let toastId = 0;

export function toast(text, kind = 'info') {
  const id = ++toastId;
  app.toasts.push({ id, text, kind });
  setTimeout(() => {
    app.toasts = app.toasts.filter((t) => t.id !== id);
  }, kind === 'error' ? 6500 : 3600);
}

const fail = (e) => toast(typeof e === 'string' ? e : (e?.message ?? 'что-то пошло не так'), 'error');

let saveTimer;
export function save() {
  clearTimeout(saveTimer);
  saveTimer = setTimeout(() => invoke('save_settings', { settings: $state.snapshot(app.settings) }).catch(fail), 220);
}

export function set(key, value) {
  app.settings[key] = value;
  save();
}

function applyData(d, first = false) {
  app.subs = d.subs;
  app.servers = d.servers;
  if (first) {
    app.settings = d.settings;
    app.status = d.status;
    app.hwid = d.hwid;
    app.admin = d.admin;
    app.elevated = d.elevated;
    app.platform = d.platform ?? 'windows';
    app.tiling = !!d.tiling;
    app.relaunchForTun = !!d.relaunchForTun;
    app.version = d.version;
    app.userAgent = d.userAgent;
  } else {
    app.settings.home = d.settings.home;
  }
}

export const serverById = (id) => app.servers.find((s) => s.id === id);

export function bestServer() {
  let best = null;
  for (const s of app.servers) if (s.ping > 0 && (!best || s.ping < best.ping)) best = s;
  return best ?? app.servers[0] ?? null;
}

/** The server a click on "connect" will use right now. */
export function target() {
  const sel = app.settings?.selected;
  if (sel && sel !== 'auto') return serverById(sel) ?? null;
  return bestServer();
}

export async function pingAll(ids = []) {
  if (app.pinging || !app.servers.length) return;
  app.pinging = true;
  try {
    await invoke('ping', { ids });
  } catch (e) {
    fail(e);
  } finally {
    app.pinging = false;
  }
}

export async function connect() {
  if (!app.servers.length) {
    app.popup = { kind: 'add' };
    return;
  }
  if (app.settings.mode === 'tun' && app.relaunchForTun) {
    app.popup = { kind: 'admin' };
    return;
  }
  const auto = !app.settings.selected || app.settings.selected === 'auto';
  if (auto && !app.servers.some((s) => s.ping > 0)) await pingAll();
  const server = target();
  if (!server) return;
  if (app.status.state === 'connecting' && app.status.server === server.id) return;
  try {
    await invoke('connect', { id: server.id });
  } catch {
    // the reason arrives with the status event and is shown from there
  }
}

export const disconnect = () => invoke('disconnect').catch(fail);

export function toggle() {
  return app.status.state === 'off' ? connect() : disconnect();
}

export function select(id) {
  if (app.settings.selected === id && app.status.state !== 'off') return;
  set('selected', id);
  if (app.status.state !== 'off') connect();
}

async function busy(key, fn) {
  if (app.busy[key]) return;
  app.busy[key] = true;
  try {
    return await fn();
  } finally {
    delete app.busy[key];
  }
}

export function importText(text) {
  return busy('import', async () => {
    try {
      const r = await invoke('import_text', { text });
      const parts = [];
      if (r.subs) parts.push(`подписок: ${r.subs}`);
      if (r.servers) parts.push(`серверов: ${r.servers}`);
      toast(`Добавлено — ${parts.join(', ')}`, 'ok');
      for (const e of r.errors ?? []) toast(e, 'error');
      if (r.skipped) toast(`Пропущено неподдерживаемых ссылок: ${r.skipped}`);
      pingAll();
      return true;
    } catch (e) {
      fail(e);
      return false;
    }
  });
}

export function refreshSub(id) {
  return busy('sub:' + id, async () => {
    try {
      await invoke('refresh_subscription', { id });
      toast('Подписка обновлена', 'ok');
      pingAll(app.servers.filter((s) => s.sub === id).map((s) => s.id));
    } catch (e) {
      fail(e);
    }
  });
}

export const refreshAll = () => Promise.all(app.subs.map((s) => refreshSub(s.id)));

export const deleteSub = (id) => invoke('delete_subscription', { id }).catch(fail);
export const deleteServer = (id) => invoke('delete_server', { id }).catch(fail);

export async function locateHome() {
  try {
    await invoke('locate_home');
    return true;
  } catch (e) {
    fail(e);
    return false;
  }
}

export async function copy(text, done = 'Скопировано') {
  try {
    await navigator.clipboard.writeText(text);
    toast(done, 'ok');
  } catch {
    toast('Не удалось скопировать', 'error');
  }
}

export async function init() {
  applyData(await invoke('get_state'), true);

  await listen('status', (s) => {
    const before = app.status.state;
    app.status = s;
    if (s.state === 'off') {
      app.traffic = { up: 0, down: 0, tup: 0, tdown: 0 };
      app.history = [];
    }
    if (s.error && before !== 'off') toast(s.error, 'error');
  });
  await listen('data', (d) => {
    applyData(d);
    const fresh = app.servers.filter((s) => s.ping == null).map((s) => s.id);
    if (fresh.length) pingAll(fresh);
  });
  await listen('ping', ({ id, ms }) => {
    const s = serverById(id);
    if (s) s.ping = ms;
  });
  await listen('traffic', (t) => {
    app.traffic = t;
    app.history = [...app.history.slice(-(HISTORY - 1)), { up: t.up, down: t.down }];
  });
  let logQueued = false;
  await listen('log', (line) => {
    logLines.push(line);
    if (logLines.length > 1500) logLines.splice(0, logLines.length - 1500);
    if (!logQueued) {
      logQueued = true;
      requestAnimationFrame(() => {
        logQueued = false;
        app.logTick += 1;
      });
    }
  });
  await listen('tray-toggle', toggle);
  logLines.push(...(await invoke('get_logs')));

  setInterval(() => {
    if (app.status.state === 'on') app.now = Date.now();
  }, 1000);

  app.ready = true;

  if (!app.settings.home && app.settings.geo_lookup && app.status.state === 'off') invoke('locate_home').catch(() => {});
  if (app.servers.length) pingAll();
  if (app.status.state === 'off' && (app.settings.autoconnect || (app.elevated && app.settings.mode === 'tun'))) connect();
}
