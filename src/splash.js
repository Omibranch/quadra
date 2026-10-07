// The startup cube, shown in its own transparent window while the main one loads. A Blender
// render (tools/intro_cube.py) packed into a sprite sheet: 96 frames, one full turn in four eased
// quarter turns, each ending in a short rest. The frames are grey and get tinted with the accent
// colour here, so the cube follows the theme.
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import sheetUrl from './assets/intro.webp';

const COLS = 12;
const FRAME = 240;
const FRAMES = 96;
const FPS = 24;
const QUARTER = 24; // frames per quarter turn; the cube stands still for the last four
const MIN_MS = 1000;

const canvas = document.querySelector('canvas');
const ctx = canvas.getContext('2d');
const stored = (key, fallback) => {
  try {
    return localStorage.getItem(key) ?? fallback;
  } catch {
    return fallback;
  }
};
const accent = stored('quadra.accent', '#3ddc84');
const lite = stored('quadra.effects', 'full') === 'lite';

let sheet = null;
let startedAt = 0;
let asked = false; // the main window is ready
let leaving = false;

function draw(index) {
  const sx = (index % COLS) * FRAME;
  const sy = Math.floor(index / COLS) * FRAME;
  ctx.globalCompositeOperation = 'copy';
  ctx.drawImage(sheet, sx, sy, FRAME, FRAME, 0, 0, FRAME, FRAME);
  ctx.globalCompositeOperation = 'multiply';
  ctx.fillStyle = accent;
  ctx.fillRect(0, 0, FRAME, FRAME);
  ctx.globalCompositeOperation = 'destination-in';
  ctx.drawImage(sheet, sx, sy, FRAME, FRAME, 0, 0, FRAME, FRAME);
}

function tick(now) {
  if (leaving) return;
  if (!lite) draw(Math.floor(((now - startedAt) / 1000) * FPS) % FRAMES);
  requestAnimationFrame(tick);
}

/** How long until the cube next stands still, no sooner than the minimum showing time. */
function untilRest() {
  const elapsed = performance.now() - startedAt;
  if (lite) return Math.max(0, 400 - elapsed);
  const quarterMs = (QUARTER / FPS) * 1000;
  const restMs = quarterMs - (4 / FPS) * 1000;
  const at = Math.max(elapsed, MIN_MS);
  const phase = at % quarterMs;
  return (phase < 1000 / FPS || phase >= restMs ? at : at - phase + restMs) - elapsed;
}

function leave() {
  if (leaving) return;
  leaving = true;
  // the main window comes up underneath while the cube grows and fades over it
  invoke('splash', { stage: 'leaving' }).catch(() => {});
  canvas.classList.add('out');
  setTimeout(() => invoke('splash', { stage: 'done' }).catch(() => {}), 340);
}

function finish() {
  if (asked) return;
  asked = true;
  if (startedAt) setTimeout(leave, untilRest());
}

const image = new Image();
image.onload = async () => {
  sheet = (await createImageBitmap(image).catch(() => null)) ?? image;
  draw(0);
  canvas.classList.add('on');
  // The window is still hidden; it is shown now that there is a cube to look at. The answer
  // says whether the main window got ready in the meantime.
  const mainReady = await invoke('splash', { stage: 'loaded' }).catch(() => true);
  startedAt = performance.now();
  requestAnimationFrame(tick);
  if (asked || mainReady) {
    asked = true;
    setTimeout(leave, untilRest());
  }
};
image.onerror = () => invoke('splash', { stage: 'done' }).catch(() => {});
image.src = sheetUrl;

listen('finish', finish);
