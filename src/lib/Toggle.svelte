<script>
  let { checked = false, onchange, disabled = false } = $props();
</script>

<button class="toggle" class:on={checked} role="switch" aria-checked={checked} aria-label="переключатель" {disabled} onclick={() => onchange?.(!checked)}>
  <i></i>
</button>

<style>
  .toggle {
    width: 38px;
    height: 20px;
    flex: none;
    padding: 3px;
    border: 1px solid var(--line-2);
    border-radius: var(--r);
    background: var(--surface-2);
    transition: background-color var(--t), border-color var(--t);
  }
  .toggle i {
    display: block;
    width: 12px;
    height: 12px;
    background: var(--text-3);
    transition: transform var(--t) var(--ease), background-color var(--t);
  }
  .toggle.on {
    background: var(--accent-soft-2);
    border-color: var(--accent-line);
  }
  .toggle.on i {
    transform: translateX(18px);
    background: var(--accent);
  }
  .toggle[disabled] {
    opacity: 0.45;
    pointer-events: none;
  }
</style>
