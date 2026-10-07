//! Country of an IPv4 address, read from the geoip.dat that ships with Xray. Used only for
//! servers whose name carries no flag.

use std::net::Ipv4Addr;
use std::path::Path;
use std::sync::OnceLock;

static TABLE: OnceLock<Vec<(u32, u32, [u8; 2])>> = OnceLock::new();

fn varint(buf: &[u8], pos: &mut usize) -> Option<u64> {
    let mut v = 0u64;
    for shift in (0..64).step_by(7) {
        let b = *buf.get(*pos)?;
        *pos += 1;
        v |= ((b & 0x7f) as u64) << shift;
        if b & 0x80 == 0 {
            return Some(v);
        }
    }
    None
}

/// Calls `f(field, bytes)` for every length-delimited field, `g(field, value)` for every varint one.
fn fields<'a>(buf: &'a [u8], mut f: impl FnMut(u64, &'a [u8]), mut g: impl FnMut(u64, u64)) {
    let mut pos = 0;
    while pos < buf.len() {
        let Some(key) = varint(buf, &mut pos) else { return };
        match key & 7 {
            0 => match varint(buf, &mut pos) {
                Some(v) => g(key >> 3, v),
                None => return,
            },
            2 => {
                let Some(len) = varint(buf, &mut pos) else { return };
                let end = pos + len as usize;
                if end > buf.len() {
                    return;
                }
                f(key >> 3, &buf[pos..end]);
                pos = end;
            }
            1 => pos += 8,
            5 => pos += 4,
            _ => return,
        }
    }
}

fn load(path: &Path) -> Vec<(u32, u32, [u8; 2])> {
    let Ok(data) = std::fs::read(path) else { return vec![] };
    let mut table = vec![];
    fields(&data, |_, entry| {
        let mut code = [0u8; 2];
        let mut cidrs: Vec<&[u8]> = vec![];
        fields(entry, |field, bytes| match field {
            1 if bytes.len() == 2 => code = [bytes[0].to_ascii_uppercase(), bytes[1].to_ascii_uppercase()],
            2 => cidrs.push(bytes),
            _ => {}
        }, |_, _| {});
        if code == [0, 0] {
            return; // "private", "cloudflare" and the like are not countries
        }
        for cidr in cidrs {
            let (mut ip, mut prefix) = (None, 32u64);
            fields(cidr, |field, bytes| if field == 1 && bytes.len() == 4 {
                ip = Some(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]));
            }, |field, v| if field == 2 { prefix = v });
            if let Some(ip) = ip {
                let size = if prefix == 0 { u32::MAX } else { (1u64 << (32 - prefix.min(32))) as u32 - 1 };
                table.push((ip, ip.saturating_add(size), code));
            }
        }
    }, |_, _| {});
    table.sort_unstable_by_key(|r| r.0);
    table
}

pub fn country(dat: &Path, ip: Ipv4Addr) -> Option<String> {
    let table = TABLE.get_or_init(|| load(dat));
    let ip = u32::from(ip);
    let idx = table.partition_point(|r| r.0 <= ip);
    table[..idx].iter().rev().take(8).find(|r| r.1 >= ip).map(|r| String::from_utf8_lossy(&r.2).into_owned())
}
