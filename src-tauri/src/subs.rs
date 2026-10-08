//! Downloading a subscription and turning it into servers.
//!
//! Providers with a device limit (Remnawave and others) want to know which device is asking:
//! the request carries `x-hwid` plus a few describing headers, and without them the panel
//! answers 404 or hands back a placeholder list.

use crate::model::{Server, Subscription};
use crate::{config, links, names, sys};
use reqwest::header::HeaderMap;
use serde_json::Value;
use sha2::{Digest, Sha256};

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn short_hash(s: &str) -> String {
    hex(&Sha256::digest(s.as_bytes())[..6])
}

/// Stable per-machine id in UUID form, the shape panels accept (10-64 chars of [a-zA-Z0-9=-]).
pub fn make_hwid() -> String {
    let mut seed = sys::machine_guid();
    if seed.is_empty() {
        // no machine id to derive from: a random one, kept in the settings from then on
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        seed = format!("{now}-{}-{:p}", std::process::id(), &now);
    }
    let h = hex(&Sha256::digest(format!("quadra_hwid_{seed}").as_bytes())).to_uppercase();
    format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}

pub fn default_user_agent() -> String {
    format!("Quadra/{}", env!("CARGO_PKG_VERSION"))
}

fn header(h: &HeaderMap, name: &str) -> String {
    let raw = h.get(name).map(|v| String::from_utf8_lossy(v.as_bytes()).into_owned()).unwrap_or_default();
    match raw.strip_prefix("base64:") {
        Some(b) => links::b64(b).map(|d| String::from_utf8_lossy(&d).into_owned()).unwrap_or_default(),
        None => raw,
    }
}

fn is_true(h: &HeaderMap, name: &str) -> bool {
    header(h, name).trim().eq_ignore_ascii_case("true")
}

pub struct Fetched {
    pub sub: Subscription,
    pub servers: Vec<Server>,
    pub skipped: usize,
}

/// Servers out of a subscription body or pasted text: Xray JSON, a base64 list, or plain links.
pub fn parse_body(text: &str) -> (Vec<Server>, usize) {
    let text = text.trim().trim_start_matches('\u{feff}');
    if text.starts_with('[') || text.starts_with('{') {
        if let Ok(v) = serde_json::from_str::<Value>(text) {
            let list = match v {
                Value::Array(a) => a,
                other => vec![other],
            };
            let servers: Vec<Server> = list.into_iter().filter(|c| c["outbounds"].is_array()).map(|c| {
                let raw = c["remarks"].as_str().unwrap_or("").to_string();
                let (proto, transport, security, address, port) = config::describe_full(&c);
                let name = names::clean(&raw);
                Server {
                    name: if name.is_empty() { address.clone() } else { name },
                    cc: names::flag_cc(&raw).or_else(|| names::guess_cc(&raw)).unwrap_or_default(),
                    note: names::clean(c["meta"]["serverDescription"].as_str().unwrap_or("")),
                    proto, transport, security, address, port,
                    config: Some(c),
                    ..Default::default()
                }
            }).collect();
            return (servers, 0);
        }
    }
    let decoded = if text.contains("://") { None } else { links::b64(text).and_then(|d| String::from_utf8(d).ok()) };
    let body = decoded.as_deref().unwrap_or(text);
    let (mut servers, mut skipped) = (vec![], 0);
    for line in body.lines().map(str::trim).filter(|l| !l.is_empty()) {
        match links::parse(line) {
            Some(s) => servers.push(s),
            None if line.contains("://") => skipped += 1,
            None => {}
        }
    }
    (servers, skipped)
}

/// Gives every server of a subscription an id that survives refreshes.
pub fn assign_ids(servers: &mut [Server], sub: Option<&str>) {
    let mut seen = std::collections::HashMap::<String, usize>::new();
    for s in servers.iter_mut() {
        let base = format!("{}|{}|{}|{}", sub.unwrap_or("manual"), s.name, s.address, s.port);
        let n = seen.entry(base.clone()).or_insert(0);
        *n += 1;
        s.id = short_hash(&format!("{base}|{n}"));
        s.sub = sub.map(String::from);
    }
}

pub async fn fetch(url: &str, id: &str, hwid: &str, send_hwid: bool, user_agent: &str) -> Result<Fetched, String> {
    let ua = if user_agent.trim().is_empty() { default_user_agent() } else { user_agent.trim().to_string() };
    let client = reqwest::Client::builder()
        .user_agent(ua)
        .timeout(std::time::Duration::from_secs(25))
        .build()
        .map_err(|e| e.to_string())?;
    let mut req = client.get(url);
    if send_hwid {
        req = req.header("x-hwid", hwid).header("x-device-os", sys::os_name()).header("x-ver-os", sys::os_version()).header("x-device-model", sys::device_name());
    }
    let resp = req.send().await.map_err(|e| {
        if e.is_timeout() { "провайдер не ответил за 25 секунд".to_string() }
        else if e.is_connect() { "не удалось соединиться с провайдером".to_string() }
        else { format!("ошибка запроса: {e}") }
    })?;
    let status = resp.status();
    let h = resp.headers().clone();
    if is_true(&h, "x-hwid-max-devices-reached") || is_true(&h, "x-hwid-limit") {
        return Err("у провайдера занят лимит устройств: удали старое устройство в личном кабинете".into());
    }
    if is_true(&h, "x-hwid-not-supported") || (status.as_u16() == 404 && !send_hwid) {
        return Err("провайдер требует идентификатор устройства: включи его отправку в настройках".into());
    }
    if !status.is_success() {
        return Err(match status.as_u16() {
            404 => "подписка не найдена: проверь ссылку".into(),
            401 | 403 => "провайдер отказал в доступе".into(),
            code => format!("провайдер ответил ошибкой {code}"),
        });
    }
    let body = resp.text().await.map_err(|e| format!("не удалось прочитать ответ: {e}"))?;
    let (mut servers, skipped) = parse_body(&body);
    if servers.is_empty() {
        return Err(if skipped > 0 { "в подписке нет серверов поддерживаемых типов".into() } else { "в ответе провайдера нет серверов".into() });
    }
    assign_ids(&mut servers, Some(id));

    let mut sub = Subscription { id: id.into(), url: url.into(), ..Default::default() };
    sub.title = names::clean(&header(&h, "profile-title"));
    if sub.title.is_empty() {
        sub.title = url.split("://").nth(1).and_then(|r| r.split('/').next()).unwrap_or("Подписка").to_string();
    }
    for part in header(&h, "subscription-userinfo").split(';') {
        if let Some((k, v)) = part.trim().split_once('=') {
            let v: u64 = v.trim().parse().unwrap_or(0);
            match k.trim() {
                "upload" => sub.upload = v,
                "download" => sub.download = v,
                "total" => sub.total = v,
                "expire" => sub.expire = v,
                _ => {}
            }
        }
    }
    sub.support_url = header(&h, "support-url");
    sub.web_url = header(&h, "profile-web-page-url");
    sub.announce = names::clean(&header(&h, "announce"));
    sub.update_hours = header(&h, "profile-update-interval").trim().parse().unwrap_or(0);
    sub.updated_at = crate::now();
    Ok(Fetched { sub, servers, skipped })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hwid_has_the_shape_panels_accept() {
        let h = make_hwid();
        assert_eq!(h.len(), 36);
        assert!(h.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'));
    }

    #[test]
    fn body_formats() {
        let link = "vless://u@h.example:443?type=tcp&security=none#a";
        let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, format!("{link}\nhy2://x@y:1#b\n"));
        let (servers, skipped) = parse_body(&b64);
        assert_eq!((servers.len(), skipped), (1, 1));
        let (servers, _) = parse_body(r#"[{"remarks":"🇩🇪 Германия","outbounds":[{"protocol":"freedom"},{"protocol":"vless","settings":{"vnext":[{"address":"1.2.3.4","port":443}]},"streamSettings":{"network":"grpc","security":"reality"}}]}]"#);
        assert_eq!((servers[0].name.as_str(), servers[0].cc.as_str(), servers[0].address.as_str(), servers[0].transport.as_str()), ("Германия", "DE", "1.2.3.4", "grpc"));
        let mut two = parse_body(&format!("{link}\n{link}")).0;
        assign_ids(&mut two, Some("s"));
        assert_ne!(two[0].id, two[1].id);
    }
}
