//! Local approval and membership administration. External roles never grant these rights.
use crate::extension_points::ExtensionPoints;
use crate::models::{AccountStatus, ApiResponse, Membership, NabimanUser, ScopeKind, UserRole};
use crate::users::{commit_users, generate_id, UserStore};
use actix_web::{web, HttpRequest, HttpResponse};

fn error(message: &str) -> HttpResponse {
    HttpResponse::BadRequest().json(ApiResponse::<()>::error(message))
}
fn forbidden() -> HttpResponse {
    HttpResponse::Forbidden().json(ApiResponse::<()>::error(
        "이 회원 또는 소속을 관리할 권한이 없습니다",
    ))
}
pub(crate) fn actor(req: &HttpRequest, store: &UserStore) -> Result<NabimanUser, HttpResponse> {
    crate::auth::extract_username(req)
        .and_then(|n| crate::users::find_user_by_name(store, &n))
        .filter(|u| {
            u.active() && Some(u.id.as_str()) == crate::auth::verified_user_id(req).as_deref()
        })
        .ok_or_else(|| HttpResponse::Unauthorized().finish())
}
pub(crate) fn require_super(req: &HttpRequest, store: &UserStore) -> Result<(), HttpResponse> {
    if actor(req, store)?.super_admin() {
        Ok(())
    } else {
        Err(forbidden())
    }
}
pub(crate) fn visible(actor: &NabimanUser, target: &NabimanUser) -> bool {
    actor.super_admin()
        || actor.id == target.id
        || (!target.super_admin()
            && target.active()
            && target
                .memberships
                .iter()
                .any(|m| actor.manages(&m.kind, &m.scope_id)))
}

pub(crate) fn validate_scope(req: &HttpRequest, m: &Membership) -> Result<(), HttpResponse> {
    if m.role == UserRole::Remote {
        return Err(error("원격 전용은 회원 등급이며 소속 역할로는 쓸 수 없습니다"));
    }
    if m.expires_at
        .as_ref()
        .is_some_and(|s| chrono::DateTime::parse_from_rfc3339(s).is_err())
    {
        return Err(error("접근 만료일 형식이 올바르지 않습니다"));
    }
    if m.scope_id.is_empty()
        || m.scope_id.len() > 128
        || !m
            .scope_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(error("소속 ID가 올바르지 않습니다"));
    }
    let exists = match m.kind {
        ScopeKind::Server => {
            let points = ExtensionPoints::of(req.app_data::<web::Data<ExtensionPoints>>());
            match points.server_exists {
                Some(exists) => exists(&m.scope_id).map_err(|_| error("서버 목록을 읽을 수 없습니다"))?,
                None => false,
            }
        }
        ScopeKind::Tenant => req
            .app_data::<web::Data<crate::mec::MecState>>()
            .and_then(|s| s.tenants.get(&m.scope_id).ok().flatten())
            .is_some(),
    };
    if exists {
        Ok(())
    } else {
        Err(error("존재하지 않는 서버 또는 tenant입니다"))
    }
}

/// A sign-up's password: eight characters or more (Korean counts a letter
/// each), and at most the 72 bytes bcrypt reads.
fn valid_signup_password(password: &str) -> bool {
    password.chars().count() >= 8 && password.len() <= 72
}

#[derive(serde::Deserialize)]
struct RegisterRequest {
    username: String,
    password: String,
    #[serde(default)]
    requested_memberships: Vec<Membership>,
}
async fn register(
    req: HttpRequest,
    body: web::Json<RegisterRequest>,
    store: web::Data<UserStore>,
) -> HttpResponse {
    if !(2..=32).contains(&body.username.len())
        || !body
            .username
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return error("아이디는 영문·숫자·밑줄·하이픈 2~32자로 입력하세요");
    }
    if !valid_signup_password(&body.password) {
        return error("비밀번호는 8자 이상으로 입력하세요 (최대 72바이트)");
    }
    if body.requested_memberships.len() > 32 {
        return error("소속 신청이 너무 많습니다");
    }
    for m in &body.requested_memberships {
        if let Err(r) = validate_scope(&req, m) {
            return r;
        }
    }
    let hash = match bcrypt::hash(&body.password, 10) {
        Ok(h) => h,
        Err(_) => return error("가입 처리 실패"),
    };
    let mut users = store.lock().unwrap();
    if users.iter().any(|u| u.username == body.username) {
        return error("이미 사용 중인 아이디입니다");
    }
    let previous = users.clone();
    let mut requested = body.requested_memberships.clone();
    for m in &mut requested {
        m.role = UserRole::Viewer;
        m.enabled = true;
        m.allow_exec = None;
        m.expires_at = None;
    }
    users.push(NabimanUser {
        share_authorization_version: uuid::Uuid::new_v4().to_string(),
        id: generate_id(),
        username: body.username.clone(),
        password_hash: hash,
        role: UserRole::Viewer,
        status: AccountStatus::Pending,
        memberships: vec![],
        requested_memberships: requested,
        scope_restricted: true,
        created_at: chrono::Utc::now().to_rfc3339(),
        last_login: None,
        totp_enabled: false,
        totp_secret: None,
        mfa: Default::default(),
        remote_permissions: Default::default(),
        access: Default::default(),
    });
    match commit_users(&mut users, previous) {
        Ok(()) => HttpResponse::Created().json(ApiResponse::ok(
            "가입 신청이 접수되었습니다. 최고관리자 승인 후 로그인할 수 있습니다.",
        )),
        Err(e) => HttpResponse::InternalServerError().json(ApiResponse::<()>::error(&e)),
    }
}

#[derive(serde::Deserialize)]
struct ReviewRequest {
    status: AccountStatus,
    #[serde(default)]
    memberships: Vec<Membership>,
}
async fn review(
    req: HttpRequest,
    id: web::Path<String>,
    body: web::Json<ReviewRequest>,
    store: web::Data<UserStore>,
    sessions: web::Data<crate::jwt_sessions::SessionStore>,
) -> HttpResponse {
    if let Err(r) = require_super(&req, &store) {
        return r;
    }
    if body.status == AccountStatus::Pending {
        return error("승인·반려·정지 중 선택하세요");
    }
    if body.memberships.len() > 32 {
        return error("소속이 너무 많습니다");
    }
    for (i, m) in body.memberships.iter().enumerate() {
        if let Err(r) = validate_scope(&req, m) {
            return r;
        }
        if body.memberships[..i]
            .iter()
            .any(|a| a.kind == m.kind && a.scope_id == m.scope_id)
        {
            return error("같은 소속을 중복 지정할 수 없습니다");
        }
    }
    let mut users = store.lock().unwrap();
    let Some(index) = users.iter().position(|u| u.id == *id) else {
        return error("회원을 찾을 수 없습니다");
    };
    if users[index].super_admin()
        && body.status != AccountStatus::Active
        && users.iter().filter(|u| u.super_admin()).count() <= 1
    {
        return error("마지막 최고관리자를 정지할 수 없습니다");
    }
    if body.status == AccountStatus::Active {
        for m in &body.memberships {
            if let Err(e) = crate::scope_settings::check_capacity(&users, &users[index].id, m) {
                return error(&e);
            }
        }
    }
    let previous = users.clone();
    let u = &mut users[index];
    u.status = body.status.clone();
    u.memberships = body.memberships.clone();
    u.scope_restricted = true;
    u.requested_memberships.clear();
    let name = u.username.clone();
    match commit_users(&mut users, previous) {
        Ok(()) => {
            crate::jwt_sessions::revoke_user_sessions(&sessions, &name, None);
            HttpResponse::Ok().json(ApiResponse::ok("회원 상태와 소속을 저장했습니다"))
        }
        Err(e) => HttpResponse::InternalServerError().json(ApiResponse::<()>::error(&e)),
    }
}

#[derive(serde::Deserialize)]
struct MemberRequest {
    username: String,
    membership: Membership,
    #[serde(default)]
    remove: bool,
}
async fn member(
    req: HttpRequest,
    body: web::Json<MemberRequest>,
    store: web::Data<UserStore>,
    sessions: web::Data<crate::jwt_sessions::SessionStore>,
) -> HttpResponse {
    let actor = match actor(&req, &store) {
        Ok(u) => u,
        Err(r) => return r,
    };
    let m = &body.membership;
    if !actor.manages(&m.kind, &m.scope_id) || (!actor.super_admin() && m.role == UserRole::Admin) {
        return forbidden();
    }
    if let Err(r) = validate_scope(&req, m) {
        return r;
    }
    let mut users = store.lock().unwrap();
    let Some(index) = users.iter().position(|u| u.username == body.username) else {
        return error("회원을 찾을 수 없습니다");
    };
    let u = &users[index];
    if !u.active() {
        return error("최고관리자가 승인한 회원만 소속에 추가할 수 있습니다");
    }
    if !actor.super_admin()
        && (u.super_admin()
            || u.id == actor.id
            || u.memberships
                .iter()
                .any(|a| a.kind == m.kind && a.scope_id == m.scope_id && a.role == UserRole::Admin))
    {
        return forbidden();
    }
    if !body.remove
        && u.memberships.len() >= 32
        && !u
            .memberships
            .iter()
            .any(|a| a.kind == m.kind && a.scope_id == m.scope_id)
    {
        return error("소속이 너무 많습니다");
    }
    if !body.remove {
        if let Err(e) = crate::scope_settings::check_capacity(&users, &u.id, m) {
            return error(&e);
        }
    }
    let previous = users.clone();
    let u = &mut users[index];
    u.memberships
        .retain(|a| a.kind != m.kind || a.scope_id != m.scope_id);
    if !body.remove {
        u.memberships.push(m.clone());
    }
    u.scope_restricted = true;
    let name = u.username.clone();
    match commit_users(&mut users, previous) {
        Ok(()) => {
            crate::jwt_sessions::revoke_user_sessions(&sessions, &name, None);
            HttpResponse::Ok().json(ApiResponse::ok("소속 권한을 변경했습니다"))
        }
        Err(e) => HttpResponse::InternalServerError().json(ApiResponse::<()>::error(&e)),
    }
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.route("/api/auth/register", web::post().to(register))
        .route("/api/users/memberships", web::post().to(member))
        .route("/api/users/{id}/review", web::post().to(review));
}

#[cfg(test)]
#[path = "membership_tests.rs"]
mod tests;
