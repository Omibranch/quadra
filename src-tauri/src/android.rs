//! Android. There is no system proxy to set and no adapter to create: the app asks for the
//! system VPN through a small Kotlin plugin (plugins/vpn), gets the tunnel device as a file
//! descriptor, and tun2proxy turns the packets read from it into SOCKS connections to the
//! core's local port. The app itself is excluded from the VPN, so the core's own connections
//! to the server go out directly.

use crate::{core, Ctx};
use serde::Deserialize;
use serde_json::json;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_quadra_vpn::Vpn;

const MTU: u16 = 1500;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Info {
    /// Where the package's native libraries are unpacked; the core is one of them.
    pub native_dir: String,
    /// The app's private folder, with the core's data files under `xray/`.
    pub files_dir: String,
    pub model: String,
    pub release: String,
}

#[derive(Deserialize)]
struct Started {
    fd: i32,
}

fn plugin(app: &AppHandle) -> tauri::State<'_, Vpn<Wry>> {
    app.state::<Vpn<Wry>>()
}

pub fn info(app: &AppHandle) -> Result<Info, String> {
    plugin(app).call("info", json!({}))
}

pub fn open_url(app: &AppHandle, url: &str) -> bool {
    let allowed = ["https://", "http://", "tg://"].iter().any(|p| url.starts_with(p));
    allowed && plugin(app).call::<serde_json::Value>("openUrl", json!({ "url": url })).is_ok()
}

/// Installed apps that have a launcher icon: `[{id, name, icon}]`, for the exclusions.
pub fn list_apps(app: &AppHandle) -> Result<serde_json::Value, String> {
    plugin(app).call::<serde_json::Value>("listApps", json!({})).map(|v| v["apps"].clone())
}

/// Brings the system VPN up (asking the user the first time) and starts carrying its packets
/// to the core. Returns what undoes it.
pub async fn start_tunnel(app: &AppHandle, ctx: &Arc<Ctx>, socks_port: u16, gen: u64) -> Result<Box<dyn FnOnce() + Send + Sync>, String> {
    let handle = app.clone();
    // which apps the VPN takes: the system does this itself, per app
    let (only, apps) = {
        let d = ctx.data.lock().unwrap();
        (d.settings.bypass_mode == "only", d.settings.bypass_apps.clone())
    };
    let started: Started = tokio::task::spawn_blocking(move || plugin(&handle).call("start", json!({ "mtu": MTU, "only": only, "apps": apps })))
        .await
        .map_err(|e| e.to_string())??;

    let proxy = tun2proxy::ProxyParameters::try_from(format!("socks5://127.0.0.1:{socks_port}").as_str())
        .map_err(|e| format!("tun2proxy: {e:?}"))?;
    let mut args = tun2proxy::Args::default();
    args.proxy(proxy).tun_fd(Some(started.fd)).close_fd_on_drop(true).dns(tun2proxy::ArgDns::Virtual).ipv6_enabled(true);
    let token = tun2proxy::CancellationToken::new();

    // tun2proxy runs on a runtime of its own: it is a long-lived packet loop
    let (app2, ctx2, token2) = (app.clone(), ctx.clone(), token.clone());
    std::thread::spawn(move || {
        let outcome = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
            Ok(rt) => rt.block_on(tun2proxy::general_run_async(args, MTU, false, token2)).map(|_| ()).map_err(|e| e.to_string()),
            Err(e) => Err(e.to_string()),
        };
        // it ended on its own while the connection was still meant to be up:
        // the system took the VPN away, or the packet loop failed
        if ctx2.gen.load(Ordering::SeqCst) == gen {
            let reason = match outcome {
                Ok(()) => "система отключила VPN".to_string(),
                Err(e) => format!("туннель остановился: {e}"),
            };
            tauri::async_runtime::spawn(async move { core::disconnect(&app2, &ctx2, Some(reason)).await });
        }
    });

    let app = app.clone();
    Ok(Box::new(move || {
        token.cancel();
        std::thread::spawn(move || {
            let _ = plugin(&app).call::<serde_json::Value>("stop", json!({}));
        });
    }))
}
