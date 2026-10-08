//! Turns a server plus the settings into the config Xray is started with.

use crate::model::{Server, Settings};
use crate::sys;
use serde_json::{json, Map, Value};

pub const TUN_TAG: &str = "quadra-tun";

const PRIVATE_NETWORKS: &[&str] = &[
    "10.0.0.0/8", "172.16.0.0/12", "192.168.0.0/16", "127.0.0.0/8", "169.254.0.0/16", "100.64.0.0/10",
    "fc00::/7", "fe80::/10", "::1/128",
];

fn listen(s: &Settings) -> &'static str {
    if s.allow_lan { "0.0.0.0" } else { "127.0.0.1" }
}

/// At the default level the per-connection access lines would drown everything else.
fn log_settings(s: &Settings) -> Value {
    if s.log_level == "warning" {
        json!({"loglevel": "warning", "access": "none"})
    } else {
        json!({"loglevel": s.log_level})
    }
}

fn sniffing() -> Value {
    json!({"enabled": true, "destOverride": ["http", "tls", "quic"]})
}

fn tun_inbound(s: &Settings) -> Value {
    let dns: Vec<String> = dns_servers(s).into_iter().filter(|d| d.parse::<std::net::IpAddr>().is_ok()).collect();
    let mut tun = json!({"tag": TUN_TAG, "protocol": "tun", "sniffing": sniffing(), "settings": {
        "mtu": 1500,
        "gateway": ["172.19.0.1/30", "fdfe:dcba:9876::1/126"],
        "dns": if dns.is_empty() { vec!["1.1.1.1".to_string(), "8.8.8.8".to_string()] } else { dns },
        "autoSystemRoutingTable": ["0.0.0.0/0", "::/0"],
        "autoOutboundsInterface": "auto"}});
    if let Some(name) = sys::TUN_NAME {
        tun["settings"]["name"] = json!(name);
    }
    tun
}

fn dns_servers(s: &Settings) -> Vec<String> {
    let list: Vec<String> = s.dns.split([',', ' ', '\n']).map(str::trim).filter(|d| !d.is_empty()).map(String::from).collect();
    if list.is_empty() { vec!["1.1.1.1".into(), "8.8.8.8".into()] } else { list }
}

/// The user's own rules. `tags` says which outbound each action maps to in this config.
fn user_rules(s: &Settings, proxy: &str, direct: &str, block: &str) -> Vec<Value> {
    let mut out = vec![];
    for r in &s.rules {
        let value = r.value.trim();
        if value.is_empty() {
            continue;
        }
        let tag = match r.action.as_str() {
            "direct" => direct,
            "block" => block,
            _ => proxy,
        };
        let rule = if r.kind == "ip" {
            json!({"type": "field", "ip": [value], "outboundTag": tag})
        } else {
            let v = if value.contains(':') { value.to_string() } else { format!("domain:{value}") };
            json!({"type": "field", "domain": [v], "outboundTag": tag})
        };
        out.push(rule);
    }
    out
}

fn cleaned(list: &[String]) -> Vec<String> {
    list.iter().map(|v| v.trim().to_string()).filter(|v| !v.is_empty()).collect()
}

/// The exclusions, as rules that go before everything else and rules that go after everything.
///
/// "exclude": the listed programs and domains go straight out. "only": the listed ones go
/// through the server and a last rule sends the rest straight out. On Android the system VPN
/// itself decides per app, so programs are not turned into rules there.
fn bypass_rules(s: &Settings, proxy: &str, direct: &str) -> (Vec<Value>, Vec<Value>) {
    let apps = if cfg!(target_os = "android") { vec![] } else { cleaned(&s.bypass_apps) };
    let domains: Vec<String> = cleaned(&s.bypass_domains).into_iter()
        .map(|d| if d.contains(':') { d } else { format!("domain:{}", d.trim_start_matches("*.")) }).collect();
    let only = s.bypass_mode == "only";
    // "only" with an empty list would cut everything off; an empty list means the mode is not in use.
    if only && apps.is_empty() && domains.is_empty() {
        return (vec![], vec![]);
    }
    // On Android the chosen apps are all the VPN carries. Narrowing them further to the listed
    // domains would leave those very apps without the server, so there the apps decide alone.
    if only && cfg!(target_os = "android") && !cleaned(&s.bypass_apps).is_empty() {
        return (vec![], vec![]);
    }
    let target = if only { proxy } else { direct };
    let mut first = vec![];
    if !apps.is_empty() {
        first.push(json!({"type": "field", "process": apps, "outboundTag": target}));
    }
    if !domains.is_empty() {
        first.push(json!({"type": "field", "domain": domains, "outboundTag": target}));
    }
    let last = if only { vec![json!({"type": "field", "network": "tcp,udp", "outboundTag": direct})] } else { vec![] };
    (first, last)
}

fn add_counters(cfg: &mut Map<String, Value>, metrics_port: u16) {
    cfg.insert("stats".into(), json!({}));
    cfg.insert("metrics".into(), json!({"tag": "quadra-metrics", "listen": format!("127.0.0.1:{metrics_port}")}));
    let policy = cfg.entry("policy").or_insert_with(|| json!({}));
    if !policy.is_object() {
        *policy = json!({});
    }
    let system = policy.as_object_mut().unwrap().entry("system").or_insert_with(|| json!({}));
    if !system.is_object() {
        *system = json!({});
    }
    for k in ["statsInboundUplink", "statsInboundDownlink", "statsOutboundUplink", "statsOutboundDownlink"] {
        system.as_object_mut().unwrap().insert(k.into(), json!(true));
    }
}

fn from_link(outbound: &Value, s: &Settings, tun: bool) -> Map<String, Value> {
    let mut rules = vec![];
    if tun {
        rules.push(json!({"type": "field", "inboundTag": [TUN_TAG], "port": 53, "outboundTag": "dns-out"}));
    }
    let (bypass_first, bypass_last) = bypass_rules(s, "proxy", "direct");
    rules.extend(bypass_first);
    rules.extend(user_rules(s, "proxy", "direct", "block"));
    if s.routing != "all" {
        // spelled out rather than "geoip:private": the core then has no 16 MB file to open at start
        rules.push(json!({"type": "field", "ip": PRIVATE_NETWORKS, "outboundTag": "direct"}));
    }
    if s.routing == "ru" {
        rules.push(json!({"type": "field", "domain": ["geosite:category-ru", "domain:ru", "domain:su", "domain:xn--p1ai"], "outboundTag": "direct"}));
        rules.push(json!({"type": "field", "ip": ["geoip:ru"], "outboundTag": "direct"}));
    }
    rules.extend(bypass_last);
    let mut proxy = outbound.clone();
    proxy["tag"] = json!("proxy");
    let mut inbounds = vec![
        json!({"tag": "socks", "listen": listen(s), "port": s.socks_port, "protocol": "socks",
               "settings": {"auth": "noauth", "udp": true}, "sniffing": sniffing()}),
        json!({"tag": "http", "listen": listen(s), "port": s.http_port, "protocol": "http", "sniffing": sniffing()}),
    ];
    if tun {
        inbounds.push(tun_inbound(s));
    }
    let cfg = json!({
        "log": log_settings(s),
        "dns": {"servers": dns_servers(s)},
        "inbounds": inbounds,
        "outbounds": [proxy, {"tag": "direct", "protocol": "freedom"}, {"tag": "block", "protocol": "blackhole"},
                      {"tag": "dns-out", "protocol": "dns"}],
        "routing": {"domainStrategy": if s.routing == "ru" { "IPIfNonMatch" } else { "AsIs" }, "rules": rules},
    });
    cfg.as_object().unwrap().clone()
}

/// A provider's ready-made config keeps its own outbounds, balancers and routing; only the
/// local side (ports, logging, counters) and the user's rules are ours.
fn from_full(config: &Value, s: &Settings, tun: bool) -> Result<Map<String, Value>, String> {
    let mut cfg = config.as_object().cloned().ok_or("конфигурация сервера повреждена")?;
    cfg.remove("remarks");
    cfg.remove("meta");
    cfg.remove("api");
    let log = cfg.entry("log").or_insert_with(|| json!({}));
    *log = log_settings(s);

    let mut inbounds: Vec<Value> = cfg.get("inbounds").and_then(Value::as_array).cloned().unwrap_or_default();
    inbounds.retain(|i| i["protocol"] != "tun");
    let (mut has_socks, mut has_http) = (false, false);
    for i in inbounds.iter_mut() {
        let port = match i["protocol"].as_str() {
            Some("socks") | Some("mixed") if !has_socks => { has_socks = true; s.socks_port }
            Some("http") if !has_http => { has_http = true; s.http_port }
            _ => continue,
        };
        i["port"] = json!(port);
        i["listen"] = json!(listen(s));
    }
    if !has_socks {
        inbounds.push(json!({"tag": "quadra-socks", "listen": listen(s), "port": s.socks_port, "protocol": "socks",
                             "settings": {"auth": "noauth", "udp": true}, "sniffing": sniffing()}));
    }
    if !has_http {
        inbounds.push(json!({"tag": "quadra-http", "listen": listen(s), "port": s.http_port, "protocol": "http", "sniffing": sniffing()}));
    }
    if tun {
        inbounds.push(tun_inbound(s));
    }
    for (n, i) in inbounds.iter_mut().enumerate() {
        if i["tag"].as_str().unwrap_or("").is_empty() {
            i["tag"] = json!(format!("quadra-in-{n}"));
        }
    }
    // Providers write their rules for traffic that enters through the local socks/http ports
    // ("inboundTag": ["socks", "http"] -> proxy). Traffic from our TUN adapter carries another
    // tag, would match none of them and fall through to the first outbound, usually "direct":
    // connected, yet nothing goes through the server. So the TUN joins those rules.
    if tun {
        let local: Vec<String> = inbounds.iter()
            .filter(|i| matches!(i["protocol"].as_str(), Some("socks" | "mixed" | "http")))
            .filter_map(|i| i["tag"].as_str().map(String::from)).collect();
        if let Some(rules) = cfg.get_mut("routing").and_then(|r| r.get_mut("rules")).and_then(Value::as_array_mut) {
            for rule in rules.iter_mut() {
                if let Some(tags) = rule.get_mut("inboundTag").and_then(Value::as_array_mut) {
                    if tags.iter().any(|t| t.as_str().is_some_and(|t| local.iter().any(|l| l == t))) {
                        tags.push(json!(TUN_TAG));
                    }
                }
            }
        }
    }
    cfg.insert("inbounds".into(), json!(inbounds));

    if !s.rules.is_empty() || !cleaned(&s.bypass_apps).is_empty() || !cleaned(&s.bypass_domains).is_empty() {
        let mut outbounds: Vec<Value> = cfg.get("outbounds").and_then(Value::as_array).cloned().unwrap_or_default();
        let tag_of = |obs: &Vec<Value>, protos: &[&str]| {
            obs.iter().find(|o| protos.contains(&o["protocol"].as_str().unwrap_or(""))).and_then(|o| o["tag"].as_str()).map(String::from)
        };
        let proxy = outbounds.iter()
            .find(|o| !matches!(o["protocol"].as_str().unwrap_or(""), "freedom" | "blackhole" | "dns" | "loopback"))
            .and_then(|o| o["tag"].as_str()).unwrap_or("proxy").to_string();
        let direct = tag_of(&outbounds, &["freedom"]).unwrap_or_else(|| {
            outbounds.push(json!({"tag": "quadra-direct", "protocol": "freedom"}));
            "quadra-direct".into()
        });
        let block = tag_of(&outbounds, &["blackhole"]).unwrap_or_else(|| {
            outbounds.push(json!({"tag": "quadra-block", "protocol": "blackhole"}));
            "quadra-block".into()
        });
        let routing = cfg.entry("routing").or_insert_with(|| json!({}));
        let (mut rules, bypass_last) = bypass_rules(s, &proxy, &direct);
        rules.extend(user_rules(s, &proxy, &direct, &block));
        // in "only" mode the rest must not reach the provider's own rules, which would proxy it
        rules.extend(bypass_last);
        rules.extend(routing["rules"].as_array().cloned().unwrap_or_default());
        routing["rules"] = json!(rules);
        cfg.insert("outbounds".into(), json!(outbounds));
    }
    Ok(cfg)
}

pub fn build(server: &Server, s: &Settings, tun: bool, metrics_port: u16) -> Result<Value, String> {
    let mut cfg = match (&server.config, &server.outbound) {
        (Some(c), _) => from_full(c, s, tun)?,
        (None, Some(o)) => from_link(o, s, tun),
        _ => return Err("у сервера нет конфигурации".into()),
    };
    add_counters(&mut cfg, metrics_port);
    Ok(Value::Object(cfg))
}

/// Where a ready-made config actually connects: its first real outbound.
pub fn describe_full(config: &Value) -> (String, String, String, String, u16) {
    let empty = vec![];
    let obs = config["outbounds"].as_array().unwrap_or(&empty);
    let Some(o) = obs.iter().find(|o| !matches!(o["protocol"].as_str().unwrap_or(""), "freedom" | "blackhole" | "dns" | "loopback")) else {
        return Default::default();
    };
    let st = &o["settings"];
    let first = if st["vnext"].is_array() { &st["vnext"][0] } else if st["servers"].is_array() { &st["servers"][0] } else { st };
    (
        o["protocol"].as_str().unwrap_or("").to_string(),
        o["streamSettings"]["network"].as_str().unwrap_or("tcp").to_string(),
        o["streamSettings"]["security"].as_str().unwrap_or("none").to_string(),
        first["address"].as_str().unwrap_or("").to_string(),
        first["port"].as_u64().unwrap_or(0) as u16,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Rule;

    #[test]
    fn tun_traffic_follows_the_rules_written_for_the_local_ports() {
        let provider = json!({
            "inbounds": [{"tag": "socks", "port": 10808, "protocol": "socks"}, {"tag": "http", "port": 10809, "protocol": "http"}],
            "outbounds": [{"tag": "direct", "protocol": "freedom"}, {"tag": "proxy", "protocol": "vless"}],
            "routing": {"rules": [
                {"type": "field", "ip": ["geoip:private"], "outboundTag": "direct"},
                {"type": "field", "inboundTag": ["socks", "http"], "balancerTag": "lb_single"}]}});
        let server = Server { config: Some(provider), ..Default::default() };
        let plain = Settings { bypass_apps: vec![], bypass_domains: vec![], ..Default::default() };
        let with_tun = build(&server, &plain, true, 4000).unwrap();
        assert_eq!(with_tun["routing"]["rules"][1]["inboundTag"], json!(["socks", "http", TUN_TAG]));
        assert!(with_tun["routing"]["rules"][0].get("inboundTag").is_none());
        let without = build(&server, &plain, false, 4000).unwrap();
        assert_eq!(without["routing"]["rules"][1]["inboundTag"], json!(["socks", "http"]));
    }

    #[test]
    fn only_mode_sends_the_listed_through_the_server_and_the_rest_past_it() {
        let s = Settings { bypass_mode: "only".into(), bypass_apps: vec!["Steam.exe".into()], bypass_domains: vec!["example.org".into()], ..Default::default() };
        let link = Server { outbound: Some(json!({"protocol": "vless"})), ..Default::default() };
        let cfg = build(&link, &s, false, 4000).unwrap();
        let rules = cfg["routing"]["rules"].as_array().unwrap();
        assert_eq!(rules[0], json!({"type": "field", "process": ["Steam.exe"], "outboundTag": "proxy"}));
        assert_eq!(rules[1]["outboundTag"], "proxy");
        assert_eq!(rules.last().unwrap(), &json!({"type": "field", "network": "tcp,udp", "outboundTag": "direct"}));
        // nothing listed: the mode is simply not in use
        let empty = Settings { bypass_mode: "only".into(), ..Default::default() };
        let cfg = build(&link, &empty, false, 4000).unwrap();
        assert!(cfg["routing"]["rules"].as_array().unwrap().iter().all(|r| r.get("network").is_none()));
        // a provider's config: its own rules stay, but after our catch-all
        let full = Server { config: Some(json!({"outbounds": [{"tag": "px", "protocol": "vless"}, {"tag": "direct", "protocol": "freedom"}],
            "routing": {"rules": [{"type": "field", "inboundTag": ["socks"], "balancerTag": "lb"}]}})), ..Default::default() };
        let cfg = build(&full, &s, false, 4000).unwrap();
        let rules = cfg["routing"]["rules"].as_array().unwrap();
        assert_eq!(rules[0]["outboundTag"], "px");
        assert_eq!(rules[2], json!({"type": "field", "network": "tcp,udp", "outboundTag": "direct"}));
        assert_eq!(rules[3]["balancerTag"], "lb");
    }

    #[test]
    fn exclusions_come_first_and_go_direct() {
        let s = Settings { bypass_apps: vec!["Steam.exe".into(), " ".into()], bypass_domains: vec!["*.example.org".into(), "geosite:google".into()], ..Default::default() };
        let link = Server { outbound: Some(json!({"protocol": "vless"})), ..Default::default() };
        let cfg = build(&link, &s, false, 4000).unwrap();
        assert_eq!(cfg["routing"]["rules"][0], json!({"type": "field", "process": ["Steam.exe"], "outboundTag": "direct"}));
        assert_eq!(cfg["routing"]["rules"][1]["domain"], json!(["domain:example.org", "geosite:google"]));
        let full = Server { config: Some(json!({"outbounds": [{"tag": "px", "protocol": "vless"}], "routing": {"rules": [{"type": "field", "port": "25", "outboundTag": "px"}]}})), ..Default::default() };
        let cfg = build(&full, &s, false, 4000).unwrap();
        assert_eq!(cfg["routing"]["rules"][0]["outboundTag"], "quadra-direct");
        assert_eq!(cfg["routing"]["rules"][2]["port"], "25");
        assert!(cfg["outbounds"].as_array().unwrap().iter().any(|o| o["tag"] == "quadra-direct"));
    }

    #[test]
    fn full_config_keeps_provider_routing_and_gets_our_ports() {
        let provider = json!({
            "remarks": "x", "inbounds": [
                {"tag": "socks", "port": 10808, "listen": "127.0.0.1", "protocol": "socks"},
                {"tag": "http", "port": 10809, "listen": "127.0.0.1", "protocol": "http"},
                {"tag": "entry-quic", "port": 10802, "protocol": "dokodemo-door"}],
            "outbounds": [{"tag": "direct", "protocol": "freedom"}, {"tag": "eu1", "protocol": "vless"}],
            "routing": {"rules": [{"type": "field", "inboundTag": ["entry-quic"], "balancerTag": "lb"}]},
            "policy": {"levels": {"8": {"connIdle": 300}}}});
        let mut s = Settings { bypass_apps: vec![], bypass_domains: vec![], ..Default::default() };
        s.rules.push(Rule { kind: "domain".into(), value: "example.org".into(), action: "direct".into() });
        let server = Server { config: Some(provider), ..Default::default() };
        let cfg = build(&server, &s, true, 4000).unwrap();
        assert_eq!(cfg["inbounds"][0]["port"], 20808);
        assert_eq!(cfg["inbounds"][1]["port"], 20809);
        assert_eq!(cfg["inbounds"][2]["port"], 10802);
        assert_eq!(cfg["inbounds"][3]["protocol"], "tun");
        assert_eq!(cfg["routing"]["rules"][0]["domain"][0], "domain:example.org");
        assert_eq!(cfg["routing"]["rules"][0]["outboundTag"], "direct");
        assert_eq!(cfg["routing"]["rules"][1]["balancerTag"], "lb");
        assert_eq!(cfg["routing"]["rules"][1]["inboundTag"], json!(["entry-quic"]));
        assert_eq!(cfg["policy"]["levels"]["8"]["connIdle"], 300);
        assert_eq!(cfg["policy"]["system"]["statsInboundDownlink"], true);
        assert!(cfg.get("remarks").is_none());
    }
}
