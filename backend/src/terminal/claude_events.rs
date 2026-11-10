//! The messages Claude sessions sent each other, kept as events: who to
//! whom, when, and the first line. The session list shows only who leads
//! whom now; this is the record behind it, for the work map (the 3D graph's
//! flashes and its timeline) and for looking back after a restart.
//!
//! Stored in SQLite (`data_dir/claude-events.sqlite3`): it grows with every
//! message, is asked for "what happened since", and should outlive the process.

use crate::lock_safe::LockExt;
use std::collections::HashMap;
use std::os::unix::fs::PermissionsExt;
use std::sync::Mutex;

use actix_web::{web, HttpRequest, HttpResponse};
use rusqlite::{params, Connection};
use serde::Serialize;

use super::session::PtyStore;
use crate::models::ApiResponse;

/// Events older than this are dropped.
const KEEP_SECS: u64 = 14 * 24 * 60 * 60;
const MAX_ROWS: i64 = 500;

static DB: Mutex<Option<Connection>> = Mutex::new(None);

fn open(path: &std::path::Path) -> rusqlite::Result<Connection> {
    let db = Connection::open(path)?;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    // Journal mode cannot change inside the migration's transaction.
    db.execute_batch("PRAGMA journal_mode=WAL;")?;
    SCHEMA.migrate(&db)?;
    Ok(db)
}

/// Versions of `claude-events.sqlite3` (see `sqlite_schema`); append, never edit.
pub(crate) const SCHEMA: crate::sqlite_schema::Schema =
    crate::sqlite_schema::Schema { name: "claude-events.sqlite3", steps: &[schema_v1] };

fn schema_v1(db: &Connection) -> rusqlite::Result<()> {
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS claude_events(
           id INTEGER PRIMARY KEY AUTOINCREMENT,
           at INTEGER NOT NULL,
           from_card TEXT NOT NULL,
           to_card TEXT NOT NULL,
           from_name TEXT NOT NULL DEFAULT '',
           to_name TEXT NOT NULL DEFAULT '',
           text TEXT NOT NULL DEFAULT '',
           UNIQUE(at, from_card, to_card, text)
         );
         CREATE INDEX IF NOT EXISTS claude_events_at ON claude_events(at);",
    )
}

fn with_db<T>(f: impl FnOnce(&Connection) -> rusqlite::Result<T>) -> Option<T> {
    let mut slot = DB.lock_safe();
    if slot.is_none() {
        *slot = open(&nabiman_server::data_dir().join("claude-events.sqlite3"))
            .map_err(|e| crate::sqlite_schema::report_once(SCHEMA.name, &e))
            .ok();
    }
    slot.as_ref().and_then(|db| f(db).ok())
}

/// One message seen in a transcript: (from card, to card, when, first line).
pub(crate) type Seen = (String, String, u64, String);

fn store(db: &Connection, events: &[Seen], names: &HashMap<String, String>, now: u64) -> rusqlite::Result<usize> {
    let mut added = 0;
    let tx = db.unchecked_transaction()?;
    {
        let mut put = tx.prepare_cached(
            "INSERT OR IGNORE INTO claude_events(at, from_card, to_card, from_name, to_name, text) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )?;
        for (from, to, at, text) in events {
            if from == to || at + KEEP_SECS < now {
                continue;
            }
            let name = |card: &String| names.get(card).cloned().unwrap_or_default();
            added += put.execute(params![*at as i64, from, to, name(from), name(to), text])?;
        }
    }
    tx.execute("DELETE FROM claude_events WHERE at < ?1", params![now.saturating_sub(KEEP_SECS) as i64])?;
    tx.commit()?;
    Ok(added)
}

/// Records the messages a scan saw; ones already stored are skipped.
/// `names`: each card's Claude name now, kept with the event since cards come and go.
pub(crate) fn record(events: &[Seen], names: &HashMap<String, String>) {
    if events.is_empty() {
        return;
    }
    let now = super::session::now_unix();
    with_db(|db| store(db, events, names, now));
}

#[derive(Serialize, Debug, PartialEq)]
struct Event {
    id: i64,
    at: u64,
    /// The session ids now, where the sender and receiver are still tiles the caller sees.
    from: Option<String>,
    to: Option<String>,
    from_name: String,
    to_name: String,
    text: String,
}

fn load(db: &Connection, after: i64, since: u64) -> rusqlite::Result<Vec<(i64, u64, String, String, String, String, String)>> {
    let mut q = db.prepare_cached(
        "SELECT id, at, from_card, to_card, from_name, to_name, text FROM claude_events
         WHERE id > ?1 AND at >= ?2 ORDER BY id DESC LIMIT ?3",
    )?;
    let rows = q.query_map(params![after, since as i64, MAX_ROWS], |r| {
        Ok((r.get(0)?, r.get::<_, i64>(1)? as u64, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?))
    })?;
    let mut out: Vec<_> = rows.collect::<rusqlite::Result<_>>()?;
    out.reverse();
    Ok(out)
}

/// The texts of the events since `since` (unix seconds), oldest first.
#[cfg(test)]
pub(crate) fn texts_since(since: u64) -> Vec<String> {
    with_db(|db| load(db, 0, since)).unwrap_or_default().into_iter().map(|r| r.6).collect()
}

#[derive(serde::Deserialize)]
struct Query {
    /// Only events with a larger id (what the page already has).
    after: Option<i64>,
    /// Only events from the last this-many seconds (default one hour).
    window: Option<u64>,
}

/// `GET /api/terminal/claude-events`: the messages between Claude sessions, oldest first.
async fn events(query: web::Query<Query>, store: web::Data<PtyStore>, req: HttpRequest, users: web::Data<crate::users::UserStore>) -> HttpResponse {
    let user_id = crate::auth::verified_user_id(&req).unwrap_or_default();
    // Remote-only members see no orchestration of the whole server.
    let member = users.lock_safe().iter().find(|u| u.id == user_id).is_none_or(crate::member_servers::is_member);
    if user_id.is_empty() || member {
        return HttpResponse::Ok().json(ApiResponse::ok(Vec::<Event>::new()));
    }
    // Looking keeps the record fresh even when no session list is open.
    let _ = super::claude_peers::look();
    let since = super::session::now_unix().saturating_sub(query.window.unwrap_or(3600).min(KEEP_SECS));
    let rows = with_db(|db| load(db, query.after.unwrap_or(0), since)).unwrap_or_default();
    let map = store.lock_safe();
    let id_of: HashMap<&str, &str> = map
        .iter()
        .filter(|(_, s)| super::session::visible_to(s, &user_id, &users))
        .map(|(id, s)| (s.tmux_name.as_str(), id.as_str()))
        .collect();
    let out: Vec<Event> = rows
        .into_iter()
        .map(|(id, at, from, to, from_name, to_name, text)| Event {
            id,
            at,
            from: id_of.get(from.as_str()).map(|s| s.to_string()),
            to: id_of.get(to.as_str()).map(|s| s.to_string()),
            from_name,
            to_name,
            text,
        })
        .collect();
    HttpResponse::Ok().json(ApiResponse::ok(out))
}

pub(crate) fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/api/terminal/claude-events", web::get().to(events));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_are_kept_once_and_read_back_since_an_id() {
        let dir = std::env::temp_dir().join(format!("nabiman-events-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = open(&dir.join("e.sqlite3")).unwrap();
        let names = HashMap::from([("nb_a".to_string(), "lead".to_string()), ("nb_b".to_string(), "worker".to_string())]);
        let now = 2_000_000;
        let seen = vec![
            ("nb_a".to_string(), "nb_b".to_string(), now - 50, "보고서 그림 갱신".to_string()),
            ("nb_b".to_string(), "nb_a".to_string(), now - 20, "완료".to_string()),
            ("nb_a".to_string(), "nb_a".to_string(), now - 10, "self".to_string()),
            ("nb_a".to_string(), "nb_b".to_string(), now - KEEP_SECS - 5, "too old".to_string()),
        ];
        assert_eq!(store(&db, &seen, &names, now).unwrap(), 2);
        // The same scan again adds nothing.
        assert_eq!(store(&db, &seen, &names, now).unwrap(), 0);
        let all = load(&db, 0, now - 3600).unwrap();
        assert_eq!(all.iter().map(|r| (r.4.as_str(), r.5.as_str(), r.6.as_str())).collect::<Vec<_>>(), vec![("lead", "worker", "보고서 그림 갱신"), ("worker", "lead", "완료")]);
        assert_eq!(load(&db, all[0].0, now - 3600).unwrap().len(), 1);
        assert_eq!(load(&db, 0, now - 30).unwrap().len(), 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
