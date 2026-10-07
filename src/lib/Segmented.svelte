<script>
  import Icon from './Icon.svelte';
  // options: [{ value, label, icon?, title? }]
  let { options, value, onchange, small = false } = $props();
</script>

<div class="seg" class:small role="radiogroup">
  {#each options as o}
    <button role="radio" aria-checked={o.value === value} class:on={o.value === value} title={o.title} onclick={() => o.value !== value && onchange?.(o.value)}>
      {#if o.icon}<Icon name={o.icon} size={14} />{/if}
      {#if o.label}<span>{o.label}</span>{/if}
    </button>
  {/each}
</div>

<style>
  .seg {
    display: inline-flex;
    padding: 2px;
    gap: 2px;
    background: var(--surface-2);
    border: 1px solid var(--line);
    border-radius: var(--r);
    flex: none;
  }
  button {
    height: 26px;
    padding: 0 11px;
    display: flex;
    align-items: center;
    gap: 7px;
    border-radius: 2px;
    color: var(--text-2);
    font-size: 12px;
    font-weight: 500;
    white-space: nowrap;
    transition: background-color var(--t-fast), color var(--t-fast);
  }
  .small button {
    height: 24px;
    padding: 0 9px;
    font-size: 11.5px;
  }
  button:hover {
    color: var(--text);
    background: var(--surface-3);
  }
  button.on {
    background: var(--accent-soft-2);
    color: var(--accent-dim);
  }
  :global([data-theme='dark']) button.on {
    color: var(--accent);
  }
</style>
