//! The super administrator's closer look at one account: what it may do
//! (remote permissions, access rules), what it has (servers, sessions, share
//! links, sign-ins), and ending its sign-ins or links.

use actix_web::{web, HttpRequest, HttpResponse};
use serde::Serialize;

use crate::models::{AccessRules, ApiResponse, NabimanUser, RemotePermissions, UserRole};
use crate::users::UserStore;

/// Applies permission and access-rule changes to `user`, refusing what makes no sense.
pub(super) fn apply(
    user: &mut NabimanUser,
    permissions: Option<&RemotePermissions>,
    access: Option<&AccessRules>,
) -> Result<(), String> {
    if let Some(p) = permissions {
        let limits = [
            (p.share_max_days, 90, "공유 링크 최대 기간"),
            (p.max_sessions, 100, "동시 세션 수"),
            (p.max_idle_minutes, 30 * 24 * 60, "최대 유휴 시간"),
            (p.max_servers, 1000, "등록 서버 수"),
            (p.max_upload_mb, 1024 * 1024, "업로드 크기"),
        ];
        if let Some((_, max, what)) = limits.iter().find(|(v, max, _)| v > max) {
            return Err(format!("{what}는 {max} 이하로 정하세요 (0은 제한 없음)"));
        }
        user.remote_permissions = p.clone();
    }
    if let Some(rules) = access {
        if user.role == UserRole::Admin && *rules != AccessRules::default() {
            return Err("최고관리자에게는 이용 기간·접속 제한을 둘 수 없습니다".into());
        }
        crate::access_rules::validate(rules)?;
        user.access = rules.clone();
    }
    Ok(())
}

#[derive(Serialize)]
struct ServerLine {
    id: String,
    name: String,
    address: String,
    status: String,
}

#[derive(Serialize)]
struct SessionLine {
    id: String,
    label: String,
    title: String,
    created_unix: u64,
    last_active_unix: u64,
    keepalive: bool,
    viewers: u32,
}

#[derive(Serialize)]
struct Overview {
    own_servers: Vec<ServerLine>,
    assigned_servers: Vec<ServerLine>,
    terminal_sessions: Vec<SessionLine>,
    share_links: Vec<crate::terminal::share::ShareLink>,
    sign_ins: Vec<crate::jwt_sessions::SessionInfo>,
}

fn find(store: &UserStore, id: &str) -> Option<NabimanUser> {
    store.lock().ok()?.iter().find(|u| u.id == id).cloned()
}

fn line(s: crate::models::RemoteServer) -> ServerLine {
    ServerLine { address: format!("{}@{}:{}", s.user, s.host, s.port), id: s.id, name: s.name, status: s.status }
}

/// GET /api/users/{id}/overview
pub(super) async fn overview(
    req: HttpRequest,
    path: web::Path<String>,
    store: web::Data<UserStore>,
    terminals: web::Data<crate::terminal::PtyStore>,
    shares: web::Data<crate::terminal::share_store::ShareStore>,
    sessions: web::Data<crate::jwt_sessions::SessionStore>,
) -> HttpResponse {
    if let Err(r) = crate::membership::require_super(&req, &store) {
        return r;
    }
    let Some(user) = find(&store, &path) else {
        return HttpResponse::NotFound().json(ApiResponse::<()>::error("회원을 찾을 수 없습니다"));
    };
    let servers = crate::remote_servers::load_servers().unwrap_or_default();
    let own_servers = servers.iter().filter(|s| s.owner_id == user.id).cloned().map(line).collect();
    let assigned_servers = servers
        .iter()
        .filter(|s| crate::member_servers::assigned(&user, s))
        .cloned()
        .map(line)
        .collect();
    let mut terminal_sessions: Vec<SessionLine> = terminals
        .lock()
        .map(|map| {
            map.iter()
                .filter(|(_, s)| s.owned_by(&user.id))
                .map(|(id, s)| SessionLine {
                    id: id.clone(),
                    label: s.label.clone(),
                    title: s.title.clone(),
                    created_unix: s.created_unix,
                    last_active_unix: s.last_active_unix,
                    keepalive: s.keepalive,
                    viewers: s.ws_count,
                })
                .collect()
        })
        .unwrap_or_default();
    terminal_sessions.sort_by_key(|s| std::cmp::Reverse(s.last_active_unix));
    let share_links = crate::terminal::share::links_of(&shares, &store, &user.id);
    let mut sign_ins: Vec<_> = sessions
        .lock()
        .map(|map| map.values().filter(|s| s.user_id == user.id).cloned().collect())
        .unwrap_or_default();
    sign_ins.sort_by(|a: &crate::jwt_sessions::SessionInfo, b| b.last_active.cmp(&a.last_active));
    HttpResponse::Ok().insert_header(("Cache-Control", "no-store")).json(ApiResponse::ok(Overview {
        own_servers,
        assigned_servers,
        terminal_sessions,
        share_links,
        sign_ins,
    }))
}

/// POST /api/users/{id}/sign-out: ends every sign-in of the account.
pub(super) async fn sign_out(
    req: HttpRequest,
    path: web::Path<String>,
    store: web::Data<UserStore>,
    sessions: web::Data<crate::jwt_sessions::SessionStore>,
) -> HttpResponse {
    if let Err(r) = crate::membership::require_super(&req, &store) {
        return r;
    }
    let Some(user) = find(&store, &path) else {
        return HttpResponse::NotFound().json(ApiResponse::<()>::error("회원을 찾을 수 없습니다"));
    };
    let own = crate::auth::verified_claims(&req).map(|c| c.sid);
    crate::jwt_sessions::revoke_user_sessions(&sessions, &user.username, own.as_deref());
    HttpResponse::Ok().json(ApiResponse::ok(format!("{} 의 로그인을 모두 끝냈습니다", user.username)))
}

/// POST /api/users/{id}/share-links/revoke: ends every link the account made.
pub(super) async fn revoke_links(
    req: HttpRequest,
    path: web::Path<String>,
    store: web::Data<UserStore>,
    shares: web::Data<crate::terminal::share_store::ShareStore>,
) -> HttpResponse {
    if let Err(r) = crate::membership::require_super(&req, &store) {
        return r;
    }
    match crate::terminal::share::revoke_links_of(&shares, &path) {
        Ok(n) => HttpResponse::Ok().json(ApiResponse::ok(format!("공유 링크 {n}개를 해제했습니다"))),
        Err(e) => HttpResponse::ServiceUnavailable().json(ApiResponse::<()>::error(&e)),
    }
}

/// GET /api/users/my-address: the address this server sees the administrator
/// at, to check an allowed-network rule against before relying on it.
pub(super) async fn my_address(req: HttpRequest, store: web::Data<UserStore>) -> HttpResponse {
    if let Err(r) = crate::membership::require_super(&req, &store) {
        return r;
    }
    let ip = crate::access_rules::client_ip(&req).map(|ip| ip.to_string()).unwrap_or_default();
    HttpResponse::Ok().insert_header(("Cache-Control", "no-store")).json(ApiResponse::ok(ip))
}
