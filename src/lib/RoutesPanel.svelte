<script>
  import { app, set, save, connect, loadApps } from './store.svelte.js';
  import Panel from './Panel.svelte';
  import Segmented from './Segmented.svelte';
  import Icon from './Icon.svelte';

  const presets = [
    { value: 'all', title: 'Всё через сервер', text: 'Весь трафик уходит в туннель, включая адреса домашней сети.' },
    { value: 'lan', title: 'Локальная сеть напрямую', text: 'Роутер, принтер и другие устройства дома остаются доступны. Подходит большинству.' },
    { value: 'ru', title: 'Россия и локальная сеть напрямую', text: 'Российские сайты и адреса открываются без сервера, остальное идёт через него.' },
  ];
  const kinds = [{ value: 'domain', label: 'Домен' }, { value: 'ip', label: 'IP' }];
  const outcomes = [{ value: 'proxy', label: 'Через сервер' }, { value: 'direct', label: 'Напрямую' }, { value: 'block', label: 'Блокировать' }];
  const live = $derived(app.status.state !== 'off');

  const phone = $derived(app.platform === 'android');
  const bypassModes = [{ value: 'exclude', label: 'Все, кроме этих' }, { value: 'only', label: 'Только эти' }];
  const only = $derived(app.settings.bypass_mode === 'only');
  let domain = $state('');

  // the list stores ids (an executable or a package); the names come from the system
  $effect(() => {
    if (app.settings.bypass_apps.length) loadApps();
  });
  const known = $derived(new Map((app.apps ?? []).map((a) => [a.id.toLowerCase(), a])));

  /** "https://www.Example.com/page, bank.ru" -> ["example.com", "bank.ru"] */
  const parse = (text) =>
    text.split(/[\s,;]+/)
      .map((v) => v.trim().toLowerCase().replace(/^[a-z]+:\/\//, '').replace(/[/?#].*$/, '').replace(/^(\*\.|www\.)/, ''))
      .filter(Boolean);

  function addDomains() {
    const fresh = parse(domain).filter((d) => !app.settings.bypass_domains.includes(d));
    if (fresh.length) {
      app.settings.bypass_domains.push(...fresh);
      save();
    }
    domain = '';
  }

  function drop(key, value) {
    app.settings[key] = app.settings[key].filter((v) => v !== value);
    save();
  }

  function add() {
    app.settings.rules.push({ kind: 'domain', value: '', action: 'direct' });
    save();
  }

  function remove(i) {
    app.settings.rules.splice(i, 1);
    save();
  }

  function patch(i, key, value) {
    app.settings.rules[i][key] = value;
    save();
  }
</script>

<Panel title="Маршрутизация">
  {#snippet actions()}
    {#if live}<button class="btn" onclick={connect}><Icon name="refresh" />Применить сейчас</button>{/if}
  {/snippet}

  <h3 class="label">Что идёт мимо сервера</h3>
  <div class="presets">
    {#each presets as p}
      <button class="preset" class:on={app.settings.routing === p.value} onclick={() => set('routing', p.value)}>
        <i></i>
        <span><b>{p.title}</b><em>{p.text}</em></span>
      </button>
    {/each}
  </div>
  <p class="note">
    Действует для серверов, добавленных ссылкой. Если провайдер присылает готовую конфигурацию (в списке у таких серверов метка XRAY),
    у неё своя маршрутизация, и она сохраняется как есть.
  </p>

  <h3 class="label">Исключения</h3>
  <Segmented options={bypassModes} value={app.settings.bypass_mode} onchange={(v) => set('bypass_mode', v)} />
  <p class="note">
    {#if only}
      Через сервер идут только выбранные приложения и сайты, всё остальное идёт напрямую. Пока ничего не выбрано, через сервер идёт всё.
    {:else}
      Выбранные приложения и сайты идут мимо сервера, всё остальное идёт через него.
    {/if}
    {#if phone}
      {#if only}Если выбраны приложения, список сайтов не учитывается: решают приложения.{/if}
    {:else}
      Приложения различаются только в режиме «Весь трафик»; в режиме «Прокси» работают сайты.
    {/if}
  </p>

  <div class="pick">
    <div class="pick-head">
      <b>Приложения</b>
      <button class="btn" onclick={() => (app.popup = { kind: 'apps' })}><Icon name="plus" />Выбрать</button>
    </div>
    {#if app.settings.bypass_apps.length}
      <div class="chips">
        {#each app.settings.bypass_apps as id (id)}
          {@const a = known.get(id.toLowerCase())}
          <span class="chip" title={id}>
            {#if a?.icon}<img src={a.icon} alt="" />{/if}
            <span>{a?.name ?? id}</span>
            <button title="Убрать" onclick={() => drop('bypass_apps', id)}><Icon name="close" size={11} /></button>
          </span>
        {/each}
      </div>
    {:else}
      <p class="none">Ничего не выбрано</p>
    {/if}
  </div>

  <div class="pick">
    <div class="pick-head">
      <b>Сайты</b>
    </div>
    <form class="enter" onsubmit={(e) => { e.preventDefault(); addDomains(); }}>
      <input class="input" bind:value={domain} spellcheck="false" autocapitalize="off" autocomplete="off" inputmode="url"
             placeholder="example.com" onpaste={() => setTimeout(() => /[\s,;]/.test(domain.trim()) && addDomains())} />
      <button class="btn" disabled={!domain.trim()}><Icon name="plus" />Добавить</button>
    </form>
    {#if app.settings.bypass_domains.length}
      <div class="chips">
        {#each app.settings.bypass_domains as d (d)}
          <span class="chip">
            <span>{d}</span>
            <button title="Убрать" onclick={() => drop('bypass_domains', d)}><Icon name="close" size={11} /></button>
          </span>
        {/each}
      </div>
    {/if}
  </div>

  <h3 class="label">Свои правила</h3>
  <p class="note top">Проверяются первыми, сверху вниз, и работают с любыми серверами.</p>
  {#each app.settings.rules as rule, i (i)}
    <div class="rule">
      <Segmented small options={kinds} value={rule.kind} onchange={(v) => patch(i, 'kind', v)} />
      <input class="input mono" value={rule.value} spellcheck="false"
             placeholder={rule.kind === 'ip' ? '10.0.0.0/8 или geoip:de' : 'example.com или geosite:google'}
             onchange={(e) => patch(i, 'value', e.currentTarget.value.trim())} />
      <Segmented small options={outcomes} value={rule.action} onchange={(v) => patch(i, 'action', v)} />
      <button class="ibtn" title="Удалить правило" onclick={() => remove(i)}><Icon name="trash" /></button>
    </div>
  {/each}
  <button class="btn add" onclick={add}><Icon name="plus" />Добавить правило</button>
</Panel>

<style>
  h3 {
    margin: 26px 0 12px;
  }
  .presets {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .preset {
    display: flex;
    align-items: flex-start;
    gap: 14px;
    padding: 12px 14px;
    text-align: left;
    background: var(--surface);
    border: 1px solid var(--line);
    border-radius: var(--r);
    transition: border-color var(--t-fast), background-color var(--t-fast);
  }
  .preset:hover {
    border-color: var(--line-2);
  }
  .preset i {
    width: 10px;
    height: 10px;
    margin-top: 4px;
    flex: none;
    border: 1px solid var(--text-3);
    transition: background-color var(--t-fast), border-color var(--t-fast);
  }
  .preset.on {
    border-color: var(--accent-line);
    background: var(--accent-soft);
  }
  .preset.on i {
    background: var(--accent);
    border-color: var(--accent);
  }
  .preset span {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .preset b {
    font-weight: 600;
  }
  .preset em {
    font-style: normal;
    font-size: 11.5px;
    color: var(--text-2);
  }
  .note {
    margin: 12px 0 0;
    max-width: 620px;
    font-size: 11.5px;
    line-height: 1.5;
    color: var(--text-3);
  }
  .note.top {
    margin: -4px 0 12px;
  }
  .pick {
    max-width: 620px;
    margin-top: 14px;
    padding: 12px 14px 14px;
    background: var(--surface);
    border: 1px solid var(--line);
    border-radius: var(--r);
  }
  .pick-head {
    min-height: 30px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }
  .pick-head b {
    font-weight: 600;
  }
  .none {
    margin: 6px 0 0;
    font-size: 12px;
    color: var(--text-3);
  }
  .enter {
    display: flex;
    gap: 8px;
    margin-top: 6px;
  }
  .enter .input {
    flex: 1;
    min-width: 0;
    height: 30px;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 12px;
  }
  .chip {
    max-width: 100%;
    height: 28px;
    display: inline-flex;
    align-items: center;
    gap: 7px;
    padding: 0 3px 0 9px;
    font-size: 12px;
    background: var(--surface-2);
    border: 1px solid var(--line-2);
    border-radius: var(--r);
  }
  .chip > span {
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .chip img {
    width: 16px;
    height: 16px;
    flex: none;
  }
  .chip button {
    width: 22px;
    height: 22px;
    flex: none;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-3);
    border-radius: 2px;
  }
  .chip button:hover {
    color: var(--danger);
    background: var(--danger-soft);
  }
  .rule {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 6px;
    animation: rise var(--t) var(--ease);
  }
  /* a narrow screen: the value takes a line of its own, the switches sit under it */
  @media (max-width: 720px) {
    .rule {
      flex-wrap: wrap;
      padding: 10px;
      background: var(--surface);
      border: 1px solid var(--line);
      border-radius: var(--r);
    }
    .rule .input {
      order: -1;
      flex: 1 1 100%;
      height: 36px;
    }
    .rule :global(.seg) {
      flex: 0 1 auto;
    }
    .rule .ibtn {
      margin-left: auto;
    }
    .enter .input,
    .enter .btn {
      height: 36px;
    }
    .chip {
      height: 32px;
    }
    .chip button {
      width: 26px;
      height: 26px;
    }
  }
  .rule .input {
    flex: 1;
    min-width: 0;
    height: 30px;
  }
  .add {
    margin-top: 6px;
  }
</style>
