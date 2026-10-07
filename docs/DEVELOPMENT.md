# Working on Quadra

## Layout

- `src/` is the interface. `lib/MapView.svelte` is the dot map and the connect animation,
  `lib/store.svelte.js` the app state, `app.css` every colour, size and speed as a variable.
  `splash.html` + `src/splash.js` are the startup cube's own window.
- `src-tauri/src/` is the backend: `subs.rs` (subscriptions and the device headers providers
  require), `links.rs` (share links to Xray outbounds), `config.rs` (the config Xray is started
  with), `core.rs` (start, confirm, watch, stop), `sys/` (what differs per operating system:
  system proxy, elevation, clipboard, machine id).
- `src-tauri/resources/xray/` holds Xray-core and its data files. It is not in git;
  `node tools/fetch-xray.mjs` fills it for the current machine (`--target <triple>` for another).
  The all-traffic mode needs the automatic routing of Xray 26.9 or newer.

## The interface in a browser

`npm run dev` serves the interface against a stand-in backend (`src/lib/mock.js`). Useful
additions to the address:

| Parameter | Effect |
|---|---|
| `?delay=4000` | the connection takes that long, to study the animation |
| `?fail` | the connection fails |
| `?boot=3000` | the app takes that long to start |
| `?platform=macos`, `?platform=linux&tiling` | how the window looks elsewhere |
| `?mock` | inside the real app: use the stand-in backend (how the README pictures are made) |

## Checking the real thing

- `tools/dev-run.ps1` builds a debug app with the webview's debugging port open and starts it.
  It uses its own identifier (`dev.quadra.client.test`), so it runs next to an installed copy
  and never touches its settings.
- `tools/cdp.mjs eval "<js>"` / `shot <file>` drives that app.
- `tools/e2e.mjs <ports|proxy|tun> "<server name>" [full|link]` connects, checks where traffic
  exits, and always disconnects again.
- `tools/window-trace.ps1` lists which windows are on screen during startup;
  `tools/splash-check.ps1` photographs the splash area. `QUADRA_SPLASH_HOLD=1` keeps the
  startup cube on screen in a debug build.
- `tools/shots.mjs <folder>` retakes the README screenshots and records the connect animation.

The core's output of the current and previous session is kept in `core.log` in the app's data
folder (`%APPDATA%\dev.quadra.client` on Windows).

## Generated assets

- `python tools/gen_map.py` rebuilds `src/assets/world.json` from Natural Earth outlines.
- `python tools/gen_icon.py && npx tauri icon tools/icon.png` rebuilds the app icon.
- The startup cube is a Blender render:
  `blender -b --factory-startup -P tools/intro_cube.py -- <dir>` writes 96 grey RGBA frames
  (one full turn in four eased quarter turns), `python tools/intro_sheet.py <dir>` packs them
  into `src/assets/intro.webp`. `src/splash.js` tints them with the accent colour. With
  `--still` the script renders one large picture, from which `python tools/brand.py <still.png>`
  makes the README logo and the installer art.

## Releases

Pushing a tag `vX.Y.Z` runs `.github/workflows/release.yml`: it builds the installers for
Windows, macOS (Apple silicon and Intel) and Linux and attaches them to a draft release.
Keep the version in `package.json`, `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json`
in step with the tag.
