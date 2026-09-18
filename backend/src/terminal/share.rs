//! Share links: a URL that opens one terminal session for someone who has no
//! NabiMan account.
//!
//! A link hands out shell access, so it carries an expiry, an optional
//! password, an optional read-only flag, and can be revoked at any time. The
//! link itself is the secret — 32 hex characters from the system RNG — and the
//! password, when set, is stored as a bcrypt hash.

use actix_web::{web, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::os::unix::io::FromRawFd;

use crate::auth::JwtSecret;
use crate::models::ApiResponse;

use super::pty::{attach_tmux, parse_size, WsBridge};
use super::session::{client_size, data_dir, now_unix, tmux_session_exists, PtyStore};
use super::share_ticket;

/// How long a password ticket stays valid — long enough to open the page, not
/// long enough to be worth passing around.
const TICKET_TTL_SECS: u64 = 12 * 3600;
const MAX_EXPIRY_SECS: u64 = 365 * 24 * 3600;

#[derive(Serialize, Deserialize, Clone)]
pub struct ShareLink {
    pub token: String,
    pub session_id: String,
    pub owner: String,
    pub label: String,
    pub created_unix: u64,
    /// 0 means the link never expires.
    pub expires_unix: u64,
    /// Stored on disk, stripped from every API response by `public_view`.
    /// (Skipping serialization here would drop it from the file too.)
    #[serde(default)]
    pub password_hash: Option<String>,
    #[serde(default)]
    pub read_only: bool,
    #[serde(default)]
    pub last_used_unix: u64,
    /// Filled in for the API so the owner can see it without the hash.
    #[serde(default)]
    pub needs_password: bool,
    /// Whether the session behind the link is still running. API-only.
    #[serde(default)]
    pub alive: bool,
}

fn shares_file() -> String {
    format!("{}/terminal_shares.json", data_dir())
}

fn load_shares() -> HashMap<String, ShareLink> {
    std::fs::read_to_string(shares_file())
        .ok()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or_default()
}

fn save_shares(shares: &HashMap<String, ShareLink>) {
    if let Ok(json) = serde_json::to_string_pretty(shares) {
        let _ = std::fs::create_dir_all(data_dir());
        let _ = std::fs::write(shares_file(), json);
    }
}

fn expired(link: &ShareLink) -> bool {
    link.expires_unix != 0 && now_unix() > link.expires_unix
}

/// Look a link up, dropping it if it has expired.
fn live_link(token: &str) -> Option<ShareLink> {
    let mut shares = load_shares();
    let link = shares.get(token)?.clone();
    if expired(&link) {
        shares.remove(token);
        save_shares(&shares);
        return None;
    }
    Some(link)
}

fn public_view(mut link: ShareLink) -> ShareLink {
    link.needs_password = link.password_hash.is_some();
    link.password_hash = None;
    link
}

// --- Owner-side API ------------------------------------------------------

#[derive(Deserialize)]
pub struct CreateShareRequest {
    /// Seconds until the link dies; 0 means never.
    pub expires_in_secs: u64,
    /// Empty or absent means anyone with the link gets straight in.
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub read_only: bool,
}

fn generate_token() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (0..4).map(|_| format!("{:08x}", rng.gen::<u32>())).collect()
}

async fn create_share(
    path: web::Path<String>,
    body: web::Json<CreateShareRequest>,
    store: web::Data<PtyStore>,
    req: HttpRequest,
) -> HttpResponse {
    let session_id = path.into_inner();
    let username = super::handlers::extract_username_header(&req);
    if body.expires_in_secs > MAX_EXPIRY_SECS {
        return HttpResponse::Ok().json(ApiResponse::<()>::error("Expiry is at most one year"));
    }

    let label = {
        let map = store.lock().unwrap();
        match map.get(&session_id) {
            Some(s) if s.owner == username => s.label.clone(),
            Some(_) => return HttpResponse::Ok().json(ApiResponse::<()>::error("Not owner")),
            None => return HttpResponse::Ok().json(ApiResponse::<()>::error("Session not found")),
        }
    };

    let password_hash = match body.password.as_deref().map(str::trim).filter(|p| !p.is_empty()) {
        Some(pw) => match bcrypt::hash(pw, 10) {
            Ok(h) => Some(h),
            Err(_) => return HttpResponse::Ok().json(ApiResponse::<()>::error("Cannot hash password")),
        },
        None => None,
    };

    let token = generate_token();
    let link = ShareLink {
        token: token.clone(),
        session_id,
        owner: username,
        label,
        created_unix: now_unix(),
        expires_unix: if body.expires_in_secs == 0 { 0 } else { now_unix() + body.expires_in_secs },
        password_hash,
        read_only: body.read_only,
        last_used_unix: 0,
        needs_password: false,
        alive: true,
    };

    let mut shares = load_shares();
    shares.insert(token, link.clone());
    save_shares(&shares);
    HttpResponse::Ok().json(ApiResponse::ok(public_view(link)))
}

async fn list_shares(store: web::Data<PtyStore>, req: HttpRequest) -> HttpResponse {
    let username = super::handlers::extract_username_header(&req);
    let shares = load_shares();
    let sessions = store.lock().unwrap();
    let mut list: Vec<ShareLink> = shares
        .into_values()
        .filter(|l| l.owner == username && !expired(l))
        .map(|l| {
            let alive = sessions
                .get(&l.session_id)
                .map(|s| tmux_session_exists(&s.tmux_name))
                .unwrap_or(false);
            ShareLink { alive, ..public_view(l) }
        })
        .collect();
    list.sort_by_key(|l| std::cmp::Reverse(l.created_unix));
    HttpResponse::Ok().json(ApiResponse::ok(list))
}

async fn revoke_share(path: web::Path<String>, req: HttpRequest) -> HttpResponse {
    let token = path.into_inner();
    let username = super::handlers::extract_username_header(&req);
    let mut shares = load_shares();
    match shares.get(&token) {
        Some(link) if link.owner == username => {
            shares.remove(&token);
            save_shares(&shares);
            HttpResponse::Ok().json(ApiResponse::ok("Link revoked"))
        }
        _ => HttpResponse::Ok().json(ApiResponse::<()>::error("Link not found")),
    }
}

// --- Visitor-side API (no account) ---------------------------------------

#[derive(Serialize)]
struct ShareInfo {
    label: String,
    needs_password: bool,
    read_only: bool,
    expires_unix: u64,
    /// False once the shell behind the link has exited.
    alive: bool,
    /// The pane's grid, so the viewer can match it before it attaches — a
    /// resize afterwards would leave the screen blank until tmux repaints.
    cols: u16,
    rows: u16,
}

async fn share_info(path: web::Path<String>, store: web::Data<PtyStore>) -> HttpResponse {
    let token = path.into_inner();
    let link = match live_link(&token) {
        Some(l) => l,
        None => return HttpResponse::Ok().json(ApiResponse::<()>::error("This link is no longer valid")),
    };
    let tmux_name = {
        let map = store.lock().unwrap();
        map.get(&link.session_id).map(|s| s.tmux_name.clone())
    };
    let alive = tmux_name.as_deref().map(tmux_session_exists).unwrap_or(false);
    let (cols, rows) = tmux_name
        .as_deref()
        .filter(|_| alive)
        .and_then(client_size)
        .unwrap_or((0, 0));
    HttpResponse::Ok().json(ApiResponse::ok(ShareInfo {
        label: link.label,
        needs_password: link.password_hash.is_some(),
        read_only: link.read_only,
        expires_unix: link.expires_unix,
        alive,
        cols,
        rows,
    }))
}

#[derive(Deserialize)]
pub struct ShareAuthRequest {
    pub password: String,
}

#[derive(Serialize)]
struct TicketResponse {
    ticket: String,
}

async fn share_auth(
    path: web::Path<String>,
    body: web::Json<ShareAuthRequest>,
    secret: web::Data<JwtSecret>,
) -> HttpResponse {
    let token = path.into_inner();
    let link = match live_link(&token) {
        Some(l) => l,
        None => return HttpResponse::Ok().json(ApiResponse::<()>::error("This link is no longer valid")),
    };
    let hash = match link.password_hash {
        Some(h) => h,
        None => return HttpResponse::Ok().json(ApiResponse::ok(TicketResponse { ticket: String::new() })),
    };
    if !bcrypt::verify(&body.password, &hash).unwrap_or(false) {
        return HttpResponse::Ok().json(ApiResponse::<()>::error("Wrong password"));
    }
    let ticket = share_ticket::sign(&secret, &token, now_unix() + TICKET_TTL_SECS);
    HttpResponse::Ok().json(ApiResponse::ok(TicketResponse { ticket }))
}

/// The visitor's terminal socket. Everything a link needs to be safe is checked
/// here: it exists, it has not expired, the password ticket holds up, and the
/// session behind it is still running.
async fn share_ws(
    path: web::Path<String>,
    req: HttpRequest,
    stream: web::Payload,
    secret: web::Data<JwtSecret>,
    store: web::Data<PtyStore>,
) -> Result<HttpResponse, actix_web::Error> {
    let token = path.into_inner();
    let query = req.query_string();

    let link = match live_link(&token) {
        Some(l) => l,
        None => return Ok(ws::start(
            super::pty::NoticeSocket::new("이 공유 링크는 만료되었거나 취소되었습니다 / link is no longer valid"),
            &req, stream,
        )?),
    };

    if link.password_hash.is_some() {
        let ticket = super::parse_query_param(query, "ticket").unwrap_or_default();
        if !share_ticket::valid(&secret, &token, &ticket) {
            return Ok(ws::start(
                super::pty::NoticeSocket::new("비밀번호 확인이 필요합니다 / password required"),
                &req, stream,
            )?);
        }
    }

    let tmux_name = {
        let map = store.lock().unwrap();
        map.get(&link.session_id).map(|s| s.tmux_name.clone())
    };
    let tmux_name = match tmux_name.filter(|n| tmux_session_exists(n)) {
        Some(n) => n,
        None => return Ok(ws::start(
            super::pty::NoticeSocket::new("공유된 세션이 종료되었습니다 / the shared session has ended"),
            &req, stream,
        )?),
    };

    // Attach at the pane's own size: opening the pseudo-terminal at the
    // visitor's size would reshape the session for the person working in it.
    let size = client_size(&tmux_name).unwrap_or_else(|| parse_size(query));
    let (fd, pid) = match attach_tmux_shared(&tmux_name, size, link.read_only) {
        Ok(r) => r,
        Err(e) => return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()>::error(&e))),
    };

    {
        let mut shares = load_shares();
        if let Some(l) = shares.get_mut(&token) {
            l.last_used_unix = now_unix();
        }
        save_shares(&shares);
        let mut map = store.lock().unwrap();
        if let Some(s) = map.get_mut(&link.session_id) {
            s.ws_count += 1;
            s.last_active_unix = now_unix();
        }
    }

    let file = unsafe { std::fs::File::from_raw_fd(fd) };
    ws::start(
        WsBridge {
            session_id: link.session_id,
            tmux_name,
            store: store.get_ref().clone(),
            master_fd: fd,
            master_file: Some(file),
            child_pid: pid,
            ended: false,
            read_only: link.read_only,
            mirror_size: true,
            last_size: Some(size),
            client_tty: super::pty::slave_tty(fd),
        },
        &req,
        stream,
    )
}

/// A read-only visitor attaches with tmux's own read-only flag as well, so a
/// stray keystroke cannot reach the shell even if the bridge let it through.
fn attach_tmux_shared(
    name: &str,
    size: (u16, u16),
    read_only: bool,
) -> Result<(i32, nix::unistd::Pid), String> {
    if read_only {
        super::pty::attach_tmux_readonly(name, size)
    } else {
        attach_tmux(name, size)
    }
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.route("/api/terminal/sessions/{id}/share-link", web::post().to(create_share))
        .route("/api/terminal/share-links", web::get().to(list_shares))
        .route("/api/terminal/share-links/{token}", web::delete().to(revoke_share))
        // Public: these validate the link themselves.
        .route("/api/share/{token}", web::get().to(share_info))
        .route("/api/share/{token}/auth", web::post().to(share_auth))
        .route("/api/share/{token}/ws", web::get().to(share_ws));
}
