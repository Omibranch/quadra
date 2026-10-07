<script>
  import { onMount } from 'svelte';
  import { appWindow } from './api.js';
  import { app } from './store.svelte.js';
  import Icon from './Icon.svelte';

  let maximized = $state(false);

  onMount(() => {
    let off;
    const sync = async () => (maximized = await appWindow.isMaximized());
    sync();
    appWindow.onResized(sync).then((fn) => (off = fn));
    return () => off?.();
  });
</script>

<header data-tauri-drag-region>
  <div class="brand" data-tauri-drag-region>
    <i class="logo" aria-hidden="true">
      {#each Array(9) as _, i}<b class:c={i === 4} class:e={i % 2 === 1}></b>{/each}
    </i>
    <span data-tauri-drag-region>Quadra</span>
  </div>
  {#if app.platform !== 'macos'}
  <div class="controls">
    <button title="Свернуть" onclick={() => appWindow.minimize()}><Icon name="minus" /></button>
    <button title={maximized ? 'Восстановить' : 'Развернуть'} onclick={() => appWindow.toggleMaximize()}>
      <Icon name={maximized ? 'restore' : 'max'} />
    </button>
    <button class="close" title="Закрыть" onclick={() => appWindow.close()}><Icon name="close" /></button>
  </div>
  {/if}
</header>

<style>
  header {
    height: var(--titlebar);
    flex: none;
    display: flex;
    align-items: center;
    justify-content: space-between;
    background: var(--surface);
    border-bottom: 1px solid var(--line);
  }
  /* macOS draws its own window buttons on the left of the bar */
  :global([data-platform='macos']) .brand {
    justify-content: center;
    padding-left: 0;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
    padding-left: 14px;
    height: 100%;
    flex: 1;
    font: 500 13px/1 var(--mono);
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--text-2);
  }
  .logo {
    display: grid;
    grid-template-columns: repeat(3, 4px);
    gap: 1px;
    pointer-events: none;
  }
  .logo b {
    width: 4px;
    height: 4px;
    background: color-mix(in srgb, var(--accent) 30%, transparent);
  }
  .logo b.e {
    background: color-mix(in srgb, var(--accent) 58%, transparent);
  }
  .logo b.c {
    background: var(--accent);
  }
  .controls {
    display: flex;
    height: 100%;
  }
  .controls button {
    width: 46px;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-2);
    transition: background-color var(--t-fast), color var(--t-fast);
  }
  .controls button:hover {
    background: var(--surface-3);
    color: var(--text);
  }
  .controls .close:hover {
    background: var(--danger);
    color: #fff;
  }
</style>
