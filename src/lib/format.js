const units = ['Б', 'КБ', 'МБ', 'ГБ', 'ТБ'];

export function bytes(n) {
  let v = Number(n) || 0;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i += 1;
  }
  return `${v >= 100 || i === 0 ? Math.round(v) : v.toFixed(1)} ${units[i]}`;
}

export const speed = (n) => `${bytes(n)}/с`;

export function duration(seconds) {
  const s = Math.max(0, Math.floor(seconds));
  const pad = (v) => String(v).padStart(2, '0');
  const h = Math.floor(s / 3600);
  return h ? `${h}:${pad(Math.floor((s % 3600) / 60))}:${pad(s % 60)}` : `${pad(Math.floor(s / 60))}:${pad(s % 60)}`;
}

function plural(n, one, few, many) {
  const a = Math.abs(n) % 100;
  const b = a % 10;
  if (a > 10 && a < 20) return many;
  if (b === 1) return one;
  if (b >= 2 && b <= 4) return few;
  return many;
}

export const count = (n, one, few, many) => `${n} ${plural(n, one, few, many)}`;

/** "ещё 59 дней", "истекла" or '' when the provider gives no date. */
export function expiry(unix) {
  if (!unix) return '';
  const days = Math.ceil((unix * 1000 - Date.now()) / 86400000);
  if (days <= 0) return 'истекла';
  return `ещё ${count(days, 'день', 'дня', 'дней')}`;
}

export function ago(unix) {
  if (!unix) return 'никогда';
  const m = Math.floor((Date.now() / 1000 - unix) / 60);
  if (m < 1) return 'только что';
  if (m < 60) return `${count(m, 'минуту', 'минуты', 'минут')} назад`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${count(h, 'час', 'часа', 'часов')} назад`;
  return `${count(Math.floor(h / 24), 'день', 'дня', 'дней')} назад`;
}

/** 0..4 filled cells for a latency in ms; -1 means unreachable. */
export function pingLevel(ms) {
  if (ms == null) return 0;
  if (ms < 0) return -1;
  if (ms < 60) return 4;
  if (ms < 120) return 3;
  if (ms < 220) return 2;
  return 1;
}

export const protoLabel = (s) => {
  if (s.full) return 'XRAY';
  const p = { shadowsocks: 'SS', vless: 'VLESS', vmess: 'VMESS', trojan: 'TROJAN' }[s.proto] ?? (s.proto || '').toUpperCase();
  const t = s.security === 'reality' ? 'REALITY' : s.transport && s.transport !== 'tcp' ? s.transport.toUpperCase() : s.security === 'tls' ? 'TLS' : '';
  return t ? `${p} · ${t}` : p;
};
