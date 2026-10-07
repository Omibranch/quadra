//! Running Xray: start, confirm the tunnel really carries traffic, watch it, stop it.

use crate::model::{Home, Status};
use crate::{config, emit_status, sys, Ctx};
use serde_json::{json, Value};
use std::net::Ipv4Addr;
use std::process::Stdio;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

pub struct Running {
    child: tokio::process::Child,
    metrics: u16,
    /// Started through the privileged wrapper (Unix, all-traffic mode): we cannot kill it,
    /// only ask it to stop.
    wrapped: bool,
}

const PROXY_BACKUP: &str = "proxy_backup.json";
/// The core's output of the current and the previous connection, for looking at afterwards.
const LOG_FILE: &str = "core.log";

fn log(app: &AppHandle, ctx: &Ctx, line: String) {
    let mut logs = ctx.logs.lock().unwrap();
    if logs.len() >= 1500 {
        logs.pop_front();
    }
    logs.push_back(line.clone());
    drop(logs);
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(ctx.dir.join(LOG_FILE)) {
        use std::io::Write;
        let _ = writeln!(f, "{line}");
    }
    let _ = app.emit("log", line);
}

fn set_status(app: &AppHandle, ctx: &Ctx, f: impl FnOnce(&mut Status)) {
    f(&mut ctx.status.lock().unwrap());
    emit_status(app, ctx);
}

/// Puts back whatever system proxy was set before us. Safe to call when nothing was changed.
pub fn restore_proxy(ctx: &Ctx) {
    let path = ctx.dir.join(PROXY_BACKUP);
    if let Ok(text) = std::fs::read_to_string(&path) {
        if let Ok(backup) = serde_json::from_str::<sys::ProxyBackup>(&text) {
            let _ = sys::restore_system_proxy(&backup);
        }
        let _ = std::fs::remove_file(path);
    }
}

async fn stop(ctx: &Ctx) {
    if let Some(mut running) = ctx.core.lock().await.take() {
        if running.wrapped {
            sys::release_tun(&ctx.dir.join("run"));
            let _ = tokio::time::timeout(Duration::from_secs(3), running.child.wait()).await;
        }
        let _ = running.child.kill().await;
    }
    restore_proxy(ctx);
}

pub async fn disconnect(app: &AppHandle, ctx: &Arc<Ctx>, error: Option<String>) {
    ctx.gen.fetch_add(1, Ordering::SeqCst);
    stop(ctx).await;
    set_status(app, ctx, |s| {
        let server = s.server.take();
        *s = Status { state: "off".into(), error, ..Default::default() };
        s.server = server;
    });
}

async fn port_free(port: u16) -> bool {
    TcpListener::bind((Ipv4Addr::LOCALHOST, port)).await.is_ok()
}

async fn free_port() -> u16 {
    match TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await {
        Ok(l) => l.local_addr().map(|a| a.port()).unwrap_or(20811),
        Err(_) => 20811,
    }
}

fn through(socks: u16, secs: u64) -> Option<reqwest::Client> {
    let proxy = reqwest::Proxy::all(format!("socks5h://127.0.0.1:{socks}")).ok()?;
    reqwest::Client::builder().proxy(proxy).timeout(Duration::from_secs(secs)).build().ok()
}

/// A real request through the tunnel. The connection counts as up only when this succeeds.
async fn probe(socks: u16) -> Option<u32> {
    let client = through(socks, 5)?;
    for url in ["http://cp.cloudflare.com/generate_204", "https://www.gstatic.com/generate_204", "http://cp.cloudflare.com/generate_204"] {
        let t = Instant::now();
        if let Ok(r) = client.get(url).send().await {
            if r.status().as_u16() < 400 {
                return Some(t.elapsed().as_millis() as u32);
            }
        }
    }
    None
}

async fn where_am_i(client: &reqwest::Client) -> Option<Home> {
    for url in ["https://ipwho.is/", "https://ipapi.co/json/"] {
        let Ok(resp) = client.get(url).send().await else { continue };
        let Ok(text) = resp.text().await else { continue };
        let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
        if let (Some(lat), Some(lon)) = (v["latitude"].as_f64(), v["longitude"].as_f64()) {
            return Some(Home {
                lat, lon,
                cc: v["country_code"].as_str().unwrap_or("").to_string(),
                ip: v["ip"].as_str().unwrap_or("").to_string(),
                manual: false,
            });
        }
    }
    None
}

/// The user's own location by IP, asked directly (never through a tunnel).
pub async fn locate_home() -> Option<Home> {
    let client = reqwest::Client::builder().no_proxy().timeout(Duration::from_secs(8)).build().ok()?;
    where_am_i(&client).await
}

fn traffic_totals(vars: &Value) -> (u64, u64) {
    let (mut up, mut down) = (0, 0);
    if let Some(inbound) = vars["stats"]["inbound"].as_object() {
        for v in inbound.values() {
            up += v["uplink"].as_u64().unwrap_or(0);
            down += v["downlink"].as_u64().unwrap_or(0);
        }
    }
    (up, down)
}

/// Runs for as long as this connection is the current one: reports speed, notices a dead core.
async fn watch(app: AppHandle, ctx: Arc<Ctx>, gen: u64) {
    let client = reqwest::Client::builder().no_proxy().timeout(Duration::from_secs(2)).build().ok();
    let mut last: Option<(u64, u64, Instant)> = None;
    loop {
        tokio::time::sleep(Duration::from_millis(1000)).await;
        if ctx.gen.load(Ordering::SeqCst) != gen {
            return;
        }
        let metrics = {
            let mut core = ctx.core.lock().await;
            match core.as_mut() {
                Some(r) => match r.child.try_wait() {
                    Ok(None) => r.metrics,
                    _ => {
                        drop(core);
                        disconnect(&app, &ctx, Some("ядро неожиданно остановилось, подробности в журнале".into())).await;
                        return;
                    }
                },
                None => return,
            }
        };
        let Some(client) = &client else { continue };
        let Ok(resp) = client.get(format!("http://127.0.0.1:{metrics}/debug/vars")).send().await else { continue };
        let Ok(vars) = resp.json_value().await else { continue };
        let (up, down) = traffic_totals(&vars);
        let now = Instant::now();
        if let Some((pu, pd, pt)) = last {
            let dt = now.duration_since(pt).as_secs_f64().max(0.2);
            let _ = app.emit("traffic", json!({
                "up": (up.saturating_sub(pu) as f64 / dt) as u64,
                "down": (down.saturating_sub(pd) as f64 / dt) as u64,
                "tup": up, "tdown": down}));
        }
        last = Some((up, down, now));
    }
}

trait JsonValue {
    async fn json_value(self) -> Result<Value, ()>;
}

impl JsonValue for reqwest::Response {
    async fn json_value(self) -> Result<Value, ()> {
        let text = self.text().await.map_err(|_| ())?;
        serde_json::from_str(&text).map_err(|_| ())
    }
}

pub async fn connect(app: AppHandle, ctx: Arc<Ctx>, id: String) -> Result<(), String> {
    let gen = ctx.gen.fetch_add(1, Ordering::SeqCst) + 1;
    let launch = ctx.launch.lock().await;
    if ctx.gen.load(Ordering::SeqCst) != gen {
        return Ok(()); // a newer request is already waiting its turn
    }
    stop(&ctx).await;
    let (server, settings) = {
        let d = ctx.data.lock().unwrap();
        let server = d.servers.iter().find(|s| s.id == id).cloned().ok_or("сервер не найден")?;
        (server, d.settings.clone())
    };
    let tun = settings.mode == "tun";
    set_status(&app, &ctx, |s| *s = Status { state: "connecting".into(), server: Some(id.clone()), tun, ..Default::default() });

    let fail = |msg: String| {
        let (app, ctx) = (app.clone(), ctx.clone());
        async move {
            if ctx.gen.load(Ordering::SeqCst) == gen {
                disconnect(&app, &ctx, Some(msg.clone())).await;
            }
            Err::<(), String>(msg)
        }
    };

    if tun && sys::needs_relaunch_for_tun() {
        return fail("режим «весь трафик» требует прав администратора".into()).await;
    }
    if tun {
        if let Ok(Some(other)) = tokio::task::spawn_blocking(sys::other_tunnel).await {
            return fail(format!("уже работает другой VPN (адаптер {other}): два туннеля мешают друг другу. Выключи его или выбери режим «Прокси»")).await;
        }
    }
    for port in [settings.socks_port, settings.http_port] {
        let mut free = port_free(port).await;
        for _ in 0..20 {
            if free {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
            free = port_free(port).await;
        }
        if !free {
            return fail(format!("порт {port} занят другой программой: смени его в настройках")).await;
        }
    }
    let metrics = free_port().await;
    let cfg = match config::build(&server, &settings, tun, metrics) {
        Ok(c) => c,
        Err(e) => return fail(e).await,
    };
    let run_dir = ctx.dir.join("run");
    let _ = std::fs::create_dir_all(&run_dir);
    let cfg_path = run_dir.join("config.json");
    if let Err(e) = std::fs::write(&cfg_path, serde_json::to_vec(&cfg).unwrap_or_default()) {
        return fail(format!("не удалось записать конфигурацию: {e}")).await;
    }

    let xray = ctx.xray.join(sys::XRAY_BIN);
    let wrapper = if tun && !sys::is_admin() { sys::tun_wrapper(&xray, &ctx.xray, &cfg_path, &run_dir) } else { None };
    let wrapped = wrapper.is_some();
    if tun && cfg!(unix) && !wrapped && !sys::is_admin() {
        return fail("для режима «весь трафик» нужен pkexec (пакет polkit): установи его или выбери другой режим".into()).await;
    }
    let mut cmd = wrapper.unwrap_or_else(|| {
        let mut c = tokio::process::Command::new(&xray);
        c.arg("run").arg("-c").arg(&cfg_path).current_dir(&ctx.xray).env("XRAY_LOCATION_ASSET", &ctx.xray);
        c
    });
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
    sys::prepare(&mut cmd);
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return fail(format!("не удалось запустить ядро: {e}")).await,
    };
    sys::adopt(&child);
    if wrapped {
        // the wrapper sends the core's output to a file; follow it
        let (app, ctx, path) = (app.clone(), ctx.clone(), run_dir.join("core.out"));
        tokio::spawn(async move {
            let mut seen = 0usize;
            while ctx.gen.load(Ordering::SeqCst) == gen {
                tokio::time::sleep(Duration::from_millis(300)).await;
                let Ok(text) = std::fs::read_to_string(&path) else { continue };
                let Some(end) = text.rfind('\n') else { continue };
                if end + 1 > seen {
                    for line in text[seen..=end].lines() {
                        log(&app, &ctx, line.to_string());
                    }
                    seen = end + 1;
                }
            }
        });
    }
    let log_path = ctx.dir.join(LOG_FILE);
    if std::fs::metadata(&log_path).map(|m| m.len() > 512 * 1024).unwrap_or(false) {
        let _ = std::fs::rename(&log_path, ctx.dir.join("core.old.log"));
    }
    log(&app, &ctx, format!("--- подключение: {} | режим {} | {} ---", server.name, settings.mode, if sys::is_admin() { "администратор" } else { "обычные права" }));
    if let Some(out) = child.stdout.take() {
        let (app, ctx) = (app.clone(), ctx.clone());
        tokio::spawn(async move {
            let mut lines = BufReader::new(out).lines();
            while let Ok(Some(l)) = lines.next_line().await {
                log(&app, &ctx, l);
            }
        });
    }
    if let Some(err) = child.stderr.take() {
        let (app, ctx) = (app.clone(), ctx.clone());
        tokio::spawn(async move {
            let mut lines = BufReader::new(err).lines();
            while let Ok(Some(l)) = lines.next_line().await {
                log(&app, &ctx, l);
            }
        });
    }

    *ctx.core.lock().await = Some(Running { child, metrics, wrapped });
    drop(launch);
    // a password prompt may stand between us and the core
    let patience = Duration::from_secs(if wrapped { 120 } else { 10 });

    let started = Instant::now();
    loop {
        if ctx.gen.load(Ordering::SeqCst) != gen {
            return Ok(());
        }
        let exited = match ctx.core.lock().await.as_mut() {
            Some(r) => !matches!(r.child.try_wait(), Ok(None)),
            None => return Ok(()),
        };
        if exited {
            tokio::time::sleep(Duration::from_millis(120)).await;
            let reason = ctx.logs.lock().unwrap().iter().rev().find(|l| l.contains("Failed") || l.contains("failed") || l.contains("rror"))
                .cloned().unwrap_or_else(|| "подробности в журнале".into());
            return fail(format!("ядро не запустилось: {}", reason.chars().take(220).collect::<String>())).await;
        }
        if TcpStream::connect((Ipv4Addr::LOCALHOST, settings.socks_port)).await.is_ok() {
            break;
        }
        if started.elapsed() > patience {
            return fail("ядро не запустилось вовремя".into()).await;
        }
        tokio::time::sleep(Duration::from_millis(60)).await;
    }

    let ms = probe(settings.socks_port).await;
    if ctx.gen.load(Ordering::SeqCst) != gen {
        return Ok(());
    }
    let Some(ms) = ms else {
        return fail("сервер не отвечает: соединение установить не удалось".into()).await;
    };

    if settings.mode == "proxy" {
        match sys::set_system_proxy(settings.http_port, settings.socks_port, &settings.bypass_domains) {
            Ok(backup) => {
                let _ = std::fs::write(ctx.dir.join(PROXY_BACKUP), serde_json::to_vec(&backup).unwrap_or_default());
            }
            Err(e) => {
                return fail(format!("не удалось включить системный прокси: {e}")).await;
            }
        }
    }
    set_status(&app, &ctx, |s| {
        s.state = "on".into();
        s.since = crate::now();
        s.ms = Some(ms);
    });

    tokio::spawn(watch(app.clone(), ctx.clone(), gen));
    if settings.geo_lookup {
        let socks = settings.socks_port;
        tokio::spawn(async move {
            let Some(client) = through(socks, 8) else { return };
            if let Some(exit) = where_am_i(&client).await {
                if ctx.gen.load(Ordering::SeqCst) == gen {
                    set_status(&app, &ctx, |s| s.exit = serde_json::to_value(&exit).ok());
                }
            }
        });
    }
    Ok(())
}

async fn tcp_ping(host: String, port: u16) -> i32 {
    let Ok(Ok(mut addrs)) = tokio::time::timeout(Duration::from_secs(3), tokio::net::lookup_host((host.as_str(), port))).await else { return -1 };
    let Some(addr) = addrs.next() else { return -1 };
    let mut best = -1i32;
    for _ in 0..2 {
        let t = Instant::now();
        if let Ok(Ok(_)) = tokio::time::timeout(Duration::from_millis(2500), TcpStream::connect(addr)).await {
            let ms = t.elapsed().as_millis() as i32;
            best = if best < 0 { ms } else { best.min(ms) };
        } else if best < 0 {
            break;
        }
    }
    best
}

/// Time to open a TCP connection to each server, measured in parallel.
pub async fn ping(app: AppHandle, ctx: Arc<Ctx>, ids: Vec<String>) {
    let targets: Vec<(String, String, u16)> = {
        let d = ctx.data.lock().unwrap();
        d.servers.iter().filter(|s| ids.is_empty() || ids.contains(&s.id)).filter(|s| !s.address.is_empty())
            .map(|s| (s.id.clone(), s.address.clone(), s.port)).collect()
    };
    let limit = Arc::new(tokio::sync::Semaphore::new(24));
    let mut tasks = vec![];
    for (id, host, port) in targets {
        let (app, ctx, limit) = (app.clone(), ctx.clone(), limit.clone());
        tasks.push(tokio::spawn(async move {
            let _permit = limit.acquire().await;
            let ms = tcp_ping(host, port).await;
            ctx.pings.lock().unwrap().insert(id.clone(), ms);
            let _ = app.emit("ping", json!({"id": id, "ms": ms}));
        }));
    }
    for t in tasks {
        let _ = t.await;
    }
}
