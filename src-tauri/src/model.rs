use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Rule {
    pub kind: String,   // "domain" | "ip"
    pub value: String,
    pub action: String, // "proxy" | "direct" | "block"
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Home {
    pub lat: f64,
    pub lon: f64,
    #[serde(default)]
    pub cc: String,
    #[serde(default)]
    pub ip: String,
    #[serde(default)]
    pub manual: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Settings {
    pub theme: String,
    pub accent: String,
    pub effects: String,
    /// "proxy" sets the system proxy, "ports" only opens the local ports, "tun" takes all traffic.
    pub mode: String,
    pub socks_port: u16,
    pub http_port: u16,
    pub allow_lan: bool,
    /// "all" | "lan" | "ru": which traffic skips the server. Applies to servers added as links.
    pub routing: String,
    pub rules: Vec<Rule>,
    /// Programs (process names) and domains that never go through the server.
    pub bypass_apps: Vec<String>,
    pub bypass_domains: Vec<String>,
    pub dns: String,
    pub close_to_tray: bool,
    pub autostart: bool,
    pub autoconnect: bool,
    pub send_hwid: bool,
    pub user_agent: String,
    pub geo_lookup: bool,
    pub home: Option<Home>,
    pub sub_update_hours: u32,
    pub log_level: String,
    pub selected: Option<String>,
    pub collapsed: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            theme: "dark".into(),
            accent: "#3ddc84".into(),
            effects: "full".into(),
            mode: "proxy".into(),
            socks_port: 20808,
            http_port: 20809,
            allow_lan: false,
            routing: "lan".into(),
            rules: vec![],
            bypass_apps: vec![],
            bypass_domains: vec![],
            dns: "1.1.1.1, 8.8.8.8".into(),
            close_to_tray: true,
            autostart: false,
            autoconnect: false,
            send_hwid: true,
            user_agent: String::new(),
            geo_lookup: true,
            home: None,
            sub_update_hours: 12,
            log_level: "warning".into(),
            selected: None,
            collapsed: vec![],
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Subscription {
    pub id: String,
    pub url: String,
    pub title: String,
    pub upload: u64,
    pub download: u64,
    pub total: u64,
    pub expire: u64,
    pub support_url: String,
    pub web_url: String,
    pub announce: String,
    pub update_hours: u32,
    pub updated_at: u64,
    pub error: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Server {
    pub id: String,
    pub sub: Option<String>,
    pub name: String,
    pub cc: String,
    pub proto: String,
    pub transport: String,
    pub security: String,
    pub address: String,
    pub port: u16,
    pub note: String,
    /// Share link the server came from, when it came from one.
    pub uri: Option<String>,
    /// Xray outbound built from the link.
    pub outbound: Option<Value>,
    /// Whole Xray config, when the provider ships ready-made configs.
    pub config: Option<Value>,
}

impl Server {
    /// What the interface needs; the configs stay on this side.
    pub fn view(&self, ping: Option<i32>) -> Value {
        json!({
            "id": self.id, "sub": self.sub, "name": self.name, "cc": self.cc, "proto": self.proto,
            "transport": self.transport, "security": self.security, "address": self.address,
            "port": self.port, "note": self.note, "full": self.config.is_some(), "ping": ping,
        })
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Data {
    pub hwid: String,
    pub settings: Settings,
    pub subs: Vec<Subscription>,
    pub servers: Vec<Server>,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct Status {
    pub state: String, // "off" | "connecting" | "on"
    pub server: Option<String>,
    pub since: u64,
    pub ms: Option<u32>,
    pub error: Option<String>,
    pub exit: Option<Value>,
    pub tun: bool,
}
