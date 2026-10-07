// Takes the README screenshots and the frames of the connect animation from the running debug
// app (tools/dev-run.ps1), using the stand-in backend so no real server or address is shown.
//   node tools/shots.mjs <output folder>
import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const out = process.argv[2] ?? 'docs/img';
mkdirSync(join(out, 'frames'), { recursive: true });
const base = 'http://127.0.0.1:1420/';
const pages = await (await fetch('http://127.0.0.1:9222/json')).json();
const page = pages.find((p) => p.type === 'page' && !p.url.includes('splash'));
const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((ok, bad) => { ws.onopen = ok; ws.onerror = bad; });
let seq = 0;
const call = (method, params = {}) => new Promise((resolve, reject) => {
  const id = ++seq;
  const on = (e) => {
    const m = JSON.parse(e.data);
    if (m.id !== id) return;
    ws.removeEventListener('message', on);
    m.error ? reject(new Error(m.error.message)) : resolve(m.result);
  };
  ws.addEventListener('message', on);
  ws.send(JSON.stringify({ id, method, params }));
});
const js = async (expression) => (await call('Runtime.evaluate', { expression: `(async () => { ${expression} })()`, awaitPromise: true, returnByValue: true })).result.value;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const goto = async (query, wait = 2600) => {
  await call('Page.navigate', { url: `${base}?mock${query}` });
  await sleep(wait);
};
const shot = async (name) => {
  const r = await call('Page.captureScreenshot', { format: 'png' });
  writeFileSync(join(out, `${name}.png`), Buffer.from(r.data, 'base64'));
  console.log('saved', name);
};
const click = (selector, text = '') => js(`
  const el = [...document.querySelectorAll(${JSON.stringify(selector)})].find((e) => e.textContent.includes(${JSON.stringify(text)}));
  if (!el) throw new Error('no element ' + ${JSON.stringify(selector + ' ' + text)});
  el.click();`);
const key = (k) => js(`window.dispatchEvent(new KeyboardEvent('keydown', { key: ${JSON.stringify(k)}, bubbles: true }));`);

// 1. the main window, idle, a far server selected so the map has something to say
await goto('');
await click('button.row:not(:first-child)', 'США (для ИИ)');
await sleep(500);
await shot('main');

// 2. the connect animation, frame by frame (assembled into an animation by ffmpeg afterwards)
await goto('&delay=1900');
await click('button.row:not(:first-child)', 'США (для ИИ)');
await sleep(600);
const frames = [];
const onFrame = (e) => {
  const m = JSON.parse(e.data);
  if (m.method !== 'Page.screencastFrame') return;
  frames.push({ t: Date.now(), data: m.params.data });
  ws.send(JSON.stringify({ id: ++seq, method: 'Page.screencastFrameAck', params: { sessionId: m.params.sessionId } }));
};
ws.addEventListener('message', onFrame);
await call('Page.startScreencast', { format: 'jpeg', quality: 92, everyNthFrame: 1 });
await sleep(700);
await click('.connect');
await sleep(5600);
await call('Page.stopScreencast');
ws.removeEventListener('message', onFrame);
// ffmpeg's concat format: each frame with how long it stayed on screen
const lines = ['ffconcat version 1.0'];
frames.forEach((f, i) => {
  const name = `f_${String(i).padStart(4, '0')}.jpg`;
  writeFileSync(join(out, 'frames', name), Buffer.from(f.data, 'base64'));
  const next = frames[i + 1]?.t ?? f.t + 600;
  lines.push(`file ${name}`, `duration ${((next - f.t) / 1000).toFixed(3)}`);
});
writeFileSync(join(out, 'frames', 'list.txt'), lines.join(String.fromCharCode(10)));
console.log('frames', frames.length, 'over', frames.at(-1).t - frames[0].t, 'ms');

// 3. connected, with some traffic on the chart
await sleep(5000);
await shot('connected');

// 4. the panels
await click('nav button[title="Настройки"]');
await sleep(700);
await shot('settings');
await click('nav button[title="Маршрутизация"]');
await sleep(700);
await shot('routes');
await key('Escape');

// 5. another accent and the light theme
await click('nav button[title="Настройки"]');
await sleep(500);
await click('.swatch[title="#4da3ff"]');
await key('Escape');
await sleep(900);
await shot('accent');
await click('nav button[title="Настройки"]');
await sleep(400);
await click('.seg button', 'Светлая');
await click('.swatch[title="#3ddc84"]');
await key('Escape');
await sleep(900);
await shot('light');

// leave the test copy as it was: dark and green
await click('nav button[title="Настройки"]');
await sleep(300);
await click('.seg button', 'Тёмная');
await key('Escape');
await call('Page.navigate', { url: base });
ws.close();
