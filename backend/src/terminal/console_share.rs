//! Share links for VM consoles. An administrator makes a short-lived link for a
//! specific console, choosing view-only or control; anyone with the link opens
//! that console with no NabiMan account. View-only is enforced here, not in the
//! page: a view-only share's input never reaches the VM.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use actix_web::{web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};

use crate::models::ApiResponse;

const MAX_TTL_SECS: u64 = 30 * 24 * 60 * 60;
/// Last second of year 9999 — a link that lasts until revoked.
const PERMANENT_UNIX: u64 = 253_402_300_799;

#[derive(Clone, Serialize, Deserialize)]
pub struct Share {
    pub token: String,
    pub console: String, // the console key (built-in name or registered id)
    pub mode: String,    // "view" | "control"
    pub owner_id: String,
    pub created_unix: u64,
    pub expires_unix: u64,
    #[serde(default)]
    pub password_hash: Option<String>,
}

/// Who keeps a console connection alive: a signed-in account's access, or a
/// still-valid share link.
pub(super) enum Lifeline {
    Account(crate::stream_access::ConnectionGrant),
    Share(String),
}

impl Lifeline {
    pub(super) fn valid(&self) -> bool {
        match self {
            Lifeline::Account(grant) => grant.valid(),
            Lifeline::Share(token) => resolve(token).is_some(),
        }
    }
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn file() -> PathBuf {
    nabiman_server::data_dir().join("console_shares.json")
}

pub fn load() -> Vec<Share> {
    std::fs::read(file())
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn save(list: &[Share]) -> Result<(), String> {
    let json = serde_json::to_vec_pretty(list).map_err(|e| e.to_string())?;
    crate::private_storage::write(&file(), &json).map_err(|e| format!("Cannot save console shares: {e}"))
}

/// A live (unexpired) share for the token.
pub fn resolve(token: &str) -> Option<Share> {
    load().into_iter().find(|s| s.token == token && s.expires_unix > now())
}

fn random_token() -> Option<String> {
    use std::io::Read;
    let mut buf = [0u8; 32];
    std::fs::File::open("/dev/urandom").ok()?.read_exact(&mut buf).ok()?;
    Some(buf.iter().map(|b| format!("{b:02x}")).collect())
}

pub fn create(console: &str, mode: &str, ttl_secs: u64, password: Option<&str>, owner_id: &str) -> Result<Share, String> {
    let mode = match mode {
        "view" | "control" => mode,
        _ => return Err("보기 전용 또는 조작 허용 중에서 선택하세요".into()),
    };
    // ttl == 0 means "until revoked".
    let expires = if ttl_secs == 0 { PERMANENT_UNIX } else { now() + ttl_secs.clamp(60, MAX_TTL_SECS) };
    let password_hash = match password.map(str::trim).filter(|p| !p.is_empty()) {
        Some(pw) if pw.len() > 72 => return Err("비밀번호는 72바이트 이하여야 합니다".into()),
        Some(pw) => Some(bcrypt::hash(pw, 10).map_err(|_| "Cannot hash password")?),
        None => None,
    };
    let mut list = load();
    list.retain(|s| s.expires_unix > now());
    if list.len() >= 200 {
        return Err("공유 링크가 너무 많습니다".into());
    }
    let share = Share {
        token: random_token().ok_or("Cannot make a share token")?,
        console: console.to_string(),
        mode: mode.to_string(),
        owner_id: owner_id.to_string(),
        created_unix: now(),
        expires_unix: expires,
        password_hash,
    };
    list.push(share.clone());
    save(&list)?;
    Ok(share)
}

pub fn revoke(token: &str) -> Result<(), String> {
    let mut list = load();
    let before = list.len();
    list.retain(|s| s.token != token);
    if list.len() == before {
        return Err("공유를 찾을 수 없습니다".into());
    }
    save(&list)
}

/// Drop every share of a console (used when the console itself is deleted).
pub fn forget_console(console: &str) {
    let mut list = load();
    if list.iter().any(|s| s.console == console) {
        list.retain(|s| s.console != console);
        let _ = save(&list);
    }
}

/// Whoever may open a console may share it (its owner, an admin, or a delegate).
fn may_share(req: &HttpRequest, console: &str) -> bool {
    super::vnc_console::may_open(req, console)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateBody {
    mode: String,
    hours: u64,
    #[serde(default)]
    password: Option<String>,
}

/// `POST /api/remote/consoles/{key}/shares` — make a share link (admin).
#[actix_web::post("/api/remote/consoles/{key}/shares")]
async fn create_share(req: HttpRequest, key: web::Path<String>, body: web::Json<CreateBody>) -> HttpResponse {
    if !may_share(&req, &key) {
        return HttpResponse::Forbidden().finish();
    }
    if super::vnc_console::console_exists(&key) {
        let owner = crate::auth::verified_user_id(&req).unwrap_or_default();
        match create(&key, &body.mode, body.hours.saturating_mul(3600), body.password.as_deref(), &owner) {
            Ok(s) => HttpResponse::Ok().json(ApiResponse::ok(serde_json::json!({
                "token": s.token, "mode": s.mode, "expires_unix": s.expires_unix
            }))),
            Err(e) => HttpResponse::BadRequest().json(ApiResponse::<()>::error(&e)),
        }
    } else {
        HttpResponse::NotFound().finish()
    }
}

/// `GET /api/remote/consoles/{key}/shares` — a console's live links (admin).
#[actix_web::get("/api/remote/consoles/{key}/shares")]
async fn list_shares(req: HttpRequest, key: web::Path<String>) -> HttpResponse {
    if !may_share(&req, &key) {
        return HttpResponse::Forbidden().finish();
    }
    let rows: Vec<_> = load()
        .into_iter()
        .filter(|s| s.console == *key && s.expires_unix > now())
        .map(|s| serde_json::json!({ "token": s.token, "mode": s.mode, "expires_unix": s.expires_unix, "needs_password": s.password_hash.is_some() }))
        .collect();
    HttpResponse::Ok().json(ApiResponse::ok(rows))
}

/// `DELETE /api/remote/consoles/{key}/shares/{token}` — revoke (admin).
#[actix_web::delete("/api/remote/consoles/{key}/shares/{token}")]
async fn revoke_share(req: HttpRequest, path: web::Path<(String, String)>) -> HttpResponse {
    if !may_share(&req, &path.0) {
        return HttpResponse::Forbidden().finish();
    }
    match revoke(&path.1) {
        Ok(()) => HttpResponse::Ok().json(ApiResponse::ok(())),
        Err(e) => HttpResponse::BadRequest().json(ApiResponse::<()>::error(&e)),
    }
}

/// `GET /api/cshare/{token}` — public: what this link opens (no secrets).
#[actix_web::get("/api/cshare/{token}")]
async fn share_info(token: web::Path<String>) -> HttpResponse {
    let Some(share) = resolve(&token) else {
        return HttpResponse::NotFound().finish();
    };
    let (label, protocol) = super::vnc_console::console_label_protocol(&share.console)
        .unwrap_or_else(|| (share.console.clone(), "vnc".into()));
    HttpResponse::Ok().json(ApiResponse::ok(serde_json::json!({
        "label": label, "protocol": protocol, "mode": share.mode, "needs_password": share.password_hash.is_some()
    })))
}

/// `GET /api/cshare/{token}/ws` — public: the console WebSocket for a share.
#[actix_web::get("/api/cshare/{token}/ws")]
async fn share_ws(
    req: HttpRequest,
    stream: web::Payload,
    token: web::Path<String>,
) -> Result<HttpResponse, actix_web::Error> {
    actix_web_actors::ws::handshake(&req)?;
    let Some(share) = resolve(&token) else {
        return Ok(HttpResponse::NotFound().finish());
    };
    if let Some(hash) = &share.password_hash {
        let pw = super::parse_query_param(req.query_string(), "pw").unwrap_or_default();
        if pw.is_empty() || !bcrypt::verify(&pw, hash).unwrap_or(false) {
            return Ok(HttpResponse::Forbidden().finish());
        }
    }
    super::vnc_console::open_shared_console(&req, stream, &share.console, share.mode != "control", token.into_inner())
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(create_share)
        .service(list_shares)
        .service(revoke_share)
        .service(share_info)
        .service(share_ws);
}
