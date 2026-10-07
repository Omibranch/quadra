// Drives the running app through WebView2's remote debugging port, for end-to-end checks.
//   node tools/cdp.mjs eval "<js expression, may be async>"
//   node tools/cdp.mjs shot <file.png>
const [, , cmd, arg] = process.argv;
const port = process.env.CDP_PORT ?? 9222;
const pages = await (await fetch(`http://127.0.0.1:${port}/json`)).json();
const page = pages.find((p) => p.type === 'page');
if (!page) throw new Error('no page');
const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((ok, bad) => { ws.onopen = ok; ws.onerror = bad; });
let id = 0;
const call = (method, params = {}) => new Promise((resolve, reject) => {
  const mine = ++id;
  const on = (e) => {
    const m = JSON.parse(e.data);
    if (m.id !== mine) return;
    ws.removeEventListener('message', on);
    m.error ? reject(new Error(m.error.message)) : resolve(m.result);
  };
  ws.addEventListener('message', on);
  ws.send(JSON.stringify({ id: mine, method, params }));
});
if (cmd === 'eval') {
  const r = await call('Runtime.evaluate', { expression: `(async () => (${arg}))()`, awaitPromise: true, returnByValue: true });
  if (r.exceptionDetails) console.log('EXCEPTION', JSON.stringify(r.exceptionDetails.exception?.value ?? r.exceptionDetails.exception?.description ?? r.exceptionDetails.text));
  else console.log(typeof r.result.value === 'string' ? r.result.value : JSON.stringify(r.result.value, null, 1));
} else if (cmd === 'shot') {
  const r = await call('Page.captureScreenshot', { format: 'png' });
  (await import('node:fs')).writeFileSync(arg, Buffer.from(r.data, 'base64'));
  console.log('saved', arg);
}
ws.close();
