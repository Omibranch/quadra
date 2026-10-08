// Stand-in backend for running the interface in a browser. Mirrors the commands and events of
// src-tauri/src/main.rs closely enough to exercise every screen.
const handlers = new Map();
const emit = (name, payload) => (handlers.get(name) ?? []).forEach((fn) => fn(payload));

const srv = (id, sub, name, cc, proto, transport, security, ping, note = '') => ({
  id, sub, name, cc, proto, transport, security, address: `${id}.example.net`, port: 443, note, full: proto === 'xray', ping,
});

const state = {
  settings: {
    theme: 'dark', accent: '#3ddc84', effects: 'full', mode: 'proxy', socks_port: 20808, http_port: 20809,
    allow_lan: false, routing: 'lan', rules: [{ kind: 'domain', value: 'example.org', action: 'direct' }],
    bypass_apps: ['Steam.exe'], bypass_domains: ['example.org', 'bank.example'],
    dns: '1.1.1.1, 8.8.8.8', close_to_tray: true, autostart: false, autoconnect: false, send_hwid: true,
    user_agent: '', geo_lookup: true, home: { lat: 55.75, lon: 37.62, cc: 'RU', ip: '203.0.113.7', manual: false },
    sub_update_hours: 12, log_level: 'warning', selected: 'a2', collapsed: [],
  },
  subs: [
    { id: 's1', url: 'https://sub.example/one', title: 'ArrowVPN', upload: 0, download: 6.4e9, total: 21474836480,
      expire: Math.floor(Date.now() / 1000) + 59 * 86400, support_url: 'https://t.me/example', web_url: '',
      announce: 'Трафик 20 Гб в день\nОсталось дней: 59', update_hours: 1, updated_at: Math.floor(Date.now() / 1000) - 1300, error: '' },
    { id: 's2', url: 'https://sub.example/two', title: 'Total VPN', upload: 0, download: 0, total: 0,
      expire: Math.floor(Date.now() / 1000) + 2 * 86400, support_url: '', web_url: '',
      announce: 'Подписка действует еще 2 дней', update_hours: 1, updated_at: Math.floor(Date.now() / 1000) - 90000,
      error: 'у провайдера занят лимит устройств: удали старое устройство в личном кабинете' },
  ],
  servers: [
    srv('a1', 's1', 'Великобритания', 'GB', 'vless', 'tcp', 'reality', 71),
    srv('a2', 's1', 'Германия', 'DE', 'vless', 'tcp', 'reality', 48),
    srv('a3', 's1', 'Швейцария', 'CH', 'vless', 'tcp', 'reality', 55),
    srv('a4', 's1', 'Польша', 'PL', 'vless', 'tcp', 'reality', 39),
    srv('a5', 's1', 'Нидерланды', 'NL', 'vless', 'tcp', 'reality', 52),
    srv('a6', 's1', 'США (для ИИ)', 'US', 'vless', 'tcp', 'reality', 142),
    srv('a7', 's1', 'Индия', 'IN', 'vless', 'tcp', 'reality', 188),
    srv('a8', 's1', 'Швеция', 'SE', 'shadowsocks', 'tcp', 'none', -1),
    srv('b1', 's2', 'Авто-выбор', 'EU', 'xray', 'tcp', 'reality', 44, 'Лучший сервер'),
    srv('b2', 's2', 'Эстония', 'EE', 'vless', 'xhttp', 'tls', 36),
    srv('b3', 's2', 'Финляндия', 'FI', 'vless', 'ws', 'tls', 33),
    srv('b4', 's2', 'США | Gemini', 'US', 'vless', 'tcp', 'reality', 151),
    srv('b5', 's2', 'Япония', 'JP', 'vless', 'grpc', 'reality', 210),
    srv('b6', 's2', 'Бразилия', 'BR', 'trojan', 'tcp', 'tls', 240),
    srv('m1', null, 'Свой сервер', 'TR', 'vless', 'tcp', 'reality', null),
  ],
  status: { state: 'off', server: null, since: 0, ms: null, error: null, exit: null, tun: false },
  hwid: '6F3A1C22-9B4E-47D0-A1F8-5C2E7D903B16',
  admin: false,
  elevated: false,
  platform: new URLSearchParams(location.search).get('platform') ?? 'windows',
  tiling: new URLSearchParams(location.search).has('tiling'),
  relaunchForTun: true,
  version: '0.1.0',
  userAgent: 'Quadra/0.1.0',
};

const snapshot = () => JSON.parse(JSON.stringify(state));
const logs = ['Xray 26.9.30 started', '[Warning] core: sample warning line'];
let traffic = null;
let pending = null;
const wait = (ms) => new Promise((r) => setTimeout(r, ms));
const params = new URLSearchParams(location.search);

function setStatus(patch) {
  Object.assign(state.status, patch);
  emit('status', { ...state.status });
}

function stopTraffic() {
  clearInterval(traffic);
  traffic = null;
}

const commands = {
  // ?boot=3000 holds the startup back, to look at the intro
  get_state: async () => {
    await wait(Number(params.get('boot') ?? 0));
    return snapshot();
  },
  ready: () => {},
  save_settings: ({ settings }) => { state.settings = settings; },
  async connect({ id }) {
    stopTraffic();
    const token = (pending = {});
    setStatus({ state: 'connecting', server: id, error: null, exit: null, ms: null });
    await wait(Number(params.get('delay') ?? 1500));
    if (pending !== token) return;
    const server = state.servers.find((s) => s.id === id);
    if (!server || server.ping === -1 || params.has('fail')) {
      setStatus({ state: 'off', error: 'сервер не отвечает: соединение установить не удалось' });
      throw 'сервер не отвечает: соединение установить не удалось';
    }
    setStatus({ state: 'on', since: Math.floor(Date.now() / 1000), ms: server.ping ?? 60 });
    let tup = 0, tdown = 0, t = 0;
    traffic = setInterval(() => {
      t += 1;
      const down = Math.round((0.5 + 0.5 * Math.sin(t / 3)) * 4.2e6 * (0.6 + Math.random() * 0.8));
      const up = Math.round(down * 0.06);
      tup += up; tdown += down;
      emit('traffic', { up, down, tup, tdown });
    }, 1000);
    setTimeout(() => pending === token && setStatus({ exit: { ip: '198.51.100.24', cc: server.cc === 'EU' ? 'NL' : server.cc, lat: 52.37, lon: 4.9 } }), 600);
  },
  async disconnect() {
    pending = null;
    stopTraffic();
    setStatus({ state: 'off', since: 0, ms: null, error: null, exit: null });
  },
  async ping({ ids }) {
    for (const s of state.servers) {
      if (ids.length && !ids.includes(s.id)) continue;
      await wait(40);
      s.ping = s.id === 'a8' ? -1 : Math.round(25 + Math.random() * 220);
      emit('ping', { id: s.id, ms: s.ping });
    }
  },
  async add_subscription({ url }) {
    await wait(700);
    if (!/^https?:\/\//.test(url)) throw 'это не ссылка на подписку';
    const id = 's' + (state.subs.length + 1);
    state.subs.push({ id, url, title: new URL(url).host, upload: 0, download: 0, total: 0, expire: 0, support_url: '', web_url: '', announce: '', update_hours: 0, updated_at: Math.floor(Date.now() / 1000), error: '' });
    state.servers.push(srv(id + 'x', id, 'Франция', 'FR', 'vless', 'tcp', 'reality', null), srv(id + 'y', id, 'Канада', 'CA', 'vless', 'tcp', 'reality', null));
    emit('data', snapshot());
    return { servers: 2, skipped: 0 };
  },
  async refresh_subscription({ id }) {
    await wait(700);
    const sub = state.subs.find((s) => s.id === id);
    sub.updated_at = Math.floor(Date.now() / 1000);
    sub.error = '';
    emit('data', snapshot());
    return { servers: state.servers.filter((s) => s.sub === id).length, skipped: 0 };
  },
  async delete_subscription({ id }) {
    state.subs = state.subs.filter((s) => s.id !== id);
    state.servers = state.servers.filter((s) => s.sub !== id);
    emit('data', snapshot());
  },
  async delete_server({ id }) {
    state.servers = state.servers.filter((s) => s.id !== id);
    emit('data', snapshot());
  },
  async import_text({ text }) {
    await wait(400);
    if (/^https?:\/\//.test(text.trim())) return (await commands.add_subscription({ url: text.trim() })) && { servers: 0, subs: 1, skipped: 0, errors: [] };
    if (!text.includes('://')) throw 'здесь нет ни ссылок на серверы, ни подписок';
    state.servers.push(srv('m' + Date.now(), null, 'Импортированный', 'IT', 'vless', 'ws', 'tls', null));
    emit('data', snapshot());
    return { servers: 1, subs: 0, skipped: 0, errors: [] };
  },
  async locate_home() {
    await wait(500);
    state.settings.home = { lat: 55.75, lon: 37.62, cc: 'RU', ip: '203.0.113.7', manual: false };
    emit('data', snapshot());
    return state.settings.home;
  },
  share: ({ id }) => {
    const size = 29;
    let cells = '';
    for (let i = 0; i < size * size; i++) {
      const [x, y] = [i % size, Math.floor(i / size)];
      const finder = (x < 7 && y < 7) || (x > size - 8 && y < 7) || (x < 7 && y > size - 8);
      const fx = x < 7 ? x : x - (size - 7), fy = y < 7 ? y : y - (size - 7);
      cells += finder ? ((fx === 0 || fx === 6 || fy === 0 || fy === 6 || (fx > 1 && fx < 5 && fy > 1 && fy < 5)) ? '1' : '0') : ((x * 7 + y * 13 + x * y) % 3 === 0 ? '1' : '0');
    }
    return { text: `vless://00000000-0000-0000-0000-000000000000@${id}.example.net:443?type=tcp&security=reality#sample`, link: true, qr: { size, cells } };
  },
  open_url: () => true,
  clipboard: () => 'vless://00000000-0000-0000-0000-000000000000@example.net:443?type=ws&security=tls#sample',
  get_logs: () => logs.slice(),
  clear_logs: () => { logs.length = 0; },
  relaunch_admin: async () => { throw 'запуск с правами администратора отменён'; },
  quit: () => {},
};

export async function invoke(cmd, args = {}) {
  if (!commands[cmd]) throw `unknown command ${cmd}`;
  return commands[cmd](args);
}

export function listen(name, fn) {
  if (!handlers.has(name)) handlers.set(name, []);
  handlers.get(name).push(fn);
  return Promise.resolve(() => handlers.set(name, handlers.get(name).filter((f) => f !== fn)));
}

export const appWindow = {
  minimize: async () => {},
  toggleMaximize: async () => {},
  close: async () => {},
  isMaximized: async () => false,
  onResized: async () => () => {},
};
