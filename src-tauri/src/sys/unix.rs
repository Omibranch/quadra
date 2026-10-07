//! macOS and Linux: the same jobs done with the tools those systems ship with.

use std::path::Path;
use std::process::Command;

pub const PLATFORM: &str = if cfg!(target_os = "macos") { "macos" } else { "linux" };
pub const XRAY_BIN: &str = "xray";
/// macOS only accepts utunN names and picks one itself.
pub const TUN_NAME: Option<&str> = if cfg!(target_os = "macos") { None } else { Some("quadra") };

fn out(program: &str, args: &[&str]) -> Option<String> {
    let o = Command::new(program).args(args).output().ok()?;
    o.status.success().then(|| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

fn ok(program: &str, args: &[&str]) -> bool {
    Command::new(program).args(args).status().map(|s| s.success()).unwrap_or(false)
}

fn has(program: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(program).is_file()))
}

pub fn prepare(cmd: &mut tokio::process::Command) {
    // On Linux the kernel kills the core if the app dies without cleaning up. macOS has no
    // such call; there the core is stopped on the way out (kill_on_drop and the exit handler).
    #[cfg(target_os = "linux")]
    unsafe {
        cmd.pre_exec(|| {
            libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL);
            Ok(())
        });
    }
    let _ = cmd;
}

pub fn adopt(_child: &tokio::process::Child) {}

pub fn is_admin() -> bool {
    unsafe { libc::geteuid() == 0 }
}

/// Only the core needs root for the all-traffic mode, and it asks for it itself (tun_wrapper).
pub fn needs_relaunch_for_tun() -> bool {
    false
}

pub fn relaunch_elevated(_args: &str) -> bool {
    false
}

const TUN_FLAG: &str = "tun.flag";

/// The all-traffic mode needs root for the tunnel device and the routes. The core is started
/// by a small script run through the system's own password prompt (pkexec, or the macOS
/// administrator dialog). We cannot signal a root process afterwards, so the script watches a
/// flag file in our folder and stops the core when it disappears.
pub fn tun_wrapper(xray: &Path, assets: &Path, config: &Path, run: &Path) -> Option<tokio::process::Command> {
    let quote = |p: &Path| format!("'{}'", p.to_string_lossy().replace('\'', r"'\''"));
    let flag = run.join(TUN_FLAG);
    let log = run.join("core.out");
    let script = run.join("tun.sh");
    let body = format!(
        "#!/bin/sh\nexport XRAY_LOCATION_ASSET={assets}\n{xray} run -c {config} > {log} 2>&1 &\npid=$!\n\
         trap 'kill $pid 2>/dev/null' EXIT INT TERM\n\
         while [ -e {flag} ] && kill -0 $pid 2>/dev/null; do sleep 0.3; done\nkill $pid 2>/dev/null\n",
        assets = quote(assets), xray = quote(xray), config = quote(config), log = quote(&log), flag = quote(&flag));
    std::fs::write(&script, body).ok()?;
    std::fs::write(&flag, b"").ok()?;
    let _ = std::fs::write(&log, b"");
    let mut cmd = if cfg!(target_os = "macos") {
        let apple = format!("do shell script \"/bin/sh {}\" with administrator privileges", quote(&script).replace('\\', "\\\\").replace('"', "\\\""));
        let mut c = tokio::process::Command::new("osascript");
        c.args(["-e", &apple]);
        c
    } else {
        if !has("pkexec") {
            return None;
        }
        let mut c = tokio::process::Command::new("pkexec");
        c.arg("/bin/sh").arg(&script);
        c
    };
    cmd.current_dir(run);
    Some(cmd)
}

pub fn release_tun(run: &Path) {
    let _ = std::fs::remove_file(run.join(TUN_FLAG));
}

/// Tiling window managers place windows themselves: no title bar buttons, no splash window,
/// and usually no tray to hide into.
pub fn tiling_wm() -> bool {
    if cfg!(target_os = "macos") {
        return false;
    }
    let env = |k: &str| std::env::var(k).unwrap_or_default().to_lowercase();
    if ["SWAYSOCK", "I3SOCK", "HYPRLAND_INSTANCE_SIGNATURE", "NIRI_SOCKET"].iter().any(|k| std::env::var_os(k).is_some()) {
        return true;
    }
    let desktop = format!("{}:{}:{}", env("XDG_CURRENT_DESKTOP"), env("XDG_SESSION_DESKTOP"), env("DESKTOP_SESSION"));
    ["sway", "i3", "hyprland", "niri", "river", "bspwm", "dwm", "awesome", "qtile", "xmonad", "herbstluftwm", "leftwm", "spectrwm", "dwl", "labwc"]
        .iter().any(|wm| desktop.split(':').any(|d| d == *wm))
}

pub fn open_url(url: &str) -> bool {
    let allowed = ["https://", "http://", "tg://"].iter().any(|p| url.starts_with(p));
    let opener = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
    allowed && Command::new(opener).arg(url).spawn().is_ok()
}

pub fn clipboard_text() -> String {
    let tries: &[(&str, &[&str])] = if cfg!(target_os = "macos") {
        &[("pbpaste", &[])]
    } else {
        &[("wl-paste", &["-n"]), ("xclip", &["-o", "-selection", "clipboard"]), ("xsel", &["-b", "-o"])]
    };
    tries.iter().find_map(|(p, a)| out(p, a)).unwrap_or_default()
}

pub fn machine_guid() -> String {
    if cfg!(target_os = "macos") {
        return out("ioreg", &["-rd1", "-c", "IOPlatformExpertDevice"])
            .and_then(|t| t.lines().find(|l| l.contains("IOPlatformUUID")).and_then(|l| l.split('"').nth(3).map(String::from)))
            .unwrap_or_default();
    }
    ["/etc/machine-id", "/var/lib/dbus/machine-id"].iter()
        .find_map(|p| std::fs::read_to_string(p).ok()).map(|s| s.trim().to_string()).unwrap_or_default()
}

pub fn os_name() -> &'static str {
    if cfg!(target_os = "macos") { "macOS" } else { "Linux" }
}

pub fn os_version() -> String {
    if cfg!(target_os = "macos") {
        return out("sw_vers", &["-productVersion"]).unwrap_or_default();
    }
    let release = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
    let field = |key: &str| release.lines().find_map(|l| l.strip_prefix(key)).map(|v| v.trim_matches('"').to_string());
    match (field("ID="), field("VERSION_ID=")) {
        (Some(id), Some(v)) => format!("{id} {v}"),
        (Some(id), None) => id,
        _ => out("uname", &["-r"]).unwrap_or_default(),
    }
}

pub fn device_name() -> String {
    std::fs::read_to_string("/etc/hostname").ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
        .or_else(|| out("hostname", &[])).unwrap_or_else(|| "PC".into())
}

/// Another VPN that already owns the default route (see the Windows side for why it matters).
pub fn other_tunnel() -> Option<String> {
    if cfg!(target_os = "macos") {
        return None; // the system keeps utun devices of its own; they say nothing
    }
    let routes = out("ip", &["-o", "route", "show", "default"])?;
    routes.lines().filter_map(|l| l.split_whitespace().skip_while(|w| *w != "dev").nth(1))
        .find(|dev| *dev != "quadra" && ["tun", "wg", "tap", "utun", "nordlynx", "proton"].iter().any(|p| dev.starts_with(p)))
        .map(String::from)
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
pub struct ProxyBackup {
    /// "gnome", "kde" or "macos": which mechanism was used.
    kind: String,
    /// GNOME: the proxy mode before us. macOS: the network services that were changed.
    before: Vec<String>,
}

fn unsupported() -> std::io::Error {
    std::io::Error::other("на этом рабочем столе нет общей настройки прокси: используй режим «Порты» или «Весь трафик»")
}

fn kde_tool() -> Option<&'static str> {
    ["kwriteconfig6", "kwriteconfig5"].into_iter().find(|t| has(t))
}

fn kde_set(tool: &str, key: &str, value: &str) -> bool {
    ok(tool, &["--file", "kioslaverc", "--group", "Proxy Settings", "--key", key, value])
}

fn kde_reload() {
    let _ = Command::new("dbus-send").args(["--type=signal", "/KIO/Scheduler", "org.kde.KIO.Scheduler.reparseSlaveConfiguration", "string:"]).status();
}

/// Points the desktop's proxy setting at the local ports and remembers how to undo it.
pub fn set_system_proxy(http_port: u16, socks_port: u16, bypass_domains: &[String]) -> std::io::Result<ProxyBackup> {
    let (http, socks) = (http_port.to_string(), socks_port.to_string());
    let mut skip: Vec<String> = ["localhost", "127.0.0.0/8", "10.0.0.0/8", "172.16.0.0/12", "192.168.0.0/16", "::1"].iter().map(|s| s.to_string()).collect();
    skip.extend(bypass_domains.iter().map(|d| d.trim().trim_start_matches("*.").to_string()).filter(|d| !d.is_empty() && !d.contains(':')));

    if cfg!(target_os = "macos") {
        let services: Vec<String> = out("networksetup", &["-listallnetworkservices"]).unwrap_or_default()
            .lines().skip(1).filter(|l| !l.starts_with('*')).map(String::from).collect();
        if services.is_empty() {
            return Err(unsupported());
        }
        for s in &services {
            ok("networksetup", &["-setwebproxy", s, "127.0.0.1", &http]);
            ok("networksetup", &["-setsecurewebproxy", s, "127.0.0.1", &http]);
            ok("networksetup", &["-setsocksfirewallproxy", s, "127.0.0.1", &socks]);
            let mut args = vec!["-setproxybypassdomains", s.as_str()];
            args.extend(skip.iter().map(String::as_str));
            ok("networksetup", &args);
        }
        return Ok(ProxyBackup { kind: "macos".into(), before: services });
    }

    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_lowercase();
    if desktop.contains("kde") {
        let tool = kde_tool().ok_or_else(unsupported)?;
        kde_set(tool, "ProxyType", "1");
        kde_set(tool, "httpProxy", &format!("http://127.0.0.1 {http}"));
        kde_set(tool, "httpsProxy", &format!("http://127.0.0.1 {http}"));
        kde_set(tool, "socksProxy", &format!("socks://127.0.0.1 {socks}"));
        kde_set(tool, "NoProxyFor", &skip.join(","));
        kde_reload();
        return Ok(ProxyBackup { kind: "kde".into(), before: vec![] });
    }
    if has("gsettings") && out("gsettings", &["get", "org.gnome.system.proxy", "mode"]).is_some() {
        let before = out("gsettings", &["get", "org.gnome.system.proxy", "mode"]).unwrap_or_default().trim_matches('\'').to_string();
        let ignore = format!("[{}]", skip.iter().map(|s| format!("'{s}'")).collect::<Vec<_>>().join(", "));
        for (schema, key, value) in [
            ("org.gnome.system.proxy.http", "host", "127.0.0.1"), ("org.gnome.system.proxy.http", "port", http.as_str()),
            ("org.gnome.system.proxy.https", "host", "127.0.0.1"), ("org.gnome.system.proxy.https", "port", http.as_str()),
            ("org.gnome.system.proxy.socks", "host", "127.0.0.1"), ("org.gnome.system.proxy.socks", "port", socks.as_str()),
            ("org.gnome.system.proxy", "ignore-hosts", ignore.as_str()), ("org.gnome.system.proxy", "mode", "manual"),
        ] {
            ok("gsettings", &["set", schema, key, value]);
        }
        return Ok(ProxyBackup { kind: "gnome".into(), before: vec![before] });
    }
    Err(unsupported())
}

pub fn restore_system_proxy(b: &ProxyBackup) -> std::io::Result<()> {
    match b.kind.as_str() {
        "macos" => {
            for s in &b.before {
                for what in ["-setwebproxystate", "-setsecurewebproxystate", "-setsocksfirewallproxystate"] {
                    ok("networksetup", &[what, s, "off"]);
                }
            }
        }
        "kde" => {
            if let Some(tool) = kde_tool() {
                kde_set(tool, "ProxyType", "0");
                kde_reload();
            }
        }
        "gnome" => {
            let mode = b.before.first().map(String::as_str).filter(|m| !m.is_empty()).unwrap_or("none");
            ok("gsettings", &["set", "org.gnome.system.proxy", "mode", mode]);
        }
        _ => {}
    }
    Ok(())
}
