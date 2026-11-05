//! How each account likes its session tiles laid out: an order of its own,
//! set by dragging, or one of the sorts. It is kept on the server so a
//! phone and a desktop show the same order.

use std::collections::{HashMap, HashSet};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use actix_web::{web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};

use super::session::PtyStore;
use crate::models::ApiResponse;

pub(crate) const MODES: [&str; 5] = ["custom", "newest", "oldest", "name", "active"];
const MAX_IDS: usize = 1000;
const MAX_ID_LEN: usize = 128;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct Order {
    /// One of `MODES`; `custom` follows `ids`.
    pub mode: String,
    /// Session ids, first shown first. Sessions not in it come before them.
    #[serde(default)]
    pub ids: Vec<String>,
}

impl Default for Order {
    fn default() -> Self {
        Order { mode: "newest".into(), ids: Vec::new() }
    }
}

static STORE: OnceLock<Mutex<HashMap<String, Order>>> = OnceLock::new();

fn file() -> PathBuf {
    nabiman_server::data_dir().join("session_order.json")
}

fn store() -> &'static Mutex<HashMap<String, Order>> {
    STORE.get_or_init(|| {
        let saved = std::fs::read(file())
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        Mutex::new(saved)
    })
}

fn save(map: &HashMap<String, Order>) -> std::io::Result<()> {
    let path = file();
    let parent = path.parent().ok_or_else(|| std::io::Error::other("no data folder"))?;
    let temporary = parent.join(format!(".session-order-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut f = OpenOptions::new().write(true).create_new(true).mode(0o600).open(&temporary)?;
        f.write_all(&serde_json::to_vec(map)?)?;
        f.sync_all()?;
        std::fs::rename(&temporary, &path)?;
        File::open(parent)?.sync_all()
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

/// The order as asked, kept to sessions that exist, each once.
fn clean(order: Order, known: &HashSet<String>) -> Option<Order> {
    if !MODES.contains(&order.mode.as_str()) || order.ids.len() > MAX_IDS {
        return None;
    }
    let mut seen = HashSet::new();
    let ids = order
        .ids
        .into_iter()
        .filter(|id| id.len() <= MAX_ID_LEN && known.contains(id) && seen.insert(id.clone()))
        .collect();
    Some(Order { mode: order.mode, ids })
}

async fn get_order(req: HttpRequest) -> HttpResponse {
    let Some(user) = crate::auth::verified_user_id(&req).filter(|u| !u.is_empty()) else {
        return HttpResponse::Unauthorized().json(ApiResponse::<()>::error("Not signed in"));
    };
    let map = store().lock().unwrap_or_else(|e| e.into_inner());
    HttpResponse::Ok().json(ApiResponse::ok(map.get(&user).cloned().unwrap_or_default()))
}

async fn put_order(req: HttpRequest, body: web::Json<Order>, sessions: web::Data<PtyStore>) -> HttpResponse {
    let Some(user) = crate::auth::verified_user_id(&req).filter(|u| !u.is_empty()) else {
        return HttpResponse::Unauthorized().json(ApiResponse::<()>::error("Not signed in"));
    };
    let known: HashSet<String> = sessions.lock().unwrap().keys().cloned().collect();
    let Some(order) = clean(body.into_inner(), &known) else {
        return HttpResponse::Ok().json(ApiResponse::<()>::error("Invalid order"));
    };
    let mut map = store().lock().unwrap_or_else(|e| e.into_inner());
    let mut next = map.clone();
    next.insert(user, order.clone());
    match save(&next) {
        Ok(()) => {
            *map = next;
            HttpResponse::Ok().json(ApiResponse::ok(order))
        }
        Err(_) => HttpResponse::ServiceUnavailable().json(ApiResponse::<()>::error("The order could not be saved")),
    }
}

pub(crate) fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/api/terminal/session-order", web::get().to(get_order))
        .route("/api/terminal/session-order", web::put().to(put_order));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_order_keeps_only_sessions_that_exist_each_once() {
        let known: HashSet<String> = ["a", "b", "c"].iter().map(|s| s.to_string()).collect();
        let asked = Order { mode: "custom".into(), ids: vec!["c".into(), "gone".into(), "a".into(), "c".into()] };
        assert_eq!(clean(asked, &known).unwrap().ids, vec!["c", "a"]);
        assert!(clean(Order { mode: "shuffle".into(), ids: vec![] }, &known).is_none());
        assert!(clean(Order { mode: "custom".into(), ids: vec!["a".into(); MAX_IDS + 1] }, &known).is_none());
    }

    #[test]
    fn nothing_saved_means_newest_first() {
        assert_eq!(Order::default().mode, "newest");
    }
}
