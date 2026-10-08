//! The whole backend. `main.rs` only calls `run()`; on Android the same function is the entry
//! point of the native library.

#[cfg(target_os = "android")]
mod android;
mod config;
mod core;
mod geoip;
mod links;
mod model;
mod names;
mod subs;
mod sys;

use model::{Data, Home, Settings, Status};
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
#[cfg(desktop)]
use tauri::menu::{Menu, MenuItem};
#[cfg(desktop)]
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, RunEvent, State, WindowEvent};
#[cfg(desktop)]
use tauri_plugin_autostart::ManagerExt;

pub struct Ctx {
    pub dir: PathBuf,
    /// Where the core binary is.
    pub xray: PathBuf,
    /// Where geoip.dat and geosite.dat are (next to the binary, except on Android).
    pub assets: PathBuf,
    pub data: Mutex<Data>,
    pub pings: Mutex<HashMap<String, i32>>,
    pub status: Mutex<Status>,
    pub logs: Mutex<VecDeque<String>>,
    pub core: tokio::sync::Mutex<Option<core::Running>>,
    /// Held while a core is being stopped and the next one started, so two connects never overlap.
    pub launch: tokio::sync::Mutex<()>,
    pub gen: AtomicU64,
    /// The core that is starting has reported that it is up.
    pub core_ready: AtomicBool,
    /// Server names already looked up, and when (see core::resolve_servers).
    pub resolved: Mutex<HashMap<String, (std::net::IpAddr, std::time::Instant)>>,
    pub hidden: bool,
    pub elevated: bool,
    /// Under a tiling window manager: no splash window, no window buttons, no hiding to a tray.
    pub tiling: bool,
    /// The startup cube's window is still up.
    pub splash: AtomicBool,
    /// The main window has finished loading.
    pub main_ready: AtomicBool,
}

impl Ctx {
    pub fn save(&self) {
        let bytes = serde_json::to_vec(&*self.data.lock().unwrap()).unwrap_or_default();
        let tmp = self.dir.join("state.json.tmp");
        if std::fs::write(&tmp, bytes).is_ok() {
            let _ = std::fs::rename(tmp, self.dir.join("state.json"));
        }
    }
}

type Shared<'a> = State<'a, Arc<Ctx>>;

pub fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

pub fn emit_status(app: &AppHandle, ctx: &Ctx) {
    let _ = app.emit("status", ctx.status.lock().unwrap().clone());
}

fn snapshot(ctx: &Ctx) -> Value {
    let d = ctx.data.lock().unwrap();
    let pings = ctx.pings.lock().unwrap();
    json!({
        "settings": d.settings,
        "subs": d.subs,
        "servers": d.servers.iter().map(|s| s.view(pings.get(&s.id).copied())).collect::<Vec<_>>(),
        "status": *ctx.status.lock().unwrap(),
        "hwid": d.hwid,
        "admin": sys::is_admin(),
        "platform": sys::PLATFORM,
        "tiling": ctx.tiling,
        "relaunchForTun": sys::needs_relaunch_for_tun(),
        "elevated": ctx.elevated,
        "version": env!("CARGO_PKG_VERSION"),
        "userAgent": subs::default_user_agent(),
    })
}

fn changed(app: &AppHandle, ctx: &Ctx) {
    ctx.save();
    let _ = app.emit("data", snapshot(ctx));
}

/// Servers named without a flag get their country from where their address points.
async fn fill_countries(ctx: &Arc<Ctx>, servers: &mut [model::Server]) {
    let dat = ctx.assets.join("geoip.dat");
    for s in servers.iter_mut().filter(|s| s.cc.is_empty() && !s.address.is_empty()) {
        let ip = match s.address.parse::<IpAddr>() {
            Ok(ip) => Some(ip),
            Err(_) => tokio::time::timeout(std::time::Duration::from_secs(2), tokio::net::lookup_host((s.address.as_str(), s.port)))
                .await.ok().and_then(|r| r.ok()).and_then(|mut a| a.find(|a| a.is_ipv4())).map(|a| a.ip()),
        };
        if let Some(IpAddr::V4(v4)) = ip {
            let dat = dat.clone();
            if let Ok(Some(cc)) = tokio::task::spawn_blocking(move || geoip::country(&dat, v4)).await {
                s.cc = cc;
            }
        }
    }
}

async fn refresh(app: &AppHandle, ctx: &Arc<Ctx>, id: &str, url: &str) -> Result<Value, String> {
    let (hwid, send, ua) = {
        let d = ctx.data.lock().unwrap();
        (d.hwid.clone(), d.settings.send_hwid, d.settings.user_agent.clone())
    };
    match subs::fetch(url, id, &hwid, send, &ua).await {
        Ok(mut f) => {
            fill_countries(ctx, &mut f.servers).await;
            let count = f.servers.len();
            {
                let mut d = ctx.data.lock().unwrap();
                d.servers.retain(|s| s.sub.as_deref() != Some(id));
                d.servers.extend(f.servers);
                match d.subs.iter_mut().find(|s| s.id == id) {
                    Some(s) => *s = f.sub,
                    None => d.subs.push(f.sub),
                }
            }
            changed(app, ctx);
            Ok(json!({"servers": count, "skipped": f.skipped}))
        }
        Err(e) => {
            let known = {
                let mut d = ctx.data.lock().unwrap();
                match d.subs.iter_mut().find(|s| s.id == id) {
                    Some(s) => { s.error = e.clone(); true }
                    None => false,
                }
            };
            if known {
                changed(app, ctx);
            }
            Err(e)
        }
    }
}

#[tauri::command]
fn get_state(ctx: Shared) -> Value {
    snapshot(&ctx)
}

/// Takes the startup cube away and makes sure the main window is up.
fn reveal(app: &AppHandle, ctx: &Ctx) {
    if ctx.splash.swap(false, Ordering::SeqCst) {
        if let Some(w) = app.get_webview_window("splash") {
            let _ = w.close();
        }
        // after the close, or the window that was active before us takes the front again
        show_main(app);
    }
}

/// The interface is loaded. With a splash on screen the cube first finishes its turn; started
/// hidden at login, nothing is shown at all.
#[tauri::command]
fn ready(app: AppHandle, ctx: Shared) {
    if cfg!(debug_assertions) && std::env::var_os("QUADRA_SPLASH_HOLD").is_some() {
        return; // development aid: keep the startup cube on screen to look at it
    }
    ctx.main_ready.store(true, Ordering::SeqCst);
    if ctx.hidden {
        return;
    }
    if !ctx.splash.load(Ordering::SeqCst) {
        show_main(&app);
        return;
    }
    let _ = app.emit_to("splash", "finish", ());
    let ctx = ctx.inner().clone();
    tauri::async_runtime::spawn(async move {
        // never leave the user looking at a cube: if the splash does not answer, go on without it
        tokio::time::sleep(std::time::Duration::from_millis(4000)).await;
        reveal(&app, &ctx);
    });
}

/// From the splash window: "loaded" shows it and tells whether the main window is already waiting,
/// "leaving" brings the main window up under the fading cube, "done" closes the splash.
#[tauri::command]
fn splash(app: AppHandle, ctx: Shared, stage: String) -> bool {
    match stage.as_str() {
        // The window is created hidden: until the page has painted, a transparent window is
        // just a black square. It comes up with the cube already drawn.
        "loaded" => {
            if let Some(w) = app.get_webview_window("splash") {
                let _ = w.show();
            }
        }
        "leaving" => show_main(&app),
        "done" => reveal(&app, &ctx),
        _ => {}
    }
    ctx.main_ready.load(Ordering::SeqCst)
}

#[tauri::command]
fn save_settings(app: AppHandle, ctx: Shared, settings: Settings) {
    let autostart = {
        let mut d = ctx.data.lock().unwrap();
        let before = d.settings.autostart;
        d.settings = settings;
        (before != d.settings.autostart).then_some(d.settings.autostart)
    };
    #[cfg(desktop)]
    if let Some(on) = autostart {
        let launcher = app.autolaunch();
        let _ = if on { launcher.enable() } else { launcher.disable() };
    }
    let _ = (&app, autostart);
    ctx.save();
}

#[tauri::command]
async fn add_subscription(app: AppHandle, ctx: Shared<'_>, url: String) -> Result<Value, String> {
    let url = url.trim().to_string();
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("это не ссылка на подписку".into());
    }
    let id = subs::short_hash(&url);
    refresh(&app, ctx.inner(), &id, &url).await
}

#[tauri::command]
async fn refresh_subscription(app: AppHandle, ctx: Shared<'_>, id: String) -> Result<Value, String> {
    let url = ctx.data.lock().unwrap().subs.iter().find(|s| s.id == id).map(|s| s.url.clone()).ok_or("подписка не найдена")?;
    refresh(&app, ctx.inner(), &id, &url).await
}

#[tauri::command]
async fn delete_subscription(app: AppHandle, ctx: Shared<'_>, id: String) -> Result<(), String> {
    let active = ctx.status.lock().unwrap().server.clone();
    let hit = {
        let mut d = ctx.data.lock().unwrap();
        let hit = d.servers.iter().any(|s| s.sub.as_deref() == Some(&id) && Some(&s.id) == active.as_ref());
        d.servers.retain(|s| s.sub.as_deref() != Some(id.as_str()));
        d.subs.retain(|s| s.id != id);
        let selected = d.settings.selected.clone();
        if selected.is_some_and(|sel| !d.servers.iter().any(|s| s.id == sel)) {
            d.settings.selected = None;
        }
        hit
    };
    if hit {
        core::disconnect(&app, ctx.inner(), None).await;
    }
    changed(&app, &ctx);
    Ok(())
}

#[tauri::command]
async fn delete_server(app: AppHandle, ctx: Shared<'_>, id: String) -> Result<(), String> {
    if ctx.status.lock().unwrap().server.as_deref() == Some(id.as_str()) {
        core::disconnect(&app, ctx.inner(), None).await;
    }
    {
        let mut d = ctx.data.lock().unwrap();
        d.servers.retain(|s| s.id != id);
        if d.settings.selected.as_deref() == Some(id.as_str()) {
            d.settings.selected = None;
        }
    }
    changed(&app, &ctx);
    Ok(())
}

/// Anything pasted or dropped: subscription addresses, share links, a base64 list, an Xray config.
#[tauri::command]
async fn import_text(app: AppHandle, ctx: Shared<'_>, text: String) -> Result<Value, String> {
    let text = text.trim();
    let is_url = |l: &str| l.starts_with("https://") || l.starts_with("http://");
    let urls: Vec<String> = text.lines().map(str::trim).filter(|l| is_url(l)).map(String::from).collect();
    let rest = if text.starts_with('[') || text.starts_with('{') { text.to_string() } else {
        text.lines().map(str::trim).filter(|l| !is_url(l)).collect::<Vec<_>>().join("\n")
    };
    let (mut added_subs, mut errors) = (0, vec![]);
    for url in urls {
        match refresh(&app, ctx.inner(), &subs::short_hash(&url), &url).await {
            Ok(_) => added_subs += 1,
            Err(e) => errors.push(e),
        }
    }
    let (mut servers, skipped) = subs::parse_body(&rest);
    subs::assign_ids(&mut servers, None);
    fill_countries(ctx.inner(), &mut servers).await;
    let added = {
        let mut d = ctx.data.lock().unwrap();
        let fresh: Vec<_> = servers.into_iter().filter(|n| !d.servers.iter().any(|s| s.id == n.id)).collect();
        let n = fresh.len();
        d.servers.extend(fresh);
        n
    };
    if added > 0 {
        changed(&app, &ctx);
    }
    if added == 0 && added_subs == 0 {
        return Err(errors.into_iter().next().unwrap_or_else(|| {
            if skipped > 0 { "этот тип ссылок не поддерживается".into() } else { "здесь нет ни ссылок на серверы, ни подписок".into() }
        }));
    }
    Ok(json!({"servers": added, "subs": added_subs, "skipped": skipped, "errors": errors}))
}

#[tauri::command]
async fn connect(app: AppHandle, ctx: Shared<'_>, id: String) -> Result<(), String> {
    core::connect(app, ctx.inner().clone(), id).await
}

#[tauri::command]
async fn disconnect(app: AppHandle, ctx: Shared<'_>) -> Result<(), String> {
    core::disconnect(&app, ctx.inner(), None).await;
    Ok(())
}

#[tauri::command]
async fn ping(app: AppHandle, ctx: Shared<'_>, ids: Vec<String>) -> Result<(), String> {
    core::ping(app, ctx.inner().clone(), ids).await;
    Ok(())
}

#[tauri::command]
async fn locate_home(app: AppHandle, ctx: Shared<'_>) -> Result<Home, String> {
    if ctx.status.lock().unwrap().state != "off" {
        return Err("определить местоположение можно только при отключённом соединении".into());
    }
    let home = core::locate_home().await.ok_or("не удалось определить местоположение")?;
    ctx.data.lock().unwrap().settings.home = Some(home.clone());
    changed(&app, &ctx);
    Ok(home)
}

/// The share link of a server and its QR code as a grid of cells.
#[tauri::command]
fn share(ctx: Shared, id: String) -> Result<Value, String> {
    let d = ctx.data.lock().unwrap();
    let s = d.servers.iter().find(|s| s.id == id).ok_or("сервер не найден")?;
    let text = match (&s.uri, &s.config) {
        (Some(uri), _) => uri.clone(),
        (None, Some(c)) => serde_json::to_string_pretty(c).unwrap_or_default(),
        _ => return Err("у сервера нет ссылки".into()),
    };
    let qr = s.uri.as_ref().and_then(|u| qrcode::QrCode::new(u.as_bytes()).ok()).map(|code| {
        let cells: String = code.to_colors().iter().map(|c| if *c == qrcode::Color::Dark { '1' } else { '0' }).collect();
        json!({"size": code.width(), "cells": cells})
    });
    Ok(json!({"text": text, "link": s.uri.is_some(), "qr": qr}))
}

#[tauri::command]
fn open_url(app: AppHandle, url: String) -> bool {
    #[cfg(target_os = "android")]
    return android::open_url(&app, &url);
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        sys::open_url(&url)
    }
}

#[tauri::command]
fn clipboard() -> String {
    sys::clipboard_text()
}

#[tauri::command]
fn get_logs(ctx: Shared) -> Vec<String> {
    ctx.logs.lock().unwrap().iter().cloned().collect()
}

#[tauri::command]
fn clear_logs(ctx: Shared) {
    ctx.logs.lock().unwrap().clear();
}

/// Restarts the app with administrator rights, which the all-traffic mode needs.
#[tauri::command]
async fn relaunch_admin(app: AppHandle, ctx: Shared<'_>) -> Result<(), String> {
    if !sys::relaunch_elevated("--elevated") {
        return Err("запуск с правами администратора отменён".into());
    }
    core::disconnect(&app, ctx.inner(), None).await;
    app.exit(0);
    Ok(())
}

#[tauri::command]
fn quit(app: AppHandle) {
    app.exit(0);
}

fn show_main(app: &AppHandle) {
    #[cfg(desktop)]
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
    let _ = app;
}

/// Development aid, debug builds only: a file `autotest.json` in the data folder
/// (`{"link": "...", "connect": true, "hold": 30}`) is imported at startup, the connection is made, and the
/// outcome is written next to it as `autotest-result.json`. This is how the Android build is
/// exercised on an emulator, where nobody is there to tap.
#[cfg(debug_assertions)]
async fn autotest(app: AppHandle, ctx: Arc<Ctx>) {
    let Ok(text) = std::fs::read_to_string(ctx.dir.join("autotest.json")) else { return };
    let Ok(job) = serde_json::from_str::<Value>(&text) else { return };
    let report = |v: Value| {
        let _ = std::fs::write(ctx.dir.join("autotest-result.json"), v.to_string());
    };
    report(json!({"stage": "started"}));
    let (mut servers, skipped) = subs::parse_body(job["link"].as_str().unwrap_or(""));
    subs::assign_ids(&mut servers, None);
    let Some(id) = servers.first().map(|s| s.id.clone()) else {
        return report(json!({"stage": "import", "error": "no server in the link", "skipped": skipped}));
    };
    {
        let mut d = ctx.data.lock().unwrap();
        d.servers.retain(|s| s.id != id);
        d.servers.extend(servers);
        d.settings.selected = Some(id.clone());
        d.settings.geo_lookup = false;
    }
    changed(&app, &ctx);
    if job["connect"].as_bool().unwrap_or(false) {
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        let result = core::connect(app.clone(), ctx.clone(), id).await;
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        let status = serde_json::to_value(&*ctx.status.lock().unwrap()).unwrap_or_default();
        report(json!({"stage": "connect", "result": result.err(), "status": status}));
        // "hold": seconds to stay connected before disconnecting again, to check the way back
        if let Some(hold) = job["hold"].as_u64() {
            tokio::time::sleep(std::time::Duration::from_secs(hold)).await;
            core::disconnect(&app, &ctx, None).await;
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            let status = serde_json::to_value(&*ctx.status.lock().unwrap()).unwrap_or_default();
            let _ = std::fs::write(ctx.dir.join("autotest-disconnect.json"), json!({"stage": "disconnect", "status": status}).to_string());
        }
    } else {
        report(json!({"stage": "imported"}));
    }
}

/// Refreshes subscriptions that are older than their update interval.
async fn auto_update(app: AppHandle, ctx: Arc<Ctx>) {
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    loop {
        let due: Vec<(String, String)> = {
            let d = ctx.data.lock().unwrap();
            let hours = d.settings.sub_update_hours;
            d.subs.iter().filter(|s| {
                let every = match (hours, s.update_hours) {
                    (0, _) => return false,
                    (h, 0) => h,
                    (h, p) => h.min(p),
                };
                now().saturating_sub(s.updated_at) >= every as u64 * 3600
            }).map(|s| (s.id.clone(), s.url.clone())).collect()
        };
        for (id, url) in due {
            let _ = refresh(&app, &ctx, &id, &url).await;
        }
        tokio::time::sleep(std::time::Duration::from_secs(600)).await;
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let args: Vec<String> = std::env::args().collect();
    let elevated = args.iter().any(|a| a == "--elevated");
    let hidden = args.iter().any(|a| a == "--hidden");
    if elevated {
        // The instance that asked for elevation is still shutting down.
        std::thread::sleep(std::time::Duration::from_millis(900));
    }

    let mut builder = tauri::Builder::default();
    #[cfg(desktop)]
    {
        builder = builder
            .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_main(app)))
            .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, Some(vec!["--hidden"])));
    }
    #[cfg(target_os = "android")]
    {
        builder = builder.plugin(tauri_plugin_quadra_vpn::init());
    }
    let app = builder
        .setup(move |app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            #[cfg(not(target_os = "android"))]
            let (xray, assets) = {
                let d = app.path().resource_dir()?.join("resources").join("xray");
                (d.clone(), d)
            };
            // On Android the core ships as a native library of the package, and its data files
            // are unpacked into the app's own folder by the plugin.
            #[cfg(target_os = "android")]
            let (xray, assets) = {
                let info = android::info(app.handle()).map_err(std::io::Error::other)?;
                sys::set_device(&info.model, &info.release);
                (PathBuf::from(&info.native_dir), PathBuf::from(&info.files_dir).join("xray"))
            };
            let tiling = sys::tiling_wm();
            let saved: Option<Data> = std::fs::read(dir.join("state.json")).ok().and_then(|b| serde_json::from_slice(&b).ok());
            let first_run = saved.is_none();
            let mut data = saved.unwrap_or_default();
            if data.hwid.is_empty() {
                data.hwid = subs::make_hwid();
            }
            if first_run && tiling {
                data.settings.close_to_tray = false; // most tiling setups have no tray to come back from
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(xray.join(sys::XRAY_BIN), std::fs::Permissions::from_mode(0o755));
            }
            let with_splash = cfg!(desktop) && !hidden && !tiling;
            let ctx = Arc::new(Ctx {
                dir, xray, assets,
                data: Mutex::new(data),
                pings: Mutex::new(HashMap::new()),
                status: Mutex::new(Status { state: "off".into(), ..Default::default() }),
                logs: Mutex::new(VecDeque::new()),
                core: tokio::sync::Mutex::new(None),
                launch: tokio::sync::Mutex::new(()),
                gen: AtomicU64::new(0),
                core_ready: AtomicBool::new(false),
                resolved: Mutex::new(HashMap::new()),
                hidden, elevated, tiling,
                splash: AtomicBool::new(with_splash),
                main_ready: AtomicBool::new(false),
            });
            core::restore_proxy(&ctx); // left over from a crash
            ctx.save();
            app.manage(ctx.clone());

            let handle = app.handle();
            #[cfg(desktop)]
            {
            let open = MenuItem::with_id(handle, "open", "Открыть", true, None::<&str>)?;
            let toggle = MenuItem::with_id(handle, "toggle", "Подключить / отключить", true, None::<&str>)?;
            let exit = MenuItem::with_id(handle, "quit", "Выход", true, None::<&str>)?;
            let menu = Menu::with_items(handle, &[&open, &toggle, &exit])?;
            let mut tray = TrayIconBuilder::with_id("main").tooltip("Quadra").menu(&menu).show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => show_main(app),
                    "toggle" => { let _ = app.emit("tray-toggle", ()); }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                        show_main(tray.app_handle());
                    }
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            let _ = tray.build(app); // a desktop without a tray is no reason not to start

            if with_splash {
                // The startup cube: a bare transparent window, nothing but the object itself.
                let mut builder = tauri::WebviewWindowBuilder::new(app, "splash", tauri::WebviewUrl::App("splash.html".into()));
                // WebView2 refuses a second window whose browser arguments differ from the first one's
                if let Some(args) = app.config().app.windows.first().and_then(|w| w.additional_browser_args.clone()) {
                    builder = builder.additional_browser_args(&args);
                }
                let built = builder
                    .title("Quadra")
                    .inner_size(320.0, 320.0)
                    .center()
                    .decorations(false)
                    .transparent(true)
                    .shadow(false)
                    .resizable(false)
                    .always_on_top(true)
                    .skip_taskbar(true)
                    .focused(false)
                    .visible(false)
                    .build();
                if built.is_err() {
                    ctx.splash.store(false, Ordering::SeqCst);
                }
            }
            }

            #[cfg(debug_assertions)]
            tauri::async_runtime::spawn(autotest(handle.clone(), ctx.clone()));
            tauri::async_runtime::spawn(auto_update(handle.clone(), ctx));
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let ctx = window.state::<Arc<Ctx>>();
                if cfg!(desktop) && window.label() == "main" && ctx.data.lock().unwrap().settings.close_to_tray {
                    api.prevent_close();
                    #[cfg(desktop)]
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_state, ready, splash, save_settings, add_subscription, refresh_subscription, delete_subscription, delete_server,
            import_text, connect, disconnect, ping, locate_home, share, open_url, clipboard, get_logs, clear_logs, relaunch_admin, quit
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app, event| {
        if let RunEvent::Exit = event {
            // The core dies with the job object; the system proxy has to be put back by hand.
            core::restore_proxy(&app.state::<Arc<Ctx>>());
        }
    });
}
