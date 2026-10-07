<script>
  import { icons } from './icons.js';
  let { name, size = 16 } = $props();
</script>

<svg width={size} height={size} viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5"
     stroke-linecap="square" stroke-linejoin="miter" aria-hidden="true">
  <path d={icons[name] ?? ''} />
</svg>

<style>
  svg {
    display: block;
    flex: none;
  }
</style>
