<script>
  // One settings row: what it is on the left, the control on the right.
  let { title, hint = '', children, stack = false } = $props();
</script>

<div class="field" class:stack>
  <div class="text">
    <b>{title}</b>
    {#if hint}<span>{hint}</span>{/if}
  </div>
  <div class="control">{@render children()}</div>
</div>

<style>
  .field {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 24px;
    min-height: 52px;
    padding: 9px 0;
    border-bottom: 1px solid var(--line);
  }
  .field.stack {
    flex-direction: column;
    align-items: stretch;
    gap: 10px;
    padding: 12px 0 14px;
  }
  .text {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }
  b {
    font-weight: 500;
  }
  span {
    font-size: 11.5px;
    line-height: 1.45;
    color: var(--text-3);
    max-width: 440px;
  }
  .control {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: none;
  }
  .stack .control {
    flex: 1;
  }
</style>
