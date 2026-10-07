// The bridge to the Rust side. Opened in a plain browser (no Tauri) it falls back to a stand-in
// backend with sample data, which is how the interface is previewed and checked. `?mock` in the
// address forces the stand-in inside the app too (that is how the README screenshots are made).
const forced = typeof location !== 'undefined' && new URLSearchParams(location.search).has('mock');
const tauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window && !forced;

let invokeImpl;
let listenImpl;
let windowImpl;

if (tauri) {
  const core = await import('@tauri-apps/api/core');
  const event = await import('@tauri-apps/api/event');
  const win = await import('@tauri-apps/api/window');
  invokeImpl = core.invoke;
  listenImpl = (name, fn) => event.listen(name, (e) => fn(e.payload));
  windowImpl = win.getCurrentWindow();
} else {
  const mock = await import('./mock.js');
  invokeImpl = mock.invoke;
  listenImpl = mock.listen;
  windowImpl = mock.appWindow;
}

export const isTauri = tauri;
export const invoke = (cmd, args) => invokeImpl(cmd, args);
export const listen = (name, fn) => listenImpl(name, fn);
export const appWindow = windowImpl;
