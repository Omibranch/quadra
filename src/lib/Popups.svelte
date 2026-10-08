<script>
  import { app, importText, copy, toast, save, loadApps } from './store.svelte.js';
  import { invoke } from './api.js';
  import Icon from './Icon.svelte';
  import Loader from './Loader.svelte';

  // Popups grow out of the place that was clicked.
  let origin = { x: innerWidth / 2, y: innerHeight / 2 };
  addEventListener('pointerdown', (e) => (origin = { x: e.clientX, y: e.clientY }), true);

  let text = $state('');
  let shared = $state(null);
  let fileInput = $state();
  const p = $derived(app.popup);

  const close = () => (app.popup = null);

  function grow(node) {
    const r = node.getBoundingClientRect();
    node.style.transformOrigin = `${origin.x - r.left}px ${origin.y - r.top}px`;
    node.querySelector('[data-focus]')?.focus();
  }

  $effect(() => {
    if (p?.kind === 'add') text = p.text ?? '';
    if (p?.kind === 'share') {
      shared = null;
      invoke('share', { id: p.id }).then((r) => (shared = r)).catch((e) => { toast(String(e), 'error'); close(); });
    }
  });

  async function submit() {
    if (!text.trim()) return;
    if (await importText(text)) close();
  }

  async function fromClipboard() {
    const t = (await invoke('clipboard')).trim();
    if (t) text = t;
    else toast('В буфере обмена нет текста');
  }

  async function fromFile(e) {
    const file = e.currentTarget.files?.[0];
    if (file) text = (await file.text()).trim();
    e.currentTarget.value = '';
  }

  async function confirmed() {
    const action = p.action;
    close();
    await action?.();
  }

  async function elevate() {
    try {
      await invoke('relaunch_admin');
    } catch (e) {
      toast(String(e), 'error');
    }
  }

  const sub = $derived(p?.kind === 'announce' ? app.subs.find((s) => s.id === p.sub) : null);

  // ---- choosing programs for the exclusions
  let find = $state('');
  const phone = $derived(app.platform === 'android');
  const chosen = $derived(new Set(app.settings?.bypass_apps.map((a) => a.toLowerCase())));
  const listed = $derived.by(() => {
    const q = find.trim().toLowerCase();
    const all = app.apps ?? [];
    // what is chosen but not installed (typed in by hand, or removed since) stays in the list
    const ids = new Set(all.map((a) => a.id.toLowerCase()));
    const extra = (app.settings?.bypass_apps ?? []).filter((id) => !ids.has(id.toLowerCase())).map((id) => ({ id, name: id }));
    return [...extra, ...all].filter((a) => !q || a.name.toLowerCase().includes(q) || a.id.toLowerCase().includes(q));
  });
  // a name typed in full that is not in the list can be added as it is: on a desktop the rule
  // goes by the name of the executable, whatever it is
  const typed = $derived.by(() => {
    const q = find.trim();
    if (phone || !q || /\s/.test(q)) return '';
    return listed.some((a) => a.id.toLowerCase() === q.toLowerCase()) ? '' : q;
  });

  $effect(() => {
    if (p?.kind === 'apps') {
      find = '';
      loadApps();
    }
  });

  function toggleApp(id) {
    const has = chosen.has(id.toLowerCase());
    app.settings.bypass_apps = has ? app.settings.bypass_apps.filter((a) => a.toLowerCase() !== id.toLowerCase()) : [...app.settings.bypass_apps, id];
    save();
  }

  function addTyped() {
    toggleApp(typed);
    find = '';
  }
</script>

{#if p}
  <div class="veil" onpointerdown={(e) => e.target === e.currentTarget && close()} role="presentation">
    {#key p.kind}
      <div class="popup {p.kind}" use:grow role="dialog" aria-modal="true">
        {#if p.kind === 'add'}
          <h3>Добавить</h3>
          <p>Ссылка на подписку, ссылки на серверы (vless, vmess, trojan, ss) или готовая конфигурация Xray. Можно несколько, по одной в строке.</p>
          <textarea class="input" rows="6" bind:value={text} data-focus spellcheck="false" placeholder="https://… или vless://…"
                    onkeydown={(e) => e.key === 'Enter' && (e.ctrlKey || e.metaKey) && submit()}></textarea>
          <div class="row">
            {#if app.platform !== 'android'}<button class="btn" onclick={fromClipboard}><Icon name="clipboard" />Из буфера</button>{/if}
            <button class="btn" onclick={() => fileInput.click()}><Icon name="file" />Из файла</button>
            <input type="file" bind:this={fileInput} onchange={fromFile} hidden />
            <span class="grow"></span>
            <button class="btn" onclick={close}>Отмена</button>
            <button class="btn primary" disabled={!text.trim() || app.busy.import} onclick={submit}>
              {#if app.busy.import}<Loader cell={3} />{/if}Добавить
            </button>
          </div>
        {:else if p.kind === 'share'}
          <h3>Ссылка на сервер</h3>
          {#if !shared}
            <div class="wait"><Loader cell={5} /></div>
          {:else}
            {#if shared.qr}
              <svg class="qr" viewBox="-2 -2 {shared.qr.size + 4} {shared.qr.size + 4}" shape-rendering="crispEdges">
                {#each shared.qr.cells as c, i}
                  {#if c === '1'}<rect x={i % shared.qr.size + 0.06} y={Math.floor(i / shared.qr.size) + 0.06} width="0.88" height="0.88" />{/if}
                {/each}
              </svg>
            {:else}
              <p>Этот сервер пришёл от провайдера готовой конфигурацией, ссылки у него нет. Скопировать можно саму конфигурацию.</p>
            {/if}
            <code>{shared.text.length > 400 ? shared.text.slice(0, 400) + ' …' : shared.text}</code>
            <div class="row">
              <span class="grow"></span>
              <button class="btn" onclick={close}>Закрыть</button>
              <button class="btn primary" data-focus onclick={() => copy(shared.text, shared.link ? 'Ссылка скопирована' : 'Конфигурация скопирована')}><Icon name="copy" />Скопировать</button>
            </div>
          {/if}
        {:else if p.kind === 'confirm'}
          <h3>{p.title}</h3>
          <p>{p.text}</p>
          <div class="row">
            <span class="grow"></span>
            <button class="btn" data-focus onclick={close}>Отмена</button>
            <button class="btn danger" onclick={confirmed}>{p.ok ?? 'Да'}</button>
          </div>
        {:else if p.kind === 'announce' && sub}
          <h3>{sub.title}</h3>
          <p class="pre">{sub.announce}</p>
          <div class="row">
            {#if sub.support_url}<button class="btn" onclick={() => invoke('open_url', { url: sub.support_url })}><Icon name="external" />Поддержка</button>{/if}
            <span class="grow"></span>
            <button class="btn primary" data-focus onclick={close}>Понятно</button>
          </div>
        {:else if p.kind === 'apps'}
          <h3>Приложения</h3>
          <p>
            {app.settings.bypass_mode === 'only' ? 'Отмеченные пойдут через сервер, остальные напрямую.' : 'Отмеченные пойдут мимо сервера.'}
          </p>
          <label class="find">
            <Icon name="search" size={14} />
            <input bind:value={find} spellcheck="false" autocapitalize="off" placeholder={phone ? 'Поиск' : 'Поиск или имя программы: app.exe'}
                   onkeydown={(e) => e.key === 'Enter' && typed && addTyped()} />
          </label>
          <div class="applist">
            {#if !app.apps}
              <div class="wait small"><Loader cell={5} /></div>
            {:else}
              {#if typed}
                <button class="app" onclick={addTyped}>
                  <i class="box"><Icon name="plus" size={12} /></i>
                  <span class="app-text"><b>Добавить «{typed}»</b><em>по имени программы</em></span>
                </button>
              {/if}
              {#each listed as a (a.id)}
                <button class="app" class:on={chosen.has(a.id.toLowerCase())} onclick={() => toggleApp(a.id)}>
                  <i class="box">{#if chosen.has(a.id.toLowerCase())}<Icon name="check" size={12} />{/if}</i>
                  {#if a.icon}<img src={a.icon} alt="" loading="lazy" />{/if}
                  <span class="app-text"><b>{a.name}</b>{#if !phone && a.id !== a.name}<em>{a.id}</em>{/if}</span>
                </button>
              {:else}
                {#if !typed}
                  <p class="hint">
                    {#if find.trim()}
                      Ничего не найдено
                    {:else if phone}
                      Система не показала ни одного приложения
                    {:else}
                      Список программ здесь недоступен. Впиши имя программы так, как оно выглядит в списке процессов, и нажми Enter.
                    {/if}
                  </p>
                {/if}
              {/each}
            {/if}
          </div>
          <div class="row">
            <span class="picked">Выбрано: {app.settings.bypass_apps.length}</span>
            <span class="grow"></span>
            <button class="btn primary" onclick={close}>Готово</button>
          </div>
        {:else if p.kind === 'admin'}
          <h3>Нужны права администратора</h3>
          <p>Режим «Весь трафик» создаёт виртуальный сетевой адаптер и меняет маршруты системы. Windows разрешает это только программе, запущенной от имени администратора.</p>
          <p>Приложение перезапустится, Windows спросит разрешение, после этого подключение начнётся само.</p>
          <div class="row">
            <span class="grow"></span>
            <button class="btn" onclick={close}>Отмена</button>
            <button class="btn primary" data-focus onclick={elevate}><Icon name="shield" />Перезапустить</button>
          </div>
        {/if}
      </div>
    {/key}
  </div>
{/if}

<style>
  .veil {
    position: fixed;
    inset: var(--titlebar) 0 0 0;
    z-index: 40;
    display: flex;
    align-items: center;
    justify-content: center;
    background: color-mix(in srgb, var(--bg) 72%, transparent);
    animation: fade-in var(--t);
  }
  .popup {
    width: 440px;
    max-width: calc(100vw - 48px);
    padding: 22px 22px 18px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    background: var(--surface);
    border: 1px solid var(--line-2);
    border-radius: var(--r-lg);
    box-shadow: var(--shadow);
    animation: grow 170ms var(--ease);
  }
  .popup.add {
    width: 520px;
  }
  .popup.share {
    width: 380px;
  }
  @keyframes grow {
    from {
      opacity: 0;
      transform: scale(0.86);
    }
  }
  h3 {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
  }
  p {
    margin: 0;
    font-size: 12.5px;
    line-height: 1.55;
    color: var(--text-2);
  }
  .pre {
    white-space: pre-wrap;
    color: var(--text);
    user-select: text;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 6px;
  }
  .grow {
    flex: 1;
  }
  .popup.apps {
    width: 460px;
    max-height: calc(100vh - var(--titlebar) - 48px);
  }
  .find {
    flex: none;
    height: 34px;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 10px;
    color: var(--text-3);
    background: var(--surface-2);
    border: 1px solid var(--line);
    border-radius: var(--r);
    cursor: text;
  }
  .find:focus-within {
    border-color: var(--accent-line);
  }
  .find input {
    flex: 1;
    min-width: 0;
    background: none;
    border: 0;
    outline: none;
    color: var(--text);
    user-select: text;
  }
  .find input::placeholder {
    color: var(--text-3);
  }
  .applist {
    flex: 1 1 340px;
    min-height: 120px;
    overflow-y: auto;
    margin: 0 -8px;
    display: flex;
    flex-direction: column;
  }
  .app {
    flex: none;
    min-height: 42px;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 5px 8px;
    text-align: left;
    border-radius: var(--r);
  }
  .app:hover {
    background: var(--surface-2);
  }
  .box {
    width: 16px;
    height: 16px;
    flex: none;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--accent-ink);
    border: 1px solid var(--text-3);
    border-radius: 2px;
    transition: background-color var(--t-fast), border-color var(--t-fast);
  }
  .app.on .box {
    background: var(--accent);
    border-color: var(--accent);
  }
  .app:not(.on) .box {
    color: var(--text-2);
  }
  .app img {
    width: 28px;
    height: 28px;
    flex: none;
  }
  .app-text {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .app-text b,
  .app-text em {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .app-text b {
    font-weight: 500;
  }
  .app-text em {
    font-style: normal;
    font-size: 11px;
    color: var(--text-3);
  }
  .hint {
    padding: 18px 8px;
    color: var(--text-3);
  }
  .picked {
    font-size: 12px;
    color: var(--text-2);
  }
  .wait.small {
    height: 120px;
  }
  /* a narrow screen: the popup takes the width it is given, and buttons wrap instead of spilling */
  @media (max-width: 720px) {
    .popup {
      max-width: calc(100vw - 24px);
      padding: 18px 16px 14px;
    }
    .row {
      flex-wrap: wrap;
    }
    .find {
      height: 40px;
    }
    .app {
      min-height: 50px;
    }
    .box {
      width: 20px;
      height: 20px;
    }
  }
  .wait {
    height: 200px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--accent);
  }
  .qr {
    width: 220px;
    height: 220px;
    align-self: center;
    background: #fff;
    border-radius: var(--r);
  }
  .qr rect {
    fill: #06140c;
  }
  code {
    display: block;
    max-height: 84px;
    overflow: auto;
    padding: 9px 10px;
    font-size: 11px;
    line-height: 1.5;
    word-break: break-all;
    color: var(--text-2);
    background: var(--surface-2);
    border: 1px solid var(--line);
    border-radius: var(--r);
    user-select: text;
  }
</style>
