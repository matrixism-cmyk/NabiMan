//! Admin-registered VNC consoles — external VMs reached by IP or domain, kept
//! beside the auto-listed internal KubeVirt bridges. Each entry is just a
//! label and a host:port; NabiMan bridges its VNC over a raw TCP connection.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct Console {
    pub id: String,
    pub label: String,
    pub host: String,
    pub port: u16,
}

fn file() -> PathBuf {
    nabiman_server::data_dir().join("vnc_consoles.json")
}

pub fn load() -> Vec<Console> {
    std::fs::read(file())
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn save(list: &[Console]) -> Result<(), String> {
    let json = serde_json::to_vec_pretty(list).map_err(|e| e.to_string())?;
    crate::private_storage::write(&file(), &json).map_err(|e| format!("Cannot save consoles: {e}"))
}

pub fn find(id: &str) -> Option<Console> {
    load().into_iter().find(|c| c.id == id)
}

/// A hostname, domain or IPv4 the admin typed — no spaces or control bytes, so
/// it cannot smuggle extra connection parameters.
fn valid_host(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 253
        && host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
}

fn clean(label: &str, host: &str, port: u16) -> Result<(String, String), String> {
    let label = label.trim();
    let host = host.trim();
    if label.is_empty() || label.len() > 80 {
        return Err("콘솔 이름을 1~80자로 입력하세요".into());
    }
    if !valid_host(host) {
        return Err("호스트는 IP 또는 도메인이어야 합니다".into());
    }
    if port == 0 {
        return Err("포트가 올바르지 않습니다".into());
    }
    Ok((label.to_string(), host.to_string()))
}

pub fn create(label: &str, host: &str, port: u16) -> Result<Console, String> {
    let (label, host) = clean(label, host, port)?;
    let mut list = load();
    if list.len() >= 100 {
        return Err("콘솔은 최대 100개까지 등록할 수 있습니다".into());
    }
    let console = Console {
        id: format!("vnc_{}", uuid::Uuid::new_v4().simple()),
        label,
        host,
        port,
    };
    list.push(console.clone());
    save(&list)?;
    Ok(console)
}

pub fn update(id: &str, label: &str, host: &str, port: u16) -> Result<Console, String> {
    let (label, host) = clean(label, host, port)?;
    let mut list = load();
    let entry = list.iter_mut().find(|c| c.id == id).ok_or("콘솔을 찾을 수 없습니다")?;
    entry.label = label;
    entry.host = host;
    entry.port = port;
    let updated = entry.clone();
    save(&list)?;
    Ok(updated)
}

pub fn delete(id: &str) -> Result<(), String> {
    let mut list = load();
    let before = list.len();
    list.retain(|c| c.id != id);
    if list.len() == before {
        return Err("콘솔을 찾을 수 없습니다".into());
    }
    save(&list)
}
