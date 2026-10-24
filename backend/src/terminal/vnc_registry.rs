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
    #[serde(default = "vnc_proto")]
    pub protocol: String, // "vnc" | "rdp"
    #[serde(default)]
    pub user: String,
    #[serde(default)]
    pub password_enc: String,
}

fn vnc_proto() -> String {
    "vnc".into()
}

/// The decrypted RDP login for a target, resolved server-side only.
pub fn rdp_login(id: &str) -> Option<(String, u16, String, String)> {
    let c = find(id)?;
    if c.protocol != "rdp" {
        return None;
    }
    let pw = if c.password_enc.is_empty() {
        String::new()
    } else {
        crate::secret_store::decrypt(&c.password_enc).ok()?
    };
    Some((c.host, c.port, c.user, pw))
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

pub fn create(label: &str, host: &str, port: u16, protocol: &str, user: &str, password: &str) -> Result<Console, String> {
    let (label, host) = clean(label, host, port)?;
    let protocol = norm_protocol(protocol)?;
    let mut list = load();
    if list.len() >= 100 {
        return Err("콘솔은 최대 100개까지 등록할 수 있습니다".into());
    }
    let console = Console {
        id: format!("vnc_{}", uuid::Uuid::new_v4().simple()),
        label,
        host,
        port,
        user: user.trim().to_string(),
        password_enc: seal(&protocol, password)?,
        protocol,
    };
    list.push(console.clone());
    save(&list)?;
    Ok(console)
}

fn norm_protocol(p: &str) -> Result<String, String> {
    match p {
        "vnc" | "rdp" => Ok(p.to_string()),
        "" => Ok("vnc".into()),
        _ => Err("알 수 없는 프로토콜입니다".into()),
    }
}

/// Encrypt a password for RDP; VNC consoles store none.
fn seal(protocol: &str, password: &str) -> Result<String, String> {
    if protocol != "rdp" || password.is_empty() {
        return Ok(String::new());
    }
    crate::secret_store::encrypt(password)
}

pub fn update(id: &str, label: &str, host: &str, port: u16, protocol: &str, user: &str, password: Option<&str>) -> Result<Console, String> {
    let (label, host) = clean(label, host, port)?;
    let protocol = norm_protocol(protocol)?;
    let mut list = load();
    let entry = list.iter_mut().find(|c| c.id == id).ok_or("콘솔을 찾을 수 없습니다")?;
    entry.label = label;
    entry.host = host;
    entry.port = port;
    entry.user = user.trim().to_string();
    // A new password re-seals; None keeps the stored one (unless switching off rdp).
    if protocol != "rdp" {
        entry.password_enc.clear();
    } else if let Some(pw) = password {
        entry.password_enc = seal(&protocol, pw)?;
    }
    entry.protocol = protocol;
    let updated = entry.clone();
    save(&list)?;
    Ok(updated)
}

/// Set (or clear) an overlay for a built-in console, keyed by its name. An
/// overlay lets a built-in VM console be renamed or switched to RDP. A plain
/// VNC overlay with no host removes the overlay (back to the default bridge).
pub fn upsert_builtin(name: &str, label: &str, protocol: &str, host: &str, port: u16, user: &str, password: Option<&str>) -> Result<Console, String> {
    let label = label.trim();
    if label.is_empty() || label.len() > 80 {
        return Err("콘솔 이름을 1~80자로 입력하세요".into());
    }
    let protocol = norm_protocol(protocol)?;
    let mut list = load();
    // A default VNC overlay (no host) is the same as no overlay: drop it.
    if protocol == "vnc" && host.trim().is_empty() {
        list.retain(|c| c.id != name);
        let out = Console { id: name.into(), label: label.into(), host: String::new(), port, protocol, user: String::new(), password_enc: String::new() };
        save(&list)?;
        return Ok(out);
    }
    if protocol == "rdp" && (host.trim().is_empty() || port == 0) {
        return Err("RDP는 호스트와 포트가 필요합니다".into());
    }
    let existing = list.iter().position(|c| c.id == name);
    let password_enc = match (password, existing.and_then(|i| list.get(i))) {
        (Some(pw), _) => seal(&protocol, pw)?,
        (None, Some(cur)) if protocol == "rdp" => cur.password_enc.clone(),
        _ => String::new(),
    };
    let console = Console { id: name.into(), label: label.into(), host: host.trim().into(), port, protocol, user: user.trim().into(), password_enc };
    match existing {
        Some(i) => list[i] = console.clone(),
        None => list.push(console.clone()),
    }
    save(&list)?;
    Ok(console)
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
