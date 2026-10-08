// End-to-end check of one connection mode against the running debug app (see dev-run.ps1).
//   node tools/e2e.mjs <ports|proxy|tun> "<server name>" [full]
// Connects, looks at where traffic really goes, and always disconnects again, whatever happens:
// the whole run is local, so it finishes and cleans up even if the network drops meanwhile.
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';

const [, , mode = 'ports', name = '', kind = ''] = process.argv;
const pages = await (await fetch('http://127.0.0.1:9222/json')).json();
const ws = new WebSocket(pages.find((p) => p.type === 'page').webSocketDebuggerUrl);
await new Promise((ok, bad) => { ws.onopen = ok; ws.onerror = bad; });
let seq = 0;
const evaluate = (expression) => new Promise((resolve, reject) => {
  const id = ++seq;
  const on = (e) => {
    const m = JSON.parse(e.data);
    if (m.id !== id) return;
    ws.removeEventListener('message', on);
    if (m.error) reject(new Error(m.error.message));
    else if (m.result.exceptionDetails) reject(new Error(JSON.stringify(m.result.exceptionDetails.exception?.value ?? m.result.exceptionDetails.text)));
    else resolve(m.result.result.value);
  };
  ws.addEventListener('message', on);
  ws.send(JSON.stringify({ id, method: 'Runtime.evaluate', params: { expression: `(async () => (${expression}))()`, awaitPromise: true, returnByValue: true } }));
});
const inv = (cmd, args = {}) => evaluate(`window.__TAURI_INTERNALS__.invoke(${JSON.stringify(cmd)}, ${JSON.stringify(args)})`);
const run = (file, args, timeout = 20000) => {
  try {
    return execFileSync(file, args, { encoding: 'utf8', timeout, stdio: ['ignore', 'pipe', 'pipe'] }).trim();
  } catch (e) {
    return `FAILED: ${(e.stderr || e.message || '').toString().trim().split('\n').pop()}`;
  }
};
const where = (extra) => {
  const out = run('curl', ['-sS', '-m', '10', ...extra, 'https://ipwho.is/']);
  try {
    const j = JSON.parse(out);
    return `${j.ip} ${j.country_code} ${j.city ?? ''}`;
  } catch {
    return out.slice(0, 160);
  }
};
const proxyKey = 'HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings';
const reg = (value) => (run('reg', ['query', proxyKey, '/v', value]).split(/\s{2,}/).pop() ?? '').trim();
const ps = (script) => run('powershell', ['-NoProfile', '-Command', script], 25000);

const state = await inv('get_state');
const original = state.settings.mode;
const server = state.servers.find((s) => s.name === name && (kind !== 'full' || s.full) && (kind !== 'link' || !s.full));
if (!server) throw new Error(`no server named "${name}"`);
console.log(`server: ${server.name} (${server.full ? 'provider config' : 'link'}), mode: ${mode}, admin: ${state.admin}`);
console.log('before   :', where([]), '| system proxy', reg('ProxyEnable'));

try {
  await inv('save_settings', { settings: { ...state.settings, mode } });
  const t = Date.now();
  let result = 'ok';
  try {
    await inv('connect', { id: server.id });
  } catch (e) {
    result = `refused: ${e.message}`;
  }
  const status = (await inv('get_state')).status;
  console.log(`connect  : ${result} in ${Date.now() - t} ms, state ${status.state}, probe ${status.ms} ms${status.error ? ', error ' + status.error : ''}`);
  if (status.state === 'on') {
    console.log('via socks:', where(['--socks5-hostname', `127.0.0.1:${state.settings.socks_port}`]));
    console.log('via http :', where(['-x', `http://127.0.0.1:${state.settings.http_port}`]));
    if (mode === 'proxy') {
      console.log('registry : ProxyEnable', reg('ProxyEnable'), '| ProxyServer', reg('ProxyServer'));
      console.log('override :', reg('ProxyOverride'));
      console.log('system   :', ps("$r = Invoke-RestMethod -TimeoutSec 12 https://ipwho.is/; \"$($r.ip) $($r.country_code) $($r.city)\""));
    }
    if (mode === 'tun') {
      console.log('adapters :', ps("(Get-NetAdapter | Where-Object Status -eq 'Up' | ForEach-Object { $_.Name }) -join ', '"));
      console.log('routes   :', ps("(Get-NetRoute -DestinationPrefix '0.0.0.0/0' | ForEach-Object { \"$($_.InterfaceAlias) metric $($_.RouteMetric)\" }) -join '; '"));
      console.log('plain    :', where([]));
    }
    await new Promise((r) => setTimeout(r, 1500));
    console.log('status   :', JSON.stringify((await inv('get_state')).status.exit));
  }
} finally {
  await inv('disconnect').catch(() => {});
  await inv('save_settings', { settings: { ...state.settings, mode: original } }).catch(() => {});
  await evaluate('location.reload()').catch(() => {});
  console.log('after    :', where([]), '| system proxy', reg('ProxyEnable'), '| override', reg('ProxyOverride').slice(0, 40) || '(none)');
  try {
    const log = readFileSync(`${process.env.APPDATA}\\dev.quadra.client\\core.log`, 'utf8').trim().split('\n');
    const from = log.map((l) => l.startsWith('---')).lastIndexOf(true);
    console.log('core log :\n  ' + log.slice(from).slice(0, 14).map((l) => l.slice(0, 170)).join('\n  '));
  } catch {}
  ws.close();
}
