<script>
  // What sits on top of the map: where the traffic goes, the mode switch, live speed.
  import { app, set, connect, target, serverById } from './store.svelte.js';
  import { countryName } from './flags.js';
  import { speed, bytes } from './format.js';
  import Flag from './Flag.svelte';
  import Icon from './Icon.svelte';
  import Segmented from './Segmented.svelte';

  const modes = [
    { value: 'proxy', label: 'Прокси', icon: 'proxy', title: 'Системный прокси: браузеры и программы, которые его учитывают' },
    { value: 'tun', label: 'Весь трафик', icon: 'tun', title: 'Виртуальный адаптер: весь трафик системы, нужны права администратора' },
    { value: 'ports', label: 'Порты', icon: 'ports', title: 'Только локальные порты SOCKS и HTTP, систему не трогаем' },
  ];

  const on = $derived(app.status.state === 'on');
  const dest = $derived(app.status.state === 'off' ? target() : (serverById(app.status.server) ?? target()));
  const exitCc = $derived(app.status.exit?.cc || dest?.cc || '');
  const peak = $derived(Math.max(1, ...app.history.map((h) => h.down + h.up)));
  const bars = $derived([...Array(Math.max(0, 48 - app.history.length)).fill(0), ...app.history.map((h) => (h.down + h.up) / peak)]);

  function setMode(mode) {
    set('mode', mode);
    if (app.status.state !== 'off') connect();
  }
</script>

<div class="hud top">
  <div class="route" class:live={on}>
    {#if app.settings.home}
      <span class="end"><Flag cc={app.settings.home.cc} cell={2} /><b>{countryName(app.settings.home.cc) || 'Ты'}</b></span>
    {:else}
      <span class="end"><b>Ты</b></span>
    {/if}
    <span class="dots" aria-hidden="true">{#each Array(5) as _, i}<i style="animation-delay:{i * 110}ms"></i>{/each}</span>
    {#if dest}
      <span class="end"><Flag cc={on ? exitCc : dest.cc} cell={2} /><b>{countryName(on ? exitCc : dest.cc) || dest.name}</b></span>
    {:else}
      <span class="end muted"><b>нет сервера</b></span>
    {/if}
  </div>
  <Segmented options={modes} value={app.settings.mode} onchange={setMode} />
</div>

<div class="hud bottom" class:live={on}>
  <div class="stat">
    <span class="label"><Icon name="down" size={12} />Загрузка</span>
    <b class="mono">{on ? speed(app.traffic.down) : '—'}</b>
  </div>
  <div class="stat">
    <span class="label"><Icon name="up" size={12} />Отдача</span>
    <b class="mono">{on ? speed(app.traffic.up) : '—'}</b>
  </div>
  <div class="chart" aria-hidden="true">
    {#each bars as h}<i style="height:{Math.max(2, Math.round(h * 34))}px"></i>{/each}
  </div>
  <div class="stat right">
    <span class="label">За сессию</span>
    <b class="mono">{on ? bytes(app.traffic.tdown + app.traffic.tup) : '—'}</b>
  </div>
</div>

<style>
  .hud {
    position: absolute;
    left: 20px;
    right: 20px;
    display: flex;
    align-items: center;
    pointer-events: none;
  }
  .hud > :global(*) {
    pointer-events: auto;
  }
  .top {
    top: 16px;
    justify-content: space-between;
    gap: 16px;
  }
  .route {
    display: flex;
    align-items: center;
    gap: 12px;
    height: 32px;
    padding: 0 12px;
    background: color-mix(in srgb, var(--surface) 82%, transparent);
    border: 1px solid var(--line);
    border-radius: var(--r);
    transition: border-color var(--t);
  }
  .route.live {
    border-color: var(--accent-line);
  }
  .end {
    display: flex;
    align-items: center;
    gap: 8px;
    white-space: nowrap;
  }
  .end b {
    font-weight: 500;
    font-size: 12px;
  }
  .muted {
    color: var(--text-3);
  }
  .dots {
    display: flex;
    gap: 3px;
  }
  .dots i {
    width: 3px;
    height: 3px;
    background: var(--line-2);
  }
  .live .dots i {
    background: var(--accent);
    animation: blink 1.1s steps(2) infinite;
  }
  .bottom {
    bottom: 18px;
    gap: 26px;
    padding: 12px 16px;
    background: color-mix(in srgb, var(--surface) 82%, transparent);
    border: 1px solid var(--line);
    border-radius: var(--r);
  }
  .stat {
    display: flex;
    flex-direction: column;
    gap: 7px;
    min-width: 104px;
  }
  .stat.right {
    text-align: right;
    align-items: flex-end;
    min-width: 88px;
  }
  .stat .label {
    display: flex;
    align-items: center;
    gap: 5px;
  }
  .stat b {
    font-size: 15px;
    font-weight: 600;
    color: var(--text-3);
    transition: color var(--t);
  }
  .live .stat b {
    color: var(--text);
  }
  .chart {
    flex: 1;
    min-width: 0;
    height: 34px;
    display: flex;
    align-items: flex-end;
    justify-content: flex-end;
    gap: 2px;
    overflow: hidden;
  }
  .chart i {
    flex: 1 1 0;
    max-width: 7px;
    min-width: 2px;
    background: var(--line-2);
    transition: height 320ms var(--ease), background-color var(--t);
  }
  .live .chart i {
    background: var(--accent-dim);
  }
</style>
