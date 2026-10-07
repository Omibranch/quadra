<script>
  import { app, set, save, connect, locateHome, copy, toast } from './store.svelte.js';
  import { countryName } from './flags.js';
  import Panel from './Panel.svelte';
  import Field from './Field.svelte';
  import Toggle from './Toggle.svelte';
  import Segmented from './Segmented.svelte';
  import Icon from './Icon.svelte';
  import Flag from './Flag.svelte';
  import Loader from './Loader.svelte';

  const accents = ['#3ddc84', '#2fe0c5', '#b6e23c', '#4da3ff', '#a78bfa', '#ff6fae', '#ff9a3c', '#f2d24b'];
  const s = $derived(app.settings);
  const live = $derived(app.status.state !== 'off');
  let locating = $state(false);

  // Settings that only take effect on the next connection.
  function setLink(key, value) {
    set(key, value);
  }

  function port(key, e) {
    const v = Math.round(Number(e.currentTarget.value));
    if (v >= 1024 && v <= 65535) setLink(key, v);
    else {
      e.currentTarget.value = s[key];
      toast('Порт должен быть от 1024 до 65535', 'error');
    }
  }

  async function locate() {
    locating = true;
    set('geo_lookup', true);
    await locateHome();
    locating = false;
  }
</script>

<Panel title="Настройки">
  {#snippet actions()}
    {#if live}<button class="btn" onclick={connect}><Icon name="refresh" />Переподключить</button>{/if}
  {/snippet}

  <h3 class="label">Вид</h3>
  <Field title="Тема">
    <Segmented value={s.theme} onchange={(v) => set('theme', v)} options={[{ value: 'dark', label: 'Тёмная' }, { value: 'light', label: 'Светлая' }]} />
  </Field>
  <Field title="Акцентный цвет" hint="Перекрашивает всё, включая карту">
    <div class="swatches">
      {#each accents as c}
        <button class="swatch" class:on={s.accent.toLowerCase() === c} style="--c:{c}" title={c} aria-label={c} onclick={() => set('accent', c)}></button>
      {/each}
      <label class="swatch custom" class:on={!accents.includes(s.accent.toLowerCase())} style="--c:{s.accent}" title="Свой цвет">
        <input type="color" value={s.accent} oninput={(e) => set('accent', e.currentTarget.value)} />
        <Icon name="edit" size={12} />
      </label>
    </div>
  </Field>
  <Field title="Эффекты" hint="«Меньше» отключает фоновую анимацию карты и сокращает переходы">
    <Segmented value={s.effects} onchange={(v) => set('effects', v)} options={[{ value: 'full', label: 'Все' }, { value: 'lite', label: 'Меньше' }]} />
  </Field>

  <h3 class="label">Подключение</h3>
  <Field title="Режим" hint="Прокси: программы, которые учитывают системный прокси. Весь трафик: виртуальный адаптер, система спросит права администратора. Порты: систему не трогаем.">
    <Segmented value={s.mode} onchange={(v) => setLink('mode', v)}
               options={[{ value: 'proxy', label: 'Прокси' }, { value: 'tun', label: 'Весь трафик' }, { value: 'ports', label: 'Порты' }]} />
  </Field>
  <Field title="Локальные порты" hint="SOCKS5 и HTTP на 127.0.0.1">
    <label class="port"><span class="mono">SOCKS</span><input class="input mono" type="number" value={s.socks_port} onchange={(e) => port('socks_port', e)} /></label>
    <label class="port"><span class="mono">HTTP</span><input class="input mono" type="number" value={s.http_port} onchange={(e) => port('http_port', e)} /></label>
  </Field>
  <Field title="Доступ из локальной сети" hint="Другие устройства в твоей сети смогут пользоваться этими портами">
    <Toggle checked={s.allow_lan} onchange={(v) => setLink('allow_lan', v)} />
  </Field>
  <Field title="DNS" hint="Через запятую. Для серверов, добавленных ссылкой">
    <input class="input mono wide" value={s.dns} spellcheck="false" onchange={(e) => setLink('dns', e.currentTarget.value)} />
  </Field>
  {#if live}<p class="note">Изменения в этом разделе применятся при следующем подключении.</p>{/if}

  <h3 class="label">Запуск</h3>
  <Field title="Запускать при входе в систему" hint="Стартует свёрнутым в трей">
    <Toggle checked={s.autostart} onchange={(v) => set('autostart', v)} />
  </Field>
  <Field title="Подключаться при запуске">
    <Toggle checked={s.autoconnect} onchange={(v) => set('autoconnect', v)} />
  </Field>
  <Field title="Крестик сворачивает в трей" hint="Выйти совсем можно из меню значка в трее">
    <Toggle checked={s.close_to_tray} onchange={(v) => set('close_to_tray', v)} />
  </Field>

  <h3 class="label">Подписки</h3>
  <Field title="Обновлять автоматически">
    <Segmented small value={s.sub_update_hours} onchange={(v) => set('sub_update_hours', v)}
               options={[{ value: 0, label: 'Выкл' }, { value: 1, label: '1 ч' }, { value: 6, label: '6 ч' }, { value: 12, label: '12 ч' }, { value: 24, label: '24 ч' }]} />
  </Field>
  <Field title="Сообщать провайдеру об устройстве" hint="Провайдеры с лимитом устройств не отдают серверы клиенту, который не назвал себя. Отправляется идентификатор, «Windows», версия системы и имя компьютера.">
    <Toggle checked={s.send_hwid} onchange={(v) => set('send_hwid', v)} />
  </Field>
  <Field title="Идентификатор устройства" hint="По нему провайдер считает это устройство">
    <code>{app.hwid}</code>
    <button class="ibtn" title="Скопировать" onclick={() => copy(app.hwid)}><Icon name="copy" /></button>
  </Field>
  <Field title="User-Agent" hint="Некоторые провайдеры пускают только знакомые клиенты. Пусто — стандартный">
    <input class="input mono wide" value={s.user_agent} placeholder={app.userAgent} spellcheck="false" onchange={(e) => set('user_agent', e.currentTarget.value.trim())} />
  </Field>

  <h3 class="label">Местоположение на карте</h3>
  <Field title="Определять по IP" hint="Один запрос к ipwho.is при запуске и один после подключения. Выключи, если не хочешь лишних запросов, и поставь точку вручную.">
    <Toggle checked={s.geo_lookup} onchange={(v) => set('geo_lookup', v)} />
  </Field>
  <Field title="Сейчас" hint={s.home ? (s.home.manual ? 'Задано вручную' : `Определено по адресу ${s.home.ip}`) : 'Неизвестно'}>
    {#if s.home}<span class="where"><Flag cc={s.home.cc} cell={2} />{countryName(s.home.cc) || 'точка на карте'}</span>{/if}
    <button class="btn" disabled={locating || live} title={live ? 'Доступно при отключённом соединении' : ''} onclick={locate}>
      {#if locating}<Loader cell={3} />{/if}Определить
    </button>
    <button class="btn" onclick={() => { app.panel = null; app.pickHome = true; }}><Icon name="pin" />На карте</button>
  </Field>

  <h3 class="label">Ядро</h3>
  <Field title="Подробность журнала">
    <Segmented small value={s.log_level} onchange={(v) => setLink('log_level', v)}
               options={[{ value: 'warning', label: 'Важное' }, { value: 'info', label: 'Подробно' }, { value: 'debug', label: 'Отладка' }]} />
  </Field>
  <Field title="О программе" hint="Соединения обслуживает Xray-core">
    <code>Quadra {app.version}{app.admin ? ' · администратор' : ''}</code>
  </Field>
</Panel>

<style>
  h3 {
    margin: 26px 0 6px;
  }
  .swatches {
    display: flex;
    gap: 6px;
  }
  .swatch {
    width: 22px;
    height: 22px;
    background: var(--c);
    border-radius: 2px;
    outline: 1px solid transparent;
    outline-offset: 2px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: rgba(0, 0, 0, 0.6);
    cursor: pointer;
    transition: transform var(--t-fast), outline-color var(--t-fast);
  }
  .swatch:hover {
    transform: scale(1.12);
  }
  .swatch.on {
    outline-color: var(--c);
  }
  .custom {
    position: relative;
    overflow: hidden;
  }
  .custom input {
    position: absolute;
    inset: -6px;
    opacity: 0;
    cursor: pointer;
  }
  .port {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .port span {
    font-size: 10.5px;
    color: var(--text-3);
    letter-spacing: 0.06em;
  }
  .port input {
    width: 82px;
    text-align: right;
  }
  .wide {
    width: 280px;
  }
  input[type='number']::-webkit-inner-spin-button {
    display: none;
  }
  code {
    font-size: 11.5px;
    color: var(--text-2);
    user-select: text;
  }
  .where {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-right: 6px;
    color: var(--text-2);
  }
  .note {
    margin: 10px 0 0;
    font-size: 11.5px;
    color: var(--warn);
  }
</style>
