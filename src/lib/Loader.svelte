<script>
  // The app's only spinner: the 3x3 block with a light running round its edge.
  let { cell = 4, still = false } = $props();
  const order = [0, 1, 2, 5, 8, 7, 6, 3];
</script>

<i class="loader" class:still style="--cell:{cell}px" aria-hidden="true">
  {#each Array(9) as _, i}
    <b class:c={i === 4} style="animation-delay:{order.indexOf(i) * 90}ms"></b>
  {/each}
</i>

<style>
  .loader {
    display: grid;
    grid-template-columns: repeat(3, var(--cell));
    gap: 1px;
    flex: none;
  }
  b {
    width: var(--cell);
    height: var(--cell);
    background: currentColor;
    opacity: 0.25;
    animation: run 720ms steps(1) infinite;
  }
  b.c {
    opacity: 1;
    animation: none;
  }
  .still b {
    animation: none;
    opacity: 0.45;
  }
  .still b.c {
    opacity: 1;
  }
  @keyframes run {
    0% {
      opacity: 1;
    }
    25% {
      opacity: 0.6;
    }
    50%,
    100% {
      opacity: 0.25;
    }
  }
</style>
