<script>
  import { app, toggle, target, pingAll, serverById } from './store.svelte.js';
  import { countryName } from './flags.js';
  import { duration, protoLabel } from './format.js';
  import Flag from './Flag.svelte';
  import Icon from './Icon.svelte';
  import Loader from './Loader.svelte';
  import ServerList from './ServerList.svelte';

  const phase = $derived(app.status.state);
  const shown = $derived(phase === 'off' ? target() : (serverById(app.status.server) ?? target()));
  const label = $derived({ off: 'Не подключено', connecting: 'Подключаюсь…', on: 'Подключено' }[phase]);
  const elapsed = $derived(phase === 'on' ? duration((app.now - app.status.since * 1000) / 1000) : '');

  const nav = [
    { id: null, icon: 'servers', title: 'Карта' },
    { id: 'routes', icon: 'routes', title: 'Маршрутизация' },
    { id: 'log', icon: 'log', title: 'Журнал' },
    { id: 'settings', icon: 'settings', title: 'Настройки' },
  ];
</script>

<aside>
  <section class="status" data-state={phase}>
    <div class="who">
      {#if shown}
        <Flag cc={shown.cc} cell={4} />
        <div class="names">
          <b>{shown.name}</b>
          <span class="state">{label}{#if phase === 'on'}<em class="mono">{elapsed}</em>{/if}</span>
          <span class="detail">
            {#if phase === 'on'}
              {[app.status.exit?.ip ?? countryName(shown.cc), shown.ping > 0 ? `${shown.ping} мс` : ''].filter(Boolean).join(' · ')}
            {:else}
              {[countryName(shown.cc), protoLabel(shown)].filter(Boolean).join(' · ')}
            {/if}
          </span>
        </div>
      {:else}
        <div class="names"><b>Нет серверов</b><span class="detail">Добавь подписку или ссылку на сервер</span></div>
      {/if}
    </div>

    <button class="connect" onclick={toggle}>
      {#if phase === 'connecting'}
        <Loader cell={4} />Отмена
      {:else if phase === 'on'}
        <Icon name="power" />Отключить
      {:else}
        <Icon name="power" />{shown ? 'Подключить' : 'Добавить сервер'}
      {/if}
    </button>
  </section>

  <div class="tools">
    <label class="search">
      <Icon name="search" size={14} />
      <input placeholder="Поиск" bind:value={app.query} spellcheck="false" />
      {#if app.query}<button class="ibtn sm" title="Очистить" onclick={() => (app.query = '')}><Icon name="close" size={12} /></button>{/if}
    </label>
    <button class="ibtn" title="Проверить пинг всех серверов" disabled={!app.servers.length} onclick={() => pingAll()}>
      {#if app.pinging}<Loader cell={3} />{:else}<Icon name="bolt" />{/if}
    </button>
    <button class="ibtn add" title="Добавить (Ctrl+V)" onclick={() => (app.popup = { kind: 'add' })}><Icon name="plus" /></button>
  </div>

  <ServerList />

  <nav>
    {#each nav as n}
      <button class:active={app.panel === n.id} title={n.title} onclick={() => (app.panel = n.id)}>
        <Icon name={n.icon} />
      </button>
    {/each}
  </nav>
</aside>

<style>
  aside {
    width: var(--sidebar);
    flex: none;
    display: flex;
    flex-direction: column;
    background: var(--surface);
    border-right: 1px solid var(--line);
    min-height: 0;
  }
  .status {
    padding: 16px 16px 14px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .who {
    display: flex;
    align-items: flex-start;
    gap: 13px;
    min-height: 58px;
  }
  .who > :global(.flag),
  .who > :global(.code) {
    margin-top: 2px;
  }
  .names {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .names b,
  .names span {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .names b {
    font-size: 17px;
    line-height: 1.15;
    font-weight: 600;
  }
  .state {
    font-size: 13px;
    color: var(--text-2);
    transition: color var(--t);
  }
  .state em {
    font-style: normal;
    margin-left: 8px;
    color: var(--text-2);
  }
  [data-state='connecting'] .state {
    color: var(--warn);
  }
  [data-state='on'] .state {
    color: var(--accent-dim);
  }
  :global([data-theme='dark']) [data-state='on'] .state {
    color: var(--accent);
  }
  .detail {
    font-size: 11.5px;
    color: var(--text-3);
  }
  .connect {
    height: 42px;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 10px;
    border-radius: var(--r);
    font-weight: 600;
    font-size: 13px;
    letter-spacing: 0.02em;
    background: var(--accent);
    color: var(--accent-ink);
    border: 1px solid var(--accent);
    transition: background-color var(--t), color var(--t), border-color var(--t), transform var(--t-fast);
  }
  .connect:hover {
    background: color-mix(in srgb, var(--accent) 88%, #fff);
  }
  .connect:active {
    transform: scale(0.98);
  }
  [data-state='connecting'] .connect {
    background: transparent;
    color: var(--warn);
    border-color: color-mix(in srgb, var(--warn) 45%, transparent);
  }
  [data-state='on'] .connect {
    background: var(--accent-soft);
    color: var(--accent);
    border-color: var(--accent-line);
  }
  [data-state='on'] .connect:hover {
    background: var(--danger-soft);
    color: var(--danger);
    border-color: color-mix(in srgb, var(--danger) 50%, transparent);
  }
  .tools {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 0 10px 10px 16px;
  }
  .search {
    flex: 1;
    min-width: 0;
    height: 30px;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 4px 0 9px;
    background: var(--surface-2);
    border: 1px solid var(--line);
    border-radius: var(--r);
    color: var(--text-3);
    cursor: text;
    transition: border-color var(--t-fast);
  }
  .search:focus-within {
    border-color: var(--accent-line);
  }
  .search input {
    flex: 1;
    min-width: 0;
    background: none;
    border: 0;
    outline: none;
    color: var(--text);
    user-select: text;
  }
  .search input::placeholder {
    color: var(--text-3);
  }
  .ibtn.sm {
    width: 22px;
    height: 22px;
  }
  .ibtn.add {
    color: var(--accent);
  }
  nav {
    flex: none;
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    border-top: 1px solid var(--line);
  }
  nav button {
    height: 44px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-3);
    position: relative;
    transition: color var(--t-fast), background-color var(--t-fast);
  }
  nav button:hover {
    color: var(--text);
    background: var(--surface-2);
  }
  nav button::after {
    content: '';
    position: absolute;
    left: 50%;
    top: 0;
    width: 18px;
    height: 2px;
    margin-left: -9px;
    background: var(--accent);
    transform: scaleX(0);
    transition: transform var(--t) var(--ease);
  }
  nav button.active {
    color: var(--accent);
  }
  nav button.active::after {
    transform: scaleX(1);
  }
</style>
