<script>
  // Toasts and the context menu: the two things that float above everything else.
  import { app } from './store.svelte.js';
  import Icon from './Icon.svelte';

  let menuEl = $state();
  const closeMenu = () => (app.menu = null);

  function place(node) {
    const r = node.getBoundingClientRect();
    const x = Math.min(app.menu.x, innerWidth - r.width - 8);
    const y = Math.min(app.menu.y, innerHeight - r.height - 8);
    node.style.left = `${Math.max(8, x)}px`;
    node.style.top = `${Math.max(8, y)}px`;
  }

  function run(item) {
    closeMenu();
    item.action();
  }
</script>

<svelte:window onblur={closeMenu} onresize={closeMenu} />

{#if app.menu}
  <div class="catch" onpointerdown={closeMenu} oncontextmenu={(e) => { e.preventDefault(); closeMenu(); }} role="presentation"></div>
  <div class="menu" bind:this={menuEl} use:place role="menu">
    {#each app.menu.items as item}
      <button role="menuitem" class:danger={item.danger} onclick={() => run(item)}>
        <Icon name={item.icon} size={14} /><span>{item.label}</span>
      </button>
    {/each}
  </div>
{/if}

<div class="toasts" aria-live="polite">
  {#each app.toasts as t (t.id)}
    <div class="toast {t.kind}"><span>{t.text}</span></div>
  {/each}
</div>

<style>
  .catch {
    position: fixed;
    inset: 0;
    z-index: 60;
  }
  .menu {
    position: fixed;
    z-index: 61;
    min-width: 200px;
    padding: 4px;
    display: flex;
    flex-direction: column;
    background: var(--surface-2);
    border: 1px solid var(--line-2);
    border-radius: var(--r-lg);
    box-shadow: var(--shadow);
    transform-origin: top left;
    animation: pop 120ms var(--ease);
  }
  @keyframes pop {
    from {
      opacity: 0;
      transform: scale(0.94);
    }
  }
  .menu button {
    height: 30px;
    padding: 0 12px 0 9px;
    display: flex;
    align-items: center;
    gap: 10px;
    border-radius: 2px;
    text-align: left;
    color: var(--text);
    transition: background-color var(--t-fast);
  }
  .menu button :global(svg) {
    color: var(--text-3);
  }
  .menu button:hover {
    background: var(--surface-3);
  }
  .menu .danger,
  .menu .danger :global(svg) {
    color: var(--danger);
  }
  .menu .danger:hover {
    background: var(--danger-soft);
  }
  .toasts {
    position: fixed;
    left: calc(var(--sidebar) + 20px);
    bottom: 108px;
    z-index: 70;
    display: flex;
    flex-direction: column;
    gap: 6px;
    pointer-events: none;
  }
  .toast {
    display: flex;
    align-items: center;
    gap: 11px;
    max-width: 460px;
    padding: 9px 14px;
    background: var(--surface-2);
    border: 1px solid var(--line-2);
    border-radius: var(--r);
    box-shadow: var(--shadow);
    animation: rise var(--t-slow) var(--ease);
  }
  .toast.ok {
    border-left: 2px solid var(--accent);
  }
  .toast.error {
    border-color: color-mix(in srgb, var(--danger) 45%, transparent);
    border-left: 2px solid var(--danger);
  }
  .toast span {
    font-size: 12.5px;
    line-height: 1.4;
  }
</style>
