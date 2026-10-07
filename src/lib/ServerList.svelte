<script>
  import { app, select, connect, refreshSub, deleteSub, deleteServer, pingAll, set, copy, bestServer } from './store.svelte.js';
  import { invoke } from './api.js';
  import { countryName } from './flags.js';
  import { bytes, expiry, protoLabel, count } from './format.js';
  import Flag from './Flag.svelte';
  import Icon from './Icon.svelte';
  import PingBar from './PingBar.svelte';
  import Loader from './Loader.svelte';

  const ROW = 46;
  const GROUP = 56;

  let scroller = $state();
  let scrollTop = $state(0);
  let height = $state(400);

  const collapsed = $derived(new Set(app.settings.collapsed));

  const matches = (s, q) =>
    !q || s.name.toLowerCase().includes(q) || countryName(s.cc).toLowerCase().includes(q) || protoLabel(s).toLowerCase().includes(q);

  // The list as flat rows with known heights, so only the visible ones are in the DOM.
  const items = $derived.by(() => {
    const q = app.query.trim().toLowerCase();
    const out = [];
    if (!q && app.servers.length) out.push({ type: 'auto', key: 'auto', h: ROW });
    const groups = [...app.subs.map((sub) => ({ sub, id: sub.id })), { sub: null, id: 'manual' }];
    for (const g of groups) {
      const servers = app.servers.filter((s) => (s.sub ?? 'manual') === g.id && matches(s, q));
      if (!g.sub && !servers.length) continue;
      if (q && !servers.length) continue;
      out.push({ type: 'group', key: 'g:' + g.id, h: GROUP, sub: g.sub, id: g.id, n: servers.length });
      if (!collapsed.has(g.id) || q) for (const s of servers) out.push({ type: 'server', key: s.id, h: ROW, s });
    }
    let y = 0;
    for (const it of out) {
      it.y = y;
      y += it.h;
    }
    return { rows: out, total: y };
  });

  const visible = $derived(items.rows.filter((it) => it.y + it.h > scrollTop - 240 && it.y < scrollTop + height + 240));
  const auto = $derived(bestServer());
  const selected = $derived(app.settings.selected ?? 'auto');

  function toggleGroup(id) {
    set('collapsed', collapsed.has(id) ? app.settings.collapsed.filter((x) => x !== id) : [...app.settings.collapsed, id]);
  }

  function open(e, itemsList) {
    e.preventDefault();
    app.menu = { x: e.clientX, y: e.clientY, items: itemsList.filter(Boolean) };
  }

  function serverMenu(e, s) {
    open(e, [
      { label: 'Подключить', icon: 'power', action: () => { set('selected', s.id); connect(); } },
      { label: 'Проверить пинг', icon: 'bolt', action: () => pingAll([s.id]) },
      { label: 'Ссылка и QR', icon: 'qr', action: () => (app.popup = { kind: 'share', id: s.id }) },
      !s.sub && { label: 'Удалить', icon: 'trash', danger: true, action: () => (app.popup = { kind: 'confirm', title: 'Удалить сервер?', text: s.name, ok: 'Удалить', action: () => deleteServer(s.id) }) },
    ]);
  }

  function groupMenu(e, sub) {
    if (!sub) return;
    open(e, [
      { label: 'Обновить', icon: 'refresh', action: () => refreshSub(sub.id) },
      { label: 'Проверить пинг', icon: 'bolt', action: () => pingAll(app.servers.filter((s) => s.sub === sub.id).map((s) => s.id)) },
      sub.announce && { label: 'Сообщение провайдера', icon: 'info', action: () => (app.popup = { kind: 'announce', sub: sub.id }) },
      sub.support_url && { label: 'Поддержка', icon: 'external', action: () => invoke('open_url', { url: sub.support_url }) },
      { label: 'Скопировать адрес', icon: 'copy', action: () => copy(sub.url, 'Адрес подписки скопирован') },
      { label: 'Удалить', icon: 'trash', danger: true, action: () => (app.popup = { kind: 'confirm', title: 'Удалить подписку?', text: `${sub.title} и все её серверы`, ok: 'Удалить', action: () => deleteSub(sub.id) }) },
    ]);
  }

  function usage(sub) {
    const parts = [];
    if (sub.total) parts.push(`${bytes(sub.upload + sub.download)} из ${bytes(sub.total)}`);
    const left = expiry(sub.expire);
    if (left) parts.push(left);
    return parts.join(' · ');
  }
</script>

<div class="list" bind:this={scroller} bind:clientHeight={height} onscroll={() => (scrollTop = scroller.scrollTop)}>
  {#if !app.servers.length && !app.subs.length}
    <div class="empty">
      <Loader cell={6} still />
      <p>Серверов пока нет</p>
      <span>Вставь ссылку на подписку или сервер: кнопка «+» или просто Ctrl+V</span>
      <button class="btn primary" onclick={() => (app.popup = { kind: 'add' })}><Icon name="plus" />Добавить</button>
    </div>
  {:else if !items.rows.length}
    <div class="empty"><p>Ничего не найдено</p><span>По запросу «{app.query}» серверов нет</span></div>
  {:else}
    <div class="spacer" style="height:{items.total}px">
      {#each visible as it (it.key)}
        {#if it.type === 'auto'}
          <button class="row" class:active={selected === 'auto'} style="transform:translateY({it.y}px)" onclick={() => select('auto')} ondblclick={connect}>
            <span class="auto"><Icon name="bolt" /></span>
            <span class="text">
              <span class="name">Авто, самый быстрый</span>
              <span class="sub mono">{auto ? auto.name : 'сначала проверю пинг'}</span>
            </span>
            {#if auto?.ping > 0}<PingBar ms={auto.ping} />{/if}
          </button>
        {:else if it.type === 'group'}
          <div class="group" style="transform:translateY({it.y}px)" oncontextmenu={(e) => groupMenu(e, it.sub)} role="presentation">
            <button class="fold" class:open={!collapsed.has(it.id)} onclick={() => toggleGroup(it.id)}>
              <Icon name="chevron" size={14} />
              <span class="text">
                <span class="title">{it.sub ? it.sub.title : 'Свои серверы'}<em class="mono">{it.n}</em></span>
                {#if it.sub?.error}
                  <span class="sub err" title={it.sub.error}>{it.sub.error}</span>
                {:else if it.sub && usage(it.sub)}
                  <span class="sub mono">{usage(it.sub)}</span>
                {:else}
                  <span class="sub mono">{count(it.n, 'сервер', 'сервера', 'серверов')}</span>
                {/if}
              </span>
            </button>
            {#if it.sub}
              <button class="ibtn" title="Обновить подписку" onclick={() => refreshSub(it.sub.id)}>
                {#if app.busy['sub:' + it.sub.id]}<Loader cell={3} />{:else}<Icon name="refresh" />{/if}
              </button>
            {/if}
          </div>
        {:else}
          {@const s = it.s}
          <button class="row" class:active={selected === s.id} class:live={app.status.server === s.id && app.status.state === 'on'}
                  style="transform:translateY({it.y}px)" onclick={() => select(s.id)}
                  ondblclick={() => { set('selected', s.id); connect(); }} oncontextmenu={(e) => serverMenu(e, s)}>
            <Flag cc={s.cc} />
            <span class="text">
              <span class="name">{s.name}</span>
              <span class="sub mono">{protoLabel(s)}{s.note ? ` · ${s.note}` : ''}</span>
            </span>
            <PingBar ms={s.ping} pending={app.pinging} />
          </button>
        {/if}
      {/each}
    </div>
  {/if}
</div>

<style>
  .list {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overflow-x: hidden;
    position: relative;
  }
  .spacer {
    position: relative;
  }
  .row,
  .group {
    position: absolute;
    left: 0;
    right: 0;
    top: 0;
    will-change: transform;
  }
  .row {
    height: 46px;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 0 12px 0 16px;
    text-align: left;
    border-left: 2px solid transparent;
    transition: background-color var(--t-fast), border-color var(--t-fast);
  }
  .row:hover {
    background: var(--surface-2);
  }
  .row.active {
    background: var(--accent-soft);
    border-left-color: var(--accent);
  }
  .row.live .name {
    color: var(--accent-dim);
  }
  :global([data-theme='dark']) .row.live .name {
    color: var(--accent);
  }
  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .name,
  .sub {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .name {
    font-weight: 500;
  }
  .sub {
    font-size: 10.5px;
    color: var(--text-3);
    letter-spacing: 0.02em;
  }
  .sub.err {
    color: var(--danger);
    font-family: var(--font);
    font-size: 11px;
  }
  .auto {
    width: 23px;
    height: 23px;
    flex: none;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--accent);
    border: 1px solid var(--accent-line);
    border-radius: 2px;
  }
  .group {
    height: 56px;
    display: flex;
    align-items: center;
    padding: 10px 10px 0 0;
    border-top: 1px solid var(--line);
    background: var(--surface);
  }
  .fold {
    flex: 1;
    min-width: 0;
    height: 100%;
    display: flex;
    align-items: center;
    gap: 9px;
    padding-left: 13px;
    text-align: left;
    color: var(--text-3);
  }
  .fold :global(svg) {
    transition: transform var(--t) var(--ease);
  }
  .fold.open :global(svg) {
    transform: rotate(90deg);
  }
  .title {
    display: flex;
    align-items: baseline;
    gap: 8px;
    font-weight: 600;
    color: var(--text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .title em {
    font-style: normal;
    font-size: 10.5px;
    color: var(--text-3);
  }
  .empty {
    height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 24px 32px;
    text-align: center;
    color: var(--text-3);
  }
  .empty p {
    margin: 10px 0 0;
    font-weight: 600;
    font-size: 14px;
    color: var(--text);
  }
  .empty span {
    font-size: 12px;
    line-height: 1.5;
  }
  .empty .btn {
    margin-top: 12px;
  }
</style>
