<script>
  import { onMount } from 'svelte';
  import { app, init, importText } from './lib/store.svelte.js';
  import { invoke } from './lib/api.js';
  import Titlebar from './lib/Titlebar.svelte';
  import Sidebar from './lib/Sidebar.svelte';
  import MapView from './lib/MapView.svelte';
  import Hud from './lib/Hud.svelte';
  import SettingsPanel from './lib/SettingsPanel.svelte';
  import RoutesPanel from './lib/RoutesPanel.svelte';
  import LogPanel from './lib/LogPanel.svelte';
  import Popups from './lib/Popups.svelte';
  import Overlays from './lib/Overlays.svelte';
  import Loader from './lib/Loader.svelte';

  let dragging = $state(false);
  let dragDepth = 0;

  const typing = (e) => e.target instanceof HTMLElement && e.target.closest('input, textarea');

  $effect(() => {
    if (!app.settings) return;
    const root = document.documentElement;
    root.dataset.theme = app.settings.theme;
    root.dataset.effects = app.settings.effects;
    root.dataset.platform = app.platform;
    root.style.setProperty('--accent', app.settings.accent);
    try {
      // remembered for the startup screen, which is drawn before the settings arrive
      localStorage.setItem('quadra.theme', app.settings.theme);
      localStorage.setItem('quadra.accent', app.settings.accent);
      localStorage.setItem('quadra.effects', app.settings.effects);
    } catch {}
  });

  onMount(async () => {
    await init();
    // Tell the backend: it lets the startup cube finish and shows this window. A timer, not a
    // frame callback: the window is still hidden, and WebKit runs no frames for a hidden window.
    setTimeout(() => invoke('ready'), 0);
  });

  function onKey(e) {
    if (e.key === 'Escape') {
      if (app.menu) app.menu = null;
      else if (app.popup) app.popup = null;
      else if (app.pickHome) app.pickHome = false;
      else if (app.panel) app.panel = null;
    } else if (e.key === 'F5' || (e.ctrlKey && (e.key === 'r' || e.key === 'R'))) {
      e.preventDefault(); // a reload would drop the interface state for nothing
    }
  }

  // Ctrl+V anywhere outside a text field adds whatever is on the clipboard.
  function onPaste(e) {
    if (typing(e) || app.popup) return;
    const text = e.clipboardData?.getData('text')?.trim();
    if (text) {
      e.preventDefault();
      importText(text);
    }
  }

  function onDragEnter(e) {
    if (!e.dataTransfer?.types.some((t) => t === 'Files' || t === 'text/plain')) return;
    dragDepth += 1;
    dragging = true;
  }

  function onDragLeave() {
    dragDepth = Math.max(0, dragDepth - 1);
    if (!dragDepth) dragging = false;
  }

  async function onDrop(e) {
    e.preventDefault();
    dragDepth = 0;
    dragging = false;
    const file = e.dataTransfer?.files?.[0];
    const text = file ? await file.text() : e.dataTransfer?.getData('text/plain');
    if (text?.trim()) importText(text.trim());
  }
</script>

<svelte:window onkeydown={onKey} onpaste={onPaste} ondragenter={onDragEnter} ondragleave={onDragLeave}
               ondragover={(e) => e.preventDefault()} ondrop={onDrop} oncontextmenu={(e) => !typing(e) && e.preventDefault()} />

{#if !app.tiling && app.platform !== 'android'}<Titlebar />{/if}

{#if app.ready}
  <main class:panel={!!app.panel}>
    <Sidebar />
    <div class="stage">
      <MapView paused={!!app.panel} />
      <Hud />
      {#if app.panel === 'settings'}
        <SettingsPanel />
      {:else if app.panel === 'routes'}
        <RoutesPanel />
      {:else if app.panel === 'log'}
        <LogPanel />
      {/if}
    </div>
  </main>
  <Popups />
  <Overlays />
  {#if dragging}
    <div class="drop"><div><Loader cell={6} still /><b>Отпусти, чтобы добавить</b></div></div>
  {/if}
{:else}
  <div class="boot"><Loader cell={7} /></div>
{/if}

<style>
  :global(#app) {
    display: flex;
    flex-direction: column;
  }
  main {
    flex: 1;
    min-height: 0;
    display: flex;
  }
  .stage {
    flex: 1;
    min-width: 0;
    position: relative;
    overflow: hidden;
    background: var(--bg);
  }
  .boot {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--accent);
  }
  .drop {
    position: fixed;
    inset: var(--titlebar) 0 0 0;
    z-index: 80;
    display: flex;
    align-items: center;
    justify-content: center;
    background: color-mix(in srgb, var(--bg) 80%, transparent);
    border: 1px solid var(--accent-line);
    pointer-events: none;
    animation: fade-in var(--t-fast);
  }
  .drop div {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 16px;
    color: var(--accent);
  }
  .drop b {
    font-size: 14px;
    font-weight: 600;
    color: var(--text);
  }
</style>
