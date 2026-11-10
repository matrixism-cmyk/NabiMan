//! What each saved server said about itself when it was last checked: its
//! hostname, OS, uptime, CPU, memory, disk and load. A check used to show
//! these once and keep only online/offline; they are kept (SQLite,
//! `data_dir/server-facts.sqlite3`) so the work map can show each machine's
//! state without checking it again.

use crate::lock_safe::LockExt;
use std::os::unix::fs::PermissionsExt;
use std::sync::Mutex;

use actix_web::{web, HttpRequest, HttpResponse};
use rusqlite::{params, Connection};
use serde::Serialize;

use crate::models::{ApiResponse, RemoteServerStatus};

static DB: Mutex<Option<Connection>> = Mutex::new(None);

fn open(path: &std::path::Path) -> rusqlite::Result<Connection> {
    let db = Connection::open(path)?;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    SCHEMA.migrate(&db)?;
    Ok(db)
}

/// Versions of `server-facts.sqlite3` (see `sqlite_schema`); append, never edit.
pub(crate) const SCHEMA: crate::sqlite_schema::Schema =
    crate::sqlite_schema::Schema { name: "server-facts.sqlite3", steps: &[schema_v1] };

fn schema_v1(db: &Connection) -> rusqlite::Result<()> {
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS server_facts(
           id TEXT PRIMARY KEY, status TEXT NOT NULL, hostname TEXT NOT NULL DEFAULT '', os TEXT NOT NULL DEFAULT '',
           uptime TEXT NOT NULL DEFAULT '', cpu TEXT NOT NULL DEFAULT '', memory TEXT NOT NULL DEFAULT '',
           disk TEXT NOT NULL DEFAULT '', load TEXT NOT NULL DEFAULT '', checked_at TEXT NOT NULL DEFAULT '');",
    )
}

fn with_db<T>(f: impl FnOnce(&Connection) -> rusqlite::Result<T>) -> Option<T> {
    let mut slot = DB.lock_safe();
    if slot.is_none() {
        *slot = open(&nabiman_server::data_dir().join("server-facts.sqlite3"))
            .map_err(|e| crate::sqlite_schema::report_once(SCHEMA.name, &e))
            .ok();
    }
    slot.as_ref().and_then(|db| f(db).ok())
}

#[derive(Serialize, Debug, PartialEq)]
pub(crate) struct Facts {
    pub id: String,
    pub status: String,
    pub hostname: String,
    pub os: String,
    pub uptime: String,
    pub cpu: String,
    pub memory: String,
    pub disk: String,
    pub load: String,
    pub checked_at: String,
}

fn clip(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).take(160).collect::<String>().trim().to_string()
}

fn store(db: &Connection, results: &[RemoteServerStatus]) -> rusqlite::Result<()> {
    for r in results {
        // An offline check measured nothing: keep what was last known, mark the state.
        if r.status != "online" {
            db.execute(
                "INSERT INTO server_facts(id,status,checked_at) VALUES (?1,?2,?3) ON CONFLICT(id) DO UPDATE SET status=?2, checked_at=?3",
                params![r.id, clip(&r.status), clip(&r.checked_at)],
            )?;
            continue;
        }
        db.execute(
            "INSERT OR REPLACE INTO server_facts(id,status,hostname,os,uptime,cpu,memory,disk,load,checked_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            params![r.id, clip(&r.status), clip(&r.hostname), clip(&r.os), clip(&r.uptime), clip(&r.cpu_usage), clip(&r.memory), clip(&r.disk), clip(&r.load), clip(&r.checked_at)],
        )?;
    }
    Ok(())
}

/// Keeps what a round of checks found.
pub(crate) fn record(results: &[RemoteServerStatus]) {
    with_db(|db| store(db, results));
}

fn load(db: &Connection) -> rusqlite::Result<Vec<Facts>> {
    db.prepare("SELECT id,status,hostname,os,uptime,cpu,memory,disk,load,checked_at FROM server_facts ORDER BY id")?
        .query_map([], |r| {
            Ok(Facts { id: r.get(0)?, status: r.get(1)?, hostname: r.get(2)?, os: r.get(3)?, uptime: r.get(4)?, cpu: r.get(5)?, memory: r.get(6)?, disk: r.get(7)?, load: r.get(8)?, checked_at: r.get(9)? })
        })?
        .collect()
}

/// `GET /api/remote-servers/facts`: what each server last reported; none for remote-only members.
pub(crate) async fn facts(req: HttpRequest, users: web::Data<crate::users::UserStore>) -> HttpResponse {
    let user_id = crate::auth::verified_user_id(&req).unwrap_or_default();
    let member = users.lock_safe().iter().find(|u| u.id == user_id).is_none_or(crate::member_servers::is_member);
    if user_id.is_empty() || member {
        return HttpResponse::Ok().json(ApiResponse::ok(Vec::<Facts>::new()));
    }
    HttpResponse::Ok().json(ApiResponse::ok(with_db(load).unwrap_or_default()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status(id: &str, state: &str, cpu: &str) -> RemoteServerStatus {
        RemoteServerStatus { id: id.into(), name: id.into(), host: "h".into(), status: state.into(), hostname: "box".into(), os: "Ubuntu 24.04".into(), uptime: "up 3 days".into(), cpu_usage: cpu.into(), memory: "4/32G".into(), disk: "40%".into(), load: "0.1".into(), checked_at: "2026-11-10T08:00:00Z".into(), checks: Vec::new() }
    }

    #[test]
    fn a_check_is_kept_and_an_offline_one_keeps_what_was_known() {
        let dir = std::env::temp_dir().join(format!("nabiman-facts-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = open(&dir.join("f.sqlite3")).unwrap();
        store(&db, &[status("a", "online", "12%"), status("b", "online", "3%")]).unwrap();
        let mut off = status("a", "offline", "");
        off.os.clear();
        off.checked_at = "2026-11-10T09:00:00Z".into();
        store(&db, &[off]).unwrap();
        let all = load(&db).unwrap();
        assert_eq!((all[0].status.as_str(), all[0].os.as_str(), all[0].cpu.as_str(), all[0].checked_at.as_str()), ("offline", "Ubuntu 24.04", "12%", "2026-11-10T09:00:00Z"));
        assert_eq!((all[1].status.as_str(), all[1].cpu.as_str()), ("online", "3%"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
