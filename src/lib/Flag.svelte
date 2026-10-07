<script>
  import { flagRows, palette } from './flags.js';
  let { cc = '', cell = 3 } = $props();
  const rows = $derived(flagRows(cc));
  const gap = 1;
  const size = $derived(cell * 6 + gap * 5);
</script>

{#if rows}
  <svg class="flag" width={size} height={size} viewBox="0 0 {size} {size}" aria-hidden="true" shape-rendering="crispEdges">
    {#each rows as row, r}
      {#each row as ch, c}
        <rect x={c * (cell + gap)} y={r * (cell + gap)} width={cell} height={cell} style="fill:{palette[ch]}" />
      {/each}
    {/each}
  </svg>
{:else}
  <span class="code mono" style="width:{size}px;height:{size}px">{(cc || '··').toUpperCase().slice(0, 2)}</span>
{/if}

<style>
  .flag {
    display: block;
    flex: none;
  }
  .code {
    flex: none;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    font-size: 10px;
    font-weight: 600;
    color: var(--text-2);
    border: 1px solid var(--line-2);
    border-radius: 2px;
  }
</style>
