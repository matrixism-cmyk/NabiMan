//! Per-session alternate credentials for a VM console, so a multi-user VM can
//! hold independent RDP sessions under different accounts. The password is kept
//! in memory for a couple of minutes against a one-use token — never stored or
//! logged — and the browser re-issues one for each (re)connect.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use actix_web::{web, HttpRequest, HttpResponse};
use rand::RngCore;

use crate::models::ApiResponse;

const TTL: Duration = Duration::from_secs(120);

struct Entry {
    target: String,
    user: String,
    password: String,
    expires: Instant,
}

fn store() -> &'static Mutex<HashMap<String, Entry>> {
    static S: OnceLock<Mutex<HashMap<String, Entry>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(HashMap::new()))
}

/// The (user, password) a console connect uses: alternate ones named by a
/// `creds` token in the query, else the given defaults; None if a token is bad.
pub(super) fn resolve(query: &str, target: &str, user: String, password: String) -> Option<(String, String)> {
    match super::parse_query_param(query, "creds") {
        Some(token) => redeem(&token, target),
        None => Some((user, password)),
    }
}

/// Take the alternate (user, password) for `target` named by a one-use token.
pub(super) fn redeem(token: &str, target: &str) -> Option<(String, String)> {
    let mut guard = store().lock().ok()?;
    guard.retain(|_, e| e.expires > Instant::now());
    let entry = guard.remove(token)?;
    if entry.target != target {
        return None;
    }
    Some((entry.user, entry.password))
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct CredsBody {
    target: String,
    user: String,
    password: String,
}

/// `POST /api/remote/console-creds` — stash alternate credentials for one
/// connect, returning a one-use token the console WebSocket redeems.
#[actix_web::post("/api/remote/console-creds")]
async fn issue(req: HttpRequest, body: web::Json<CredsBody>) -> HttpResponse {
    if !super::vnc_console::may_open(&req, &body.target) {
        return HttpResponse::Forbidden().finish();
    }
    if super::vnc_registry::rdp_login(&body.target).is_none() {
        return HttpResponse::NotFound().finish();
    }
    let mut bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut bytes);
    let token = hex::encode(bytes);
    let mut guard = store().lock().unwrap();
    guard.retain(|_, e| e.expires > Instant::now());
    guard.insert(
        token.clone(),
        Entry {
            target: body.target.clone(),
            user: body.user.clone(),
            password: body.password.clone(),
            expires: Instant::now() + TTL,
        },
    );
    HttpResponse::Ok().json(ApiResponse::ok(serde_json::json!({ "token": token })))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(issue);
}
