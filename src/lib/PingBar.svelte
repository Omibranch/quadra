<script>
  import { pingLevel } from './format.js';
  let { ms = null, pending = false } = $props();
  const level = $derived(pingLevel(ms));
</script>

<span class="ping" class:bad={level === -1} class:pending={pending && ms == null}>
  <span class="num mono">{level === -1 ? 'нет' : ms == null ? '' : ms}</span>
  <span class="cells" aria-hidden="true">
    {#each [1, 2, 3, 4] as n}<i class:on={level >= n}></i>{/each}
  </span>
</span>

<style>
  .ping {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    flex: none;
  }
  .num {
    min-width: 26px;
    text-align: right;
    font-size: 11px;
    color: var(--text-2);
  }
  .cells {
    display: grid;
    grid-template-columns: repeat(4, 4px);
    gap: 1px;
  }
  .cells i {
    width: 4px;
    height: 4px;
    background: var(--line-2);
    transition: background-color var(--t);
  }
  .cells i.on {
    background: var(--accent);
  }
  .bad .num {
    color: var(--danger);
  }
  .bad .cells i {
    background: color-mix(in srgb, var(--danger) 45%, transparent);
  }
  .pending .cells i {
    animation: blink 0.9s steps(2) infinite;
  }
  .pending .cells i:nth-child(2) {
    animation-delay: 0.12s;
  }
  .pending .cells i:nth-child(3) {
    animation-delay: 0.24s;
  }
  .pending .cells i:nth-child(4) {
    animation-delay: 0.36s;
  }
</style>
