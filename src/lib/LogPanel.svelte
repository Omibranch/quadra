<script>
  import { tick } from 'svelte';
  import { app, logLines, copy } from './store.svelte.js';
  import { invoke } from './api.js';
  import Panel from './Panel.svelte';
  import Segmented from './Segmented.svelte';
  import Icon from './Icon.svelte';

  const levels = [{ value: 'all', label: 'Всё' }, { value: 'warn', label: 'Предупреждения' }, { value: 'error', label: 'Ошибки' }];
  let level = $state('all');
  let box = $state();
  let pinned = true;

  const kind = (l) => (/\[error\]|failed|panic/i.test(l) ? 'error' : /\[warning\]/i.test(l) ? 'warn' : l.startsWith('---') ? 'mark' : '');

  const lines = $derived.by(() => {
    app.logTick;
    const all = logLines.slice(-700).map((text) => ({ text, kind: kind(text) }));
    if (level === 'all') return all;
    return all.filter((l) => l.kind === 'error' || l.kind === 'mark' || (level === 'warn' && l.kind === 'warn'));
  });

  $effect(() => {
    lines;
    if (pinned) tick().then(() => box && (box.scrollTop = box.scrollHeight));
  });

  async function clear() {
    await invoke('clear_logs');
    logLines.length = 0;
    app.logTick += 1;
  }
</script>

<Panel title="Журнал">
  {#snippet actions()}
    <Segmented small options={levels} value={level} onchange={(v) => (level = v)} />
    <button class="ibtn" title="Скопировать" onclick={() => copy(lines.map((l) => l.text).join('\n'), 'Журнал скопирован')}><Icon name="copy" /></button>
    <button class="ibtn" title="Очистить" onclick={clear}><Icon name="trash" /></button>
  {/snippet}

  <div class="log code" bind:this={box} onscroll={() => (pinned = box.scrollHeight - box.scrollTop - box.clientHeight < 40)}>
    {#each lines as l}
      <div class={l.kind}>{l.text}</div>
    {:else}
      <p>Журнал пуст. Сюда пишет ядро во время подключения и работы.</p>
    {/each}
  </div>
</Panel>

<style>
  .log {
    position: absolute;
    inset: 56px 0 0 0;
    overflow: auto;
    padding: 14px 28px 24px;
    font-size: 11.5px;
    line-height: 1.6;
    color: var(--text-2);
    user-select: text;
    white-space: pre-wrap;
    word-break: break-word;
  }
  .warn {
    color: var(--warn);
  }
  .error {
    color: var(--danger);
  }
  .mark {
    color: var(--accent-dim);
    margin-top: 8px;
  }
  p {
    font-family: var(--font);
    color: var(--text-3);
  }
</style>
