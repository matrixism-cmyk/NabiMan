//! The network around the servers, for the work map: devices (firewall,
//! switches, cluster nodes, VIPs…), the links between them, and which device
//! each saved server sits behind. Entered by an administrator (or imported
//! from the operations records); NabiMan does not discover it.
//!
//! Kept in SQLite (`data_dir/topology.sqlite3`), replaced as one document.

use std::os::unix::fs::PermissionsExt;
use std::sync::Mutex;

use actix_web::{web, HttpRequest, HttpResponse};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::models::ApiResponse;

const KINDS: [&str; 10] = ["firewall", "router", "switch", "node", "vip", "gateway", "dongle", "internet", "storage", "other"];
const MAX_DEVICES: usize = 200;
const MAX_LINKS: usize = 600;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub(crate) struct Device {
    pub key: String,
    pub name: String,
    pub kind: String,
    #[serde(default)]
    pub address: String,
    #[serde(default)]
    pub note: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub(crate) struct Link {
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub label: String,
}

/// A saved server (by its host) and the device it sits behind.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub(crate) struct HostTie {
    pub device: String,
    pub server_host: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub(crate) struct Topology {
    #[serde(default)]
    pub devices: Vec<Device>,
    #[serde(default)]
    pub links: Vec<Link>,
    #[serde(default)]
    pub hosts: Vec<HostTie>,
}

fn clean(s: &str, n: usize) -> String {
    s.chars().filter(|c| !c.is_control()).take(n).collect::<String>().trim().to_string()
}

fn key_ok(k: &str) -> bool {
    !k.is_empty() && k.len() <= 64 && k.chars().all(|c| c.is_ascii_alphanumeric() || "-_.:".contains(c))
}

/// The document as it will be stored, or why it cannot be.
pub(crate) fn checked(doc: Topology) -> Result<Topology, String> {
    if doc.devices.len() > MAX_DEVICES || doc.links.len() > MAX_LINKS || doc.hosts.len() > MAX_DEVICES {
        return Err("too many devices or links".into());
    }
    let mut out = Topology::default();
    for d in doc.devices {
        if !key_ok(&d.key) {
            return Err(format!("bad device key: {}", clean(&d.key, 40)));
        }
        if out.devices.iter().any(|x| x.key == d.key) {
            return Err(format!("device key used twice: {}", d.key));
        }
        let kind = if KINDS.contains(&d.kind.as_str()) { d.kind } else { "other".to_string() };
        out.devices.push(Device { key: d.key, name: clean(&d.name, 80), kind, address: clean(&d.address, 80), note: clean(&d.note, 200) });
    }
    let known = |k: &str| out.devices.iter().any(|d| d.key == k);
    for l in doc.links {
        if !known(&l.from) || !known(&l.to) || l.from == l.to {
            return Err(format!("link between unknown devices: {} → {}", clean(&l.from, 40), clean(&l.to, 40)));
        }
        if !out.links.iter().any(|x| x.from == l.from && x.to == l.to) {
            out.links.push(Link { from: l.from, to: l.to, label: clean(&l.label, 120) });
        }
    }
    for h in doc.hosts {
        if !known(&h.device) {
            return Err(format!("host tied to an unknown device: {}", clean(&h.device, 40)));
        }
        let host = clean(&h.server_host, 253);
        if !host.is_empty() && !out.hosts.iter().any(|x| x.server_host == host) {
            out.hosts.push(HostTie { device: h.device, server_host: host });
        }
    }
    Ok(out)
}

static DB: Mutex<Option<Connection>> = Mutex::new(None);

fn open(path: &std::path::Path) -> rusqlite::Result<Connection> {
    let db = Connection::open(path)?;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS topo_devices(key TEXT PRIMARY KEY, name TEXT NOT NULL, kind TEXT NOT NULL, address TEXT NOT NULL DEFAULT '', note TEXT NOT NULL DEFAULT '', pos INTEGER NOT NULL DEFAULT 0);
         CREATE TABLE IF NOT EXISTS topo_links(from_key TEXT NOT NULL, to_key TEXT NOT NULL, label TEXT NOT NULL DEFAULT '', pos INTEGER NOT NULL DEFAULT 0, PRIMARY KEY(from_key, to_key));
         CREATE TABLE IF NOT EXISTS topo_hosts(server_host TEXT PRIMARY KEY, device_key TEXT NOT NULL);",
    )?;
    Ok(db)
}

fn with_db<T>(f: impl FnOnce(&mut Connection) -> rusqlite::Result<T>) -> Option<T> {
    let mut slot = DB.lock().unwrap_or_else(|e| e.into_inner());
    if slot.is_none() {
        *slot = open(&nabiman_server::data_dir().join("topology.sqlite3")).ok();
    }
    slot.as_mut().and_then(|db| f(db).ok())
}

fn save(db: &mut Connection, doc: &Topology) -> rusqlite::Result<()> {
    let tx = db.transaction()?;
    tx.execute_batch("DELETE FROM topo_devices; DELETE FROM topo_links; DELETE FROM topo_hosts;")?;
    for (i, d) in doc.devices.iter().enumerate() {
        tx.execute("INSERT INTO topo_devices(key,name,kind,address,note,pos) VALUES (?1,?2,?3,?4,?5,?6)", params![d.key, d.name, d.kind, d.address, d.note, i as i64])?;
    }
    for (i, l) in doc.links.iter().enumerate() {
        tx.execute("INSERT INTO topo_links(from_key,to_key,label,pos) VALUES (?1,?2,?3,?4)", params![l.from, l.to, l.label, i as i64])?;
    }
    for h in &doc.hosts {
        tx.execute("INSERT INTO topo_hosts(server_host,device_key) VALUES (?1,?2)", params![h.server_host, h.device])?;
    }
    tx.commit()
}

fn load(db: &mut Connection) -> rusqlite::Result<Topology> {
    let devices = db
        .prepare("SELECT key,name,kind,address,note FROM topo_devices ORDER BY pos")?
        .query_map([], |r| Ok(Device { key: r.get(0)?, name: r.get(1)?, kind: r.get(2)?, address: r.get(3)?, note: r.get(4)? }))?
        .collect::<rusqlite::Result<_>>()?;
    let links = db
        .prepare("SELECT from_key,to_key,label FROM topo_links ORDER BY pos")?
        .query_map([], |r| Ok(Link { from: r.get(0)?, to: r.get(1)?, label: r.get(2)? }))?
        .collect::<rusqlite::Result<_>>()?;
    let hosts = db
        .prepare("SELECT device_key,server_host FROM topo_hosts ORDER BY server_host")?
        .query_map([], |r| Ok(HostTie { device: r.get(0)?, server_host: r.get(1)? }))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(Topology { devices, links, hosts })
}

fn member(req: &HttpRequest, users: &crate::users::UserStore) -> bool {
    let user_id = crate::auth::verified_user_id(req).unwrap_or_default();
    user_id.is_empty() || users.lock().unwrap().iter().find(|u| u.id == user_id).is_none_or(crate::member_servers::is_member)
}

/// `GET /api/terminal/topology`: the network as entered; empty for remote-only members.
async fn get(req: HttpRequest, users: web::Data<crate::users::UserStore>) -> HttpResponse {
    if member(&req, &users) {
        return HttpResponse::Ok().json(ApiResponse::ok(Topology::default()));
    }
    HttpResponse::Ok().json(ApiResponse::ok(with_db(load).unwrap_or_default()))
}

/// `PUT /api/terminal/topology`: a super administrator replaces the whole document.
async fn put(body: web::Json<Topology>, req: HttpRequest, users: web::Data<crate::users::UserStore>) -> HttpResponse {
    let actor = match crate::membership::actor(&req, &users) {
        Ok(a) => a,
        Err(r) => return r,
    };
    if !actor.super_admin() {
        return HttpResponse::Forbidden().json(ApiResponse::<()>::error("Only a super administrator edits the topology"));
    }
    let doc = match checked(body.into_inner()) {
        Ok(d) => d,
        Err(e) => return HttpResponse::Ok().json(ApiResponse::<()>::error(&e)),
    };
    match with_db(|db| save(db, &doc)) {
        Some(()) => HttpResponse::Ok().json(ApiResponse::ok(doc)),
        None => HttpResponse::InternalServerError().json(ApiResponse::<()>::error("could not save the topology")),
    }
}

pub(crate) fn configure(cfg: &mut web::ServiceConfig) {
    cfg.app_data(web::JsonConfig::default().limit(512 * 1024))
        .route("/api/terminal/topology", web::get().to(get))
        .route("/api/terminal/topology", web::put().to(put));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev(key: &str, kind: &str) -> Device {
        Device { key: key.into(), name: key.to_uppercase(), kind: kind.into(), address: "10.0.0.1".into(), note: String::new() }
    }

    #[test]
    fn a_document_is_checked_stored_and_read_back_in_order() {
        let doc = Topology {
            devices: vec![dev("inet", "internet"), dev("fw", "firewall"), dev("n1", "weird")],
            links: vec![Link { from: "inet".into(), to: "fw".into(), label: "공인 /28".into() }, Link { from: "fw".into(), to: "n1".into(), label: String::new() }, Link { from: "inet".into(), to: "fw".into(), label: "dup".into() }],
            hosts: vec![HostTie { device: "n1".into(), server_host: "172.20.26.159".into() }],
        };
        let ok = checked(doc).unwrap();
        assert_eq!(ok.devices[2].kind, "other", "an unknown kind is kept as other");
        assert_eq!(ok.links.len(), 2, "the same link twice is one");
        let dir = std::env::temp_dir().join(format!("nabiman-topo-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut db = open(&dir.join("t.sqlite3")).unwrap();
        save(&mut db, &ok).unwrap();
        assert_eq!(load(&mut db).unwrap(), ok);
        // Saving again replaces it whole.
        save(&mut db, &Topology::default()).unwrap();
        assert_eq!(load(&mut db).unwrap(), Topology::default());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn broken_documents_are_refused() {
        assert!(checked(Topology { devices: vec![dev("a b", "node")], ..Default::default() }).is_err());
        assert!(checked(Topology { devices: vec![dev("a", "node"), dev("a", "node")], ..Default::default() }).is_err());
        assert!(checked(Topology { devices: vec![dev("a", "node")], links: vec![Link { from: "a".into(), to: "nope".into(), label: String::new() }], ..Default::default() }).is_err());
        assert!(checked(Topology { devices: vec![dev("a", "node")], hosts: vec![HostTie { device: "x".into(), server_host: "h".into() }], ..Default::default() }).is_err());
    }
}
