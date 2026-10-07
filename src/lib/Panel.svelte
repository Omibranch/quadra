<script>
  import { app } from './store.svelte.js';
  import Icon from './Icon.svelte';
  let { title, children, actions } = $props();
</script>

<section class="panel">
  <header>
    <h2>{title}</h2>
    <div class="actions">{@render actions?.()}</div>
    <button class="ibtn" title="Закрыть (Esc)" onclick={() => (app.panel = null)}><Icon name="close" /></button>
  </header>
  <div class="body">{@render children()}</div>
</section>

<style>
  .panel {
    position: absolute;
    inset: 0;
    z-index: 5;
    display: flex;
    flex-direction: column;
    background: var(--bg);
    animation: slide var(--t-slow) var(--ease);
  }
  @keyframes slide {
    from {
      opacity: 0;
      transform: translateX(28px);
    }
  }
  header {
    flex: none;
    height: 56px;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 0 14px 0 28px;
    border-bottom: 1px solid var(--line);
  }
  h2 {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
    flex: 1;
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 8px 28px 40px;
  }
</style>
