<script>
  import { app, set, save, connect } from './store.svelte.js';
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

  const parse = (text) => text.split(/[\s,;]+/).map((v) => v.trim()).filter(Boolean);

  function setList(key, e) {
    app.settings[key] = parse(e.currentTarget.value);
    e.currentTarget.value = app.settings[key].join(', ');
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
  <p class="note top">
    Эти программы и сайты всегда идут мимо сервера, в любом режиме и с любым сервером. Программы различаются
    только в режиме «Весь трафик»; в режиме «Прокси» работают домены.
  </p>
  <label class="list">
    <span>Программы</span>
    <input class="input mono" value={app.settings.bypass_apps.join(', ')} spellcheck="false" placeholder="chrome.exe, Steam.exe"
           onchange={(e) => setList('bypass_apps', e)} />
  </label>
  <label class="list">
    <span>Домены</span>
    <input class="input mono" value={app.settings.bypass_domains.join(', ')} spellcheck="false" placeholder="example.com, bank.ru"
           onchange={(e) => setList('bypass_domains', e)} />
  </label>

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
  .list {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-bottom: 6px;
  }
  .list span {
    width: 86px;
    flex: none;
    color: var(--text-2);
  }
  .list .input {
    flex: 1;
    min-width: 0;
    height: 30px;
  }
  .rule {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 6px;
    animation: rise var(--t) var(--ease);
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
