//! Share links: a URL that opens one terminal session — or, for a server
//! reached only for its files, that server's files — for someone who has no
//! NabiMan account.
//!
//! A link hands out shell access, so it carries an expiry, an optional
//! password, an optional read-only flag, and can be revoked at any time. The
//! link itself is the secret — 64 hex characters from the system RNG — and the
//! password, when set, is stored as a bcrypt hash.

#[path = "share_admission.rs"]
mod admission;
use admission::{share_auth, share_ws};

#[path = "share_management.rs"]
mod management;
use management::{create_share, list_shares, revoke_share};

#[path = "share_files.rs"]
mod files;
use files::{create_files_share, files_link_alive};
pub(crate) use files::files_link_server;
#[cfg(test)]
use management::{generate_token, CreateShareRequest};

use super::share_store::ShareStore;
use crate::{models::NabimanUser, users::UserStore};
use actix_web::{web, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use std::io;
use std::os::unix::io::FromRawFd;

use crate::models::ApiResponse;

use super::pty::{attach_tmux, parse_size, WsBridge};
use super::session::{client_size, now_unix, tmux_session_exists, PtyStore};
use super::share_ticket;

const MAX_EXPIRY_SECS: u64 = 30 * 24 * 3600;

#[derive(Serialize, Deserialize, Clone)]
pub struct ShareLink {
    pub token: String,
    pub session_id: String,
    pub owner: String,
    /// Legacy links have no immutable owner ID and must be reissued.
    #[serde(default)]
    pub owner_id: String,
    /// Missing generations are legacy links requiring explicit reissue.
    #[serde(default)]
    pub owner_authorization_version: String,
    pub label: String,
    pub created_unix: u64,
    /// Legacy 0 values are retained for cleanup and denied admission.
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
    /// Admission is denied for legacy, expired or invalid-owner links even if tmux runs.
    #[serde(default)]
    pub usable: bool,
    /// Whether the visitor also gets the file panel, and what it may do.
    #[serde(default)]
    pub files: ShareFiles,
    /// Set instead of `session_id` for a link to a saved server's files alone.
    #[serde(default)]
    pub server_id: String,
    /// How that server was reached when the link was made; any change to it
    /// (address, user, protocol, password, approved key) ends the link.
    #[serde(default)]
    pub server_binding: String,
}

/// File access a link grants alongside its terminal. Links made before this
/// existed carry none.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ShareFiles {
    #[default]
    None,
    /// Browse and download.
    Read,
    /// Also upload, make folders, rename, move and delete.
    Write,
}

impl ShareLink {
    /// A link to a server's files, with no terminal behind it.
    pub(crate) fn files_only(&self) -> bool {
        self.session_id.is_empty() && !self.server_id.is_empty()
    }
    pub(crate) fn same_authorization(&self, other: &Self) -> bool {
        self.token == other.token
            && self.owner_id == other.owner_id
            && self.owner_authorization_version == other.owner_authorization_version
            && self.owner == other.owner
            && self.session_id == other.session_id
            && self.server_id == other.server_id
            && self.server_binding == other.server_binding
            && self.read_only == other.read_only
            && self.files == other.files
            && self.password_hash == other.password_hash
            && self.created_unix == other.created_unix
            && self.expires_unix == other.expires_unix
    }
}

fn expired(link: &ShareLink) -> bool {
    let now = now_unix();
    link.expires_unix == 0
        || now < link.created_unix
        || now >= link.expires_unix
        || link.expires_unix.saturating_sub(link.created_unix) > MAX_EXPIRY_SECS
}

pub(crate) fn live_link(shares: &ShareStore, users: &UserStore, token: &str) -> Option<ShareLink> {
    let link = shares.lock().ok()?.get(token)?;
    (!expired(&link)
        && !link.owner_id.is_empty()
        && users.lock().ok()?.iter().any(|u| {
            u.id == link.owner_id
                && u.username == link.owner
                && u.super_admin()
                && crate::scope_access::permission(u, "GET", "/api/terminal")
                && !link.owner_authorization_version.is_empty()
                && u.share_authorization_version == link.owner_authorization_version
        }))
    .then_some(link)
}

fn current_owner(req: &HttpRequest, users: &UserStore) -> Option<NabimanUser> {
    let username = crate::auth::extract_username(req)?;
    users
        .lock()
        .ok()?
        .iter()
        .find(|u| u.username == username && u.super_admin())
        .cloned()
}
fn no_store(mut response: HttpResponse) -> HttpResponse {
    response.headers_mut().insert(
        actix_web::http::header::CACHE_CONTROL,
        actix_web::http::header::HeaderValue::from_static("no-store"),
    );
    response
}
fn storage_error() -> HttpResponse {
    no_store(
        HttpResponse::ServiceUnavailable().json(ApiResponse::<()>::error(
            "Share registry unavailable; access denied",
        )),
    )
}

/// Live capability: link snapshot, immutable owner and exact restored terminal tuple.
pub(crate) struct ShareGrant {
    pub(super) link: ShareLink,
    pub(super) tmux_name: String,
    pub(super) shares: ShareStore,
    pub(super) users: UserStore,
    pub(super) sessions: PtyStore,
}
impl ShareGrant {
    pub(crate) fn valid(&self) -> bool {
        let Some(link) = live_link(&self.shares, &self.users, &self.link.token) else {
            return false;
        };
        if !link.same_authorization(&self.link) {
            return false;
        }
        self.sessions
            .lock()
            .ok()
            .and_then(|m| {
                m.get(&link.session_id).map(|s| {
                    s.owned_by(&link.owner_id)
                        && s.tmux_name == self.tmux_name
                        && super::remote_access::allowed(s)
                })
            })
            .unwrap_or(false)
    }
}

fn public_view(mut link: ShareLink) -> ShareLink {
    link.usable = link.usable
        && !expired(&link)
        && !link.owner_id.is_empty()
        && !link.owner_authorization_version.is_empty();
    link.needs_password = link.password_hash.is_some();
    link.password_hash = None;
    link
}

// --- Owner-side API ------------------------------------------------------

#[derive(Serialize)]
struct ShareInfo {
    /// `terminal`, or `files` for a link to a server's files alone.
    kind: &'static str,
    /// The visitor may dictate into the terminal: it is set up, and the link lets them type.
    dictation: bool,
    label: String,
    needs_password: bool,
    read_only: bool,
    files: ShareFiles,
    expires_unix: u64,
    /// False once the shell behind the link has exited.
    alive: bool,
    /// The pane's grid, so the viewer can match it before it attaches — a
    /// resize afterwards would leave the screen blank until tmux repaints.
    cols: u16,
    rows: u16,
}

async fn share_info(
    path: web::Path<String>,
    store: web::Data<PtyStore>,
    shares: web::Data<ShareStore>,
    users: web::Data<UserStore>,
) -> HttpResponse {
    if !shares.lock().is_ok_and(|r| r.ready()) {
        return storage_error();
    }
    let token = path.into_inner();
    let link = match live_link(&shares, &users, &token) {
        Some(l) => l,
        None => {
            return HttpResponse::Ok()
                .insert_header((actix_web::http::header::CACHE_CONTROL, "no-store"))
                .json(ApiResponse::<()>::error("This link is no longer valid"))
        }
    };
    let tmux_name = {
        let map = store.lock().unwrap();
        map.get(&link.session_id)
            .filter(|s| s.owned_by(&link.owner_id) && super::remote_access::allowed(s))
            .map(|s| s.tmux_name.clone())
    };
    let alive = if link.files_only() {
        files_link_alive(&link)
    } else {
        tmux_name.as_deref().map(tmux_session_exists).unwrap_or(false)
    };
    let (cols, rows) = tmux_name
        .as_deref()
        .filter(|_| alive)
        .and_then(client_size)
        .unwrap_or((0, 0));
    no_store(
        HttpResponse::Ok()
            .insert_header((actix_web::http::header::CACHE_CONTROL, "no-store"))
            .json(ApiResponse::ok(ShareInfo {
                kind: if link.files_only() { "files" } else { "terminal" },
                dictation: !link.read_only && !link.files_only() && super::dictation::load().key().is_some(),
                label: link.label,
                needs_password: link.password_hash.is_some(),
                read_only: link.read_only,
                files: link.files,
                expires_unix: link.expires_unix,
                alive,
                cols,
                rows,
            })),
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
    cfg.route(
        "/api/terminal/sessions/{id}/share-link",
        web::post().to(create_share),
    )
    .route(
        "/api/terminal/files/{id}/share-link",
        web::post().to(create_files_share),
    )
    .route("/api/terminal/share-links", web::get().to(list_shares))
    .route(
        "/api/terminal/share-links/{token}",
        web::delete().to(revoke_share),
    )
    // Public: these validate the link themselves.
    .route("/api/share/{token}", web::get().to(share_info))
    .route("/api/share/{token}/auth", web::post().to(share_auth))
    .route("/api/share/{token}/ws", web::get().to(share_ws));
}

#[cfg(test)]
#[path = "share_tests.rs"]
mod tests;
