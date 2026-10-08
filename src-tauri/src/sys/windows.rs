//! Windows: the few Win32 calls the app needs, declared by hand.

use std::ffi::{c_void, OsStr};
use std::os::windows::ffi::OsStrExt;
use winreg::enums::*;
use winreg::RegKey;

#[repr(C)]
struct BasicLimits {
    per_process_user_time: i64,
    per_job_user_time: i64,
    limit_flags: u32,
    min_working_set: usize,
    max_working_set: usize,
    active_process_limit: u32,
    affinity: usize,
    priority_class: u32,
    scheduling_class: u32,
}

#[repr(C)]
struct ExtendedLimits {
    basic: BasicLimits,
    io: [u64; 6],
    process_memory_limit: usize,
    job_memory_limit: usize,
    peak_process_memory: usize,
    peak_job_memory: usize,
}

#[link(name = "kernel32")]
extern "system" {
    fn CreateJobObjectW(attrs: *const c_void, name: *const u16) -> *mut c_void;
    fn SetInformationJobObject(job: *mut c_void, class: i32, info: *const c_void, len: u32) -> i32;
    fn AssignProcessToJobObject(job: *mut c_void, process: *mut c_void) -> i32;
}

#[link(name = "wininet")]
extern "system" {
    fn InternetSetOptionW(handle: *const c_void, option: u32, buffer: *const c_void, len: u32) -> i32;
}

#[link(name = "shell32")]
extern "system" {
    fn ShellExecuteW(hwnd: *const c_void, op: *const u16, file: *const u16, params: *const u16, dir: *const u16, show: i32) -> isize;
    fn IsUserAnAdmin() -> i32;
}

#[link(name = "user32")]
extern "system" {
    fn OpenClipboard(owner: *const c_void) -> i32;
    fn CloseClipboard() -> i32;
    fn GetClipboardData(format: u32) -> *mut c_void;
}

#[link(name = "kernel32")]
extern "system" {
    fn GlobalLock(mem: *mut c_void) -> *const u16;
    fn GlobalUnlock(mem: *mut c_void) -> i32;
}

/// Text currently on the clipboard, read natively so the webview never asks for permission.
pub fn clipboard_text() -> String {
    unsafe {
        if OpenClipboard(std::ptr::null()) == 0 {
            return String::new();
        }
        let mut text = String::new();
        let mem = GetClipboardData(13); // CF_UNICODETEXT
        if !mem.is_null() {
            let ptr = GlobalLock(mem);
            if !ptr.is_null() {
                let mut len = 0;
                while *ptr.add(len) != 0 && len < 4_000_000 {
                    len += 1;
                }
                text = String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len));
                GlobalUnlock(mem);
            }
        }
        CloseClipboard();
        text
    }
}

fn wide(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(Some(0)).collect()
}

pub const PLATFORM: &str = "windows";
pub const XRAY_BIN: &str = "xray.exe";
pub const TUN_NAME: Option<&str> = Some("quadra");

/// A job that kills everything in it when this process goes away, so the core never outlives the app.
fn job() -> usize {
    static JOB: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *JOB.get_or_init(|| unsafe {
        let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        let mut info: ExtendedLimits = std::mem::zeroed();
        info.basic.limit_flags = 0x2000; // JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
        SetInformationJobObject(job, 9, &info as *const _ as *const c_void, std::mem::size_of::<ExtendedLimits>() as u32);
        job as usize
    })
}

/// How the core is started on this platform: without a console window.
pub fn prepare(cmd: &mut tokio::process::Command) {
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
}

/// Ties a started core to the life of the app.
pub fn adopt(child: &tokio::process::Child) {
    if let Some(handle) = child.raw_handle() {
        unsafe {
            AssignProcessToJobObject(job() as *mut c_void, handle as *mut c_void);
        }
    }
}

/// The all-traffic mode needs the whole app elevated here; elsewhere only the core is.
pub fn needs_relaunch_for_tun() -> bool {
    !is_admin()
}

/// Unix starts the core for the all-traffic mode through a privileged wrapper; Windows does not.
pub fn tun_wrapper(_xray: &std::path::Path, _assets: &std::path::Path, _config: &std::path::Path, _run: &std::path::Path) -> Option<tokio::process::Command> {
    None
}

pub fn release_tun(_run: &std::path::Path) {}

pub fn tiling_wm() -> bool {
    false
}

pub fn os_name() -> &'static str {
    "Windows"
}

pub fn device_name() -> String {
    std::env::var("COMPUTERNAME").unwrap_or_else(|_| "PC".into())
}

pub fn is_admin() -> bool {
    unsafe { IsUserAnAdmin() != 0 }
}

fn shell(op: &str, file: &str, params: &str) -> bool {
    let (op, file, params) = (wide(op), wide(file), wide(params));
    unsafe { ShellExecuteW(std::ptr::null(), op.as_ptr(), file.as_ptr(), params.as_ptr(), std::ptr::null(), 1) > 32 }
}

pub fn open_url(url: &str) -> bool {
    let ok = ["https://", "http://", "tg://"].iter().any(|p| url.starts_with(p));
    ok && shell("open", url, "")
}

/// Starts this program again with administrator rights (shows the UAC prompt).
pub fn relaunch_elevated(args: &str) -> bool {
    std::env::current_exe().ok().and_then(|p| p.to_str().map(|p| shell("runas", p, args))).unwrap_or(false)
}

#[link(name = "iphlpapi")]
extern "system" {
    fn GetAdaptersAddresses(family: u32, flags: u32, reserved: *const c_void, addresses: *mut u64, size: *mut u32) -> u32;
}

unsafe fn wide_at(ptr: *const u16) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let mut len = 0;
    while *ptr.add(len) != 0 && len < 512 {
        len += 1;
    }
    String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len))
}

/// Whether an adapter is some VPN's tunnel. Windows has tunnel-ish adapters of its own (Teredo,
/// 6to4, ISATAP: interface type 131) that are always there and carry no VPN; those do not count.
fn is_vpn_tunnel(description: &str, if_type: u32) -> bool {
    const IF_TYPE_PROP_VIRTUAL: u32 = 53; // what Wintun adapters report
    const IF_TYPE_TUNNEL: u32 = 131;
    let d = description.to_lowercase();
    if if_type == IF_TYPE_TUNNEL {
        return false;
    }
    ["tap-windows", "wireguard", "openvpn", "wintun"].iter().any(|k| d.contains(k))
        || (if_type == IF_TYPE_PROP_VIRTUAL && d.contains("tun"))
}

/// Name of another VPN's tunnel adapter that is up right now, if there is one. Two tunnels
/// that both claim the default route starve each other, so the all-traffic mode refuses to
/// start next to one. Asked from the system directly: this runs before every connection and
/// has to cost nothing.
pub fn other_tunnel() -> Option<String> {
    // Fields of IP_ADAPTER_ADDRESSES on 64-bit Windows, by offset: Next 8, Description 64,
    // FriendlyName 72, IfType 100, OperStatus 104 (1 = up).
    const NEXT: usize = 8;
    const DESCRIPTION: usize = 64;
    const FRIENDLY_NAME: usize = 72;
    const IF_TYPE: usize = 100;
    const OPER_STATUS: usize = 104;
    if std::mem::size_of::<usize>() != 8 {
        return None;
    }
    unsafe {
        let mut size: u32 = 32 * 1024;
        let mut buffer: Vec<u64> = Vec::new();
        let mut ok = false;
        for _ in 0..3 {
            buffer.resize(size as usize / 8 + 1, 0);
            // skip unicast, anycast, multicast and DNS lists: only the adapters themselves
            match GetAdaptersAddresses(0, 0x1 | 0x2 | 0x4 | 0x8, std::ptr::null(), buffer.as_mut_ptr(), &mut size) {
                0 => {
                    ok = true;
                    break;
                }
                111 => continue, // ERROR_BUFFER_OVERFLOW: size now holds what is needed
                _ => return None,
            }
        }
        if !ok {
            return None;
        }
        let mut adapter = buffer.as_ptr() as *const u8;
        while !adapter.is_null() {
            let description = wide_at(*(adapter.add(DESCRIPTION) as *const *const u16));
            let name = wide_at(*(adapter.add(FRIENDLY_NAME) as *const *const u16));
            let if_type = *(adapter.add(IF_TYPE) as *const u32);
            let up = *(adapter.add(OPER_STATUS) as *const i32) == 1;
            if up && name != "quadra" && is_vpn_tunnel(&description, if_type) {
                return Some(name);
            }
            adapter = *(adapter.add(NEXT) as *const *const u8);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::is_vpn_tunnel;

    #[test]
    fn windows_own_tunnel_adapters_are_not_a_vpn() {
        for (description, if_type) in [("Teredo Tunneling Pseudo-Interface", 131), ("Microsoft 6to4 Adapter", 131),
            ("Microsoft ISATAP Adapter", 131), ("VirtualBox Host-Only Ethernet Adapter", 6), ("Realtek PCIe GbE Family Controller", 6),
            ("Hyper-V Virtual Ethernet Adapter", 6), ("Software Loopback Interface 1", 24)] {
            assert!(!is_vpn_tunnel(description, if_type), "{description}");
        }
        for (description, if_type) in [("sing-tun Tunnel", 53), ("WireGuard Tunnel", 53), ("Wintun Userspace Tunnel", 53),
            ("TAP-Windows Adapter V9", 6), ("OpenVPN Data Channel Offload", 53), ("Xray Tunnel", 53)] {
            assert!(is_vpn_tunnel(description, if_type), "{description}");
        }
    }
}

/// Programs the user could want to exclude: what is in the Start menu and what is running with
/// a window right now. The routing rule matches by executable name, so that is the id.
pub fn list_apps() -> Vec<(String, String)> {
    use std::os::windows::process::CommandExt;
    // WScript.Shell is the simplest thing that can read where a shortcut points
    let script = r#"
$sh = New-Object -ComObject WScript.Shell
$found = @{}
$win = $env:windir
$dirs = @([Environment]::GetFolderPath('CommonPrograms'), [Environment]::GetFolderPath('Programs'))
Get-ChildItem $dirs -Recurse -Filter *.lnk -ErrorAction SilentlyContinue | ForEach-Object {
  try { $l = $sh.CreateShortcut($_.FullName); $t = $l.TargetPath; $a = $l.Arguments } catch { $t = ''; $a = '' }
  # parts of Windows itself, and the icon stubs that installer-made shortcuts point at
  if ($t -notlike '*.exe' -or $t -like "$win\*" -or $_.BaseName -match 'uninstall|удал|readme|license|help|manual|documentation') { return }
  $exe = [IO.Path]::GetFileName($t)
  # an updater that starts the real program: the program is named in its arguments
  if ($exe -ieq 'Update.exe') {
    if ($a -match '--processStart\s+"?([^"]+?\.exe)') { $exe = $Matches[1] } else { return }
  }
  $k = $exe.ToLower()
  # several shortcuts to one program: the shortest name is the program's own
  if (-not $found.ContainsKey($k) -or $found[$k][1].Length -gt $_.BaseName.Length) { $found[$k] = @($exe, $_.BaseName) }
}
Get-Process | Where-Object { $_.MainWindowTitle -and $_.Path -and $_.Path -notlike "$win\*" } | ForEach-Object {
  $exe = [IO.Path]::GetFileName($_.Path)
  $k = $exe.ToLower()
  if (-not $found.ContainsKey($k)) {
    $name = $null
    try { $name = $_.MainModule.FileVersionInfo.FileDescription } catch { }
    if (-not $name) { $name = $_.ProcessName }
    $found[$k] = @($exe, $name)
  }
}
$found.Values | ForEach-Object { "$($_[0])`t$($_[1])" }
"#;
    let out = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &format!("[Console]::OutputEncoding = [Text.Encoding]::UTF8; {script}")])
        .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
        .output();
    let Ok(out) = out else { return vec![] };
    let mut apps: Vec<(String, String)> = String::from_utf8_lossy(&out.stdout).lines()
        .filter_map(|l| l.trim().split_once('\t').map(|(exe, name)| (exe.trim().to_string(), name.trim().to_string())))
        .filter(|(exe, _)| !exe.is_empty())
        .collect();
    apps.sort_by_key(|(_, name)| name.to_lowercase());
    apps
}

pub fn machine_guid() -> String {
    RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey_with_flags(r"SOFTWARE\Microsoft\Cryptography", KEY_READ | KEY_WOW64_64KEY)
        .and_then(|k| k.get_value::<String, _>("MachineGuid"))
        .unwrap_or_default()
}

pub fn os_version() -> String {
    let key = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion");
    let build: u32 = key.ok().and_then(|k| k.get_value::<String, _>("CurrentBuild").ok()).and_then(|b| b.parse().ok()).unwrap_or(0);
    format!("{}.{}", if build >= 22000 { 11 } else { 10 }, build)
}

const INTERNET_SETTINGS: &str = r"Software\Microsoft\Windows\CurrentVersion\Internet Settings";

#[derive(serde::Serialize, serde::Deserialize, Default)]
pub struct ProxyBackup {
    enable: u32,
    server: Option<String>,
    bypass: Option<String>,
}

fn notify_proxy_change() {
    unsafe {
        InternetSetOptionW(std::ptr::null(), 39, std::ptr::null(), 0); // INTERNET_OPTION_SETTINGS_CHANGED
        InternetSetOptionW(std::ptr::null(), 37, std::ptr::null(), 0); // INTERNET_OPTION_REFRESH
    }
}

/// Points the system proxy at the local HTTP port and returns what was there before.
pub fn set_system_proxy(http_port: u16, _socks_port: u16, bypass_domains: &[String]) -> std::io::Result<ProxyBackup> {
    let (key, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(INTERNET_SETTINGS)?;
    let backup = ProxyBackup {
        enable: key.get_value("ProxyEnable").unwrap_or(0),
        server: key.get_value("ProxyServer").ok(),
        bypass: key.get_value("ProxyOverride").ok(),
    };
    key.set_value("ProxyEnable", &1u32)?;
    key.set_value("ProxyServer", &format!("127.0.0.1:{http_port}"))?;
    let mut skip = String::from("localhost;127.*;10.*;172.16.*;172.17.*;172.18.*;172.19.*;172.2*;172.30.*;172.31.*;192.168.*");
    for d in bypass_domains.iter().map(|d| d.trim().trim_start_matches("*.")).filter(|d| !d.is_empty() && !d.contains(':')) {
        skip.push_str(&format!(";{d};*.{d}"));
    }
    skip.push_str(";<local>");
    key.set_value("ProxyOverride", &skip)?;
    notify_proxy_change();
    Ok(backup)
}

pub fn restore_system_proxy(b: &ProxyBackup) -> std::io::Result<()> {
    let (key, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(INTERNET_SETTINGS)?;
    key.set_value("ProxyEnable", &b.enable)?;
    match &b.server {
        Some(s) => key.set_value("ProxyServer", s)?,
        None => { let _ = key.delete_value("ProxyServer"); }
    }
    match &b.bypass {
        Some(s) => key.set_value("ProxyOverride", s)?,
        None => { let _ = key.delete_value("ProxyOverride"); }
    }
    notify_proxy_change();
    Ok(())
}
