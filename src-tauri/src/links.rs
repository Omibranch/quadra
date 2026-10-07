//! Share links (vless://, vmess://, trojan://, ss://) -> Xray outbound.

use crate::model::Server;
use crate::names;
use base64::Engine;
use percent_encoding::percent_decode_str;
use serde_json::{json, Map, Value};
use std::collections::HashMap;

pub fn b64(s: &str) -> Option<Vec<u8>> {
    let t: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    let t = t.trim_end_matches('=').replace('-', "+").replace('_', "/");
    base64::engine::general_purpose::STANDARD_NO_PAD.decode(t).ok()
}

fn pct(s: &str) -> String {
    percent_decode_str(s).decode_utf8_lossy().into_owned()
}

struct Url {
    user: String,
    host: String,
    port: u16,
    query: HashMap<String, String>,
    name: String,
}

fn split(rest: &str) -> Option<Url> {
    let (rest, frag) = match rest.split_once('#') {
        Some((a, b)) => (a, pct(b)),
        None => (rest, String::new()),
    };
    let (rest, query) = match rest.split_once('?') {
        Some((a, b)) => (a, b),
        None => (rest, ""),
    };
    let rest = rest.trim_end_matches('/');
    let (user, hostport) = match rest.rsplit_once('@') {
        Some((a, b)) => (pct(a), b),
        None => (String::new(), rest),
    };
    let (host, port) = if let Some(h) = hostport.strip_prefix('[') {
        let (h, p) = h.split_once(']')?;
        (h.to_string(), p.trim_start_matches(':'))
    } else {
        let (h, p) = hostport.rsplit_once(':')?;
        (h.to_string(), p)
    };
    let query = query
        .split('&')
        .filter_map(|kv| kv.split_once('='))
        .map(|(k, v)| (k.to_string(), pct(&v.replace('+', " "))))
        .collect();
    Some(Url { user, host, port: port.parse().ok()?, query, name: frag })
}

fn put(m: &mut Map<String, Value>, key: &str, v: Option<&String>) {
    if let Some(v) = v.filter(|v| !v.is_empty()) {
        m.insert(key.into(), json!(v));
    }
}

/// streamSettings from the query parameters every link type shares.
fn stream(q: &HashMap<String, String>, default_security: &str) -> (Value, String, String) {
    let net = match q.get("type").map(String::as_str).unwrap_or("tcp") {
        "raw" | "" => "tcp",
        "splithttp" | "h2" | "http" => "xhttp",
        other => other,
    }
    .to_string();
    let security = q.get("security").cloned().filter(|s| !s.is_empty()).unwrap_or(default_security.into());
    let mut s = Map::new();
    s.insert("network".into(), json!(net));
    let host = q.get("host");
    let path = q.get("path");
    match net.as_str() {
        "tcp" => {
            if q.get("headerType").map(String::as_str) == Some("http") {
                let hosts: Vec<&str> = host.map(|h| h.split(',').collect()).unwrap_or_default();
                s.insert("tcpSettings".into(), json!({"header": {"type": "http", "request": {
                    "path": [path.cloned().unwrap_or("/".into())], "headers": {"Host": hosts}}}}));
            }
        }
        "ws" => {
            let mut w = Map::new();
            put(&mut w, "path", path);
            put(&mut w, "host", host);
            s.insert("wsSettings".into(), Value::Object(w));
        }
        "grpc" => {
            let mut g = Map::new();
            put(&mut g, "serviceName", q.get("serviceName"));
            put(&mut g, "authority", q.get("authority"));
            g.insert("multiMode".into(), json!(q.get("mode").map(String::as_str) == Some("multi")));
            s.insert("grpcSettings".into(), Value::Object(g));
        }
        "xhttp" => {
            let mut x = Map::new();
            put(&mut x, "path", path);
            put(&mut x, "host", host);
            x.insert("mode".into(), json!(q.get("mode").cloned().unwrap_or("auto".into())));
            if let Some(extra) = q.get("extra").and_then(|e| serde_json::from_str::<Value>(e).ok()) {
                x.insert("extra".into(), extra);
            }
            s.insert("xhttpSettings".into(), Value::Object(x));
        }
        "httpupgrade" => {
            let mut h = Map::new();
            put(&mut h, "path", path);
            put(&mut h, "host", host);
            s.insert("httpupgradeSettings".into(), Value::Object(h));
        }
        "kcp" => {
            let mut k = Map::new();
            put(&mut k, "seed", q.get("seed"));
            k.insert("header".into(), json!({"type": q.get("headerType").cloned().unwrap_or("none".into())}));
            s.insert("kcpSettings".into(), Value::Object(k));
        }
        _ => {}
    }
    let fp = q.get("fp").cloned().filter(|f| !f.is_empty());
    match security.as_str() {
        "tls" => {
            let mut t = Map::new();
            put(&mut t, "serverName", q.get("sni").or(host));
            if let Some(fp) = &fp {
                t.insert("fingerprint".into(), json!(fp));
            }
            if let Some(alpn) = q.get("alpn").filter(|a| !a.is_empty()) {
                t.insert("alpn".into(), json!(alpn.split(',').collect::<Vec<_>>()));
            }
            let insecure = ["allowInsecure", "insecure"].iter().any(|k| matches!(q.get(*k).map(String::as_str), Some("1" | "true")));
            if insecure {
                t.insert("allowInsecure".into(), json!(true));
            }
            s.insert("security".into(), json!("tls"));
            s.insert("tlsSettings".into(), Value::Object(t));
        }
        "reality" => {
            let mut r = Map::new();
            put(&mut r, "serverName", q.get("sni"));
            r.insert("fingerprint".into(), json!(fp.unwrap_or("chrome".into())));
            put(&mut r, "publicKey", q.get("pbk"));
            r.insert("shortId".into(), json!(q.get("sid").cloned().unwrap_or_default()));
            put(&mut r, "spiderX", q.get("spx"));
            put(&mut r, "mldsa65Verify", q.get("pqv"));
            s.insert("security".into(), json!("reality"));
            s.insert("realitySettings".into(), Value::Object(r));
        }
        _ => {
            s.insert("security".into(), json!("none"));
        }
    }
    (Value::Object(s), net, security)
}

fn finish(proto: &str, u: &Url, uri: &str, outbound: Value, net: String, security: String) -> Server {
    let raw = if u.name.is_empty() { format!("{}:{}", u.host, u.port) } else { u.name.clone() };
    let name = names::clean(&raw);
    Server {
        name: if name.is_empty() { u.host.clone() } else { name },
        cc: names::flag_cc(&raw).or_else(|| names::guess_cc(&raw)).unwrap_or_default(),
        proto: proto.into(),
        transport: net,
        security,
        address: u.host.clone(),
        port: u.port,
        uri: Some(uri.to_string()),
        outbound: Some(outbound),
        ..Default::default()
    }
}

pub fn parse(line: &str) -> Option<Server> {
    let line = line.trim();
    let (scheme, rest) = line.split_once("://")?;
    match scheme.to_ascii_lowercase().as_str() {
        "vless" => {
            let u = split(rest)?;
            let (st, net, sec) = stream(&u.query, "none");
            let mut user = Map::new();
            user.insert("id".into(), json!(u.user));
            user.insert("encryption".into(), json!(u.query.get("encryption").cloned().filter(|e| !e.is_empty()).unwrap_or("none".into())));
            put(&mut user, "flow", u.query.get("flow"));
            let ob = json!({"tag": "proxy", "protocol": "vless",
                "settings": {"vnext": [{"address": u.host, "port": u.port, "users": [Value::Object(user)]}]},
                "streamSettings": st});
            Some(finish("vless", &u, line, ob, net, sec))
        }
        "trojan" => {
            let u = split(rest)?;
            let (st, net, sec) = stream(&u.query, "tls");
            let ob = json!({"tag": "proxy", "protocol": "trojan",
                "settings": {"servers": [{"address": u.host, "port": u.port, "password": u.user}]},
                "streamSettings": st});
            Some(finish("trojan", &u, line, ob, net, sec))
        }
        "ss" => {
            // Either ss://base64(method:password)@host:port or ss://base64(method:password@host:port).
            let mut u = split(rest).filter(|u| !u.user.is_empty()).or_else(|| {
                let (body, frag) = rest.split_once('#').unwrap_or((rest, ""));
                let body = body.split('?').next()?;
                let plain = String::from_utf8(b64(body)?).ok()?;
                split(&format!("{plain}#{frag}"))
            })?;
            if !u.user.contains(':') {
                u.user = String::from_utf8(b64(&u.user)?).ok()?;
            }
            let (method, password) = u.user.split_once(':')?;
            let ob = json!({"tag": "proxy", "protocol": "shadowsocks",
                "settings": {"servers": [{"address": u.host, "port": u.port, "method": method, "password": password}]}});
            Some(finish("shadowsocks", &u, line, ob, "tcp".into(), "none".into()))
        }
        "vmess" => {
            let v: Value = serde_json::from_slice(&b64(rest)?).ok()?;
            let s = |k: &str| match v.get(k) {
                Some(Value::String(x)) => x.clone(),
                Some(Value::Number(n)) => n.to_string(),
                _ => String::new(),
            };
            let mut q = HashMap::new();
            for (from, to) in [("net", "type"), ("host", "host"), ("path", "path"), ("sni", "sni"), ("alpn", "alpn"), ("fp", "fp")] {
                q.insert(to.to_string(), s(from));
            }
            if q["type"] == "grpc" {
                q.insert("serviceName".into(), s("path"));
            }
            if s("type") == "http" {
                q.insert("headerType".into(), "http".into());
            }
            q.insert("security".into(), if s("tls") == "tls" { "tls".into() } else { "none".into() });
            let u = Url { user: s("id"), host: s("add"), port: s("port").parse().ok()?, query: q, name: s("ps") };
            let (st, net, sec) = stream(&u.query, "none");
            let scy = Some(s("scy")).filter(|x| !x.is_empty()).unwrap_or("auto".into());
            let ob = json!({"tag": "proxy", "protocol": "vmess",
                "settings": {"vnext": [{"address": u.host, "port": u.port,
                    "users": [{"id": u.user, "alterId": s("aid").parse::<u32>().unwrap_or(0), "security": scy}]}]},
                "streamSettings": st});
            Some(finish("vmess", &u, line, ob, net, sec))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vless_reality() {
        let s = parse("vless://11111111-2222-3333-4444-555555555555@example.com:443?encryption=none&flow=xtls-rprx-vision&type=tcp&security=reality&sni=www.site.com&fp=firefox&pbk=PUBKEY&sid=ab12&spx=%2F#%F0%9F%87%A9%F0%9F%87%AA%20%D0%93%D0%B5%D1%80%D0%BC%D0%B0%D0%BD%D0%B8%D1%8F").unwrap();
        assert_eq!((s.name.as_str(), s.cc.as_str(), s.port), ("Германия", "DE", 443));
        let ob = s.outbound.unwrap();
        assert_eq!(ob["settings"]["vnext"][0]["users"][0]["flow"], "xtls-rprx-vision");
        assert_eq!(ob["streamSettings"]["realitySettings"]["publicKey"], "PUBKEY");
        assert_eq!(ob["streamSettings"]["realitySettings"]["spiderX"], "/");
    }

    #[test]
    fn vless_xhttp_extra_and_ws() {
        let s = parse("vless://u@[2001:db8::1]:8443?type=xhttp&security=tls&sni=a.b&path=%2Fx&host=a.b&mode=packet-up&alpn=h2%2Chttp%2F1.1&extra=%7B%22xPaddingBytes%22%3A%22100-1000%22%7D#x").unwrap();
        let st = &s.outbound.unwrap()["streamSettings"];
        assert_eq!(s.address, "2001:db8::1");
        assert_eq!(st["xhttpSettings"]["extra"]["xPaddingBytes"], "100-1000");
        assert_eq!(st["tlsSettings"]["alpn"][1], "http/1.1");
        let w = parse("vless://u@h.example:443?type=ws&security=tls&path=%2Fws&host=cdn.example#w").unwrap();
        assert_eq!(w.outbound.unwrap()["streamSettings"]["wsSettings"]["host"], "cdn.example");
    }

    #[test]
    fn shadowsocks_both_forms() {
        let a = parse("ss://Y2hhY2hhMjAtaWV0Zi1wb2x5MTMwNTpwYXNz@1.2.3.4:8388#n").unwrap();
        let b = parse("ss://Y2hhY2hhMjAtaWV0Zi1wb2x5MTMwNTpwYXNzQDEuMi4zLjQ6ODM4OA#n").unwrap();
        for s in [a, b] {
            let srv = &s.outbound.unwrap()["settings"]["servers"][0];
            assert_eq!((srv["method"].as_str(), srv["password"].as_str(), srv["port"].as_u64()), (Some("chacha20-ietf-poly1305"), Some("pass"), Some(8388)));
        }
    }
}
