//! Operational policies for service membership, separate from infrastructure credentials.
use crate::models::{ApiResponse, Membership, NabimanUser, ScopeKind, UserRole};
use crate::users::UserStore;
use actix_web::{web, HttpRequest, HttpResponse};
use std::{path::PathBuf, sync::Mutex};
#[path = "scope_policy_store.rs"]
mod storage;
use storage::{load, Record};

static SAVE_LOCK: Mutex<()> = Mutex::new(());
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ScopeSettings {
    pub contact_email: String,
    pub member_notice: String,
    pub member_limit: Option<usize>,
    pub allow_commands: bool,
    pub require_mfa: bool,
}
impl Default for ScopeSettings {
    fn default() -> Self {
        Self {
            contact_email: String::new(),
            member_notice: String::new(),
            member_limit: None,
            allow_commands: true,
            require_mfa: false,
        }
    }
}
#[cfg(test)]
thread_local! { pub(crate) static TEST_PATH: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) }; }
fn path() -> PathBuf {
    #[cfg(test)]
    if let Some(path) = TEST_PATH.with(|p| p.borrow().clone()) {
        return path;
    }
    PathBuf::from(std::env::var("NABIMAN_DATA_DIR").unwrap_or_else(|_| "./data".into()))
        .join("scope-settings.json")
}
fn key(kind: &ScopeKind, id: &str) -> String {
    format!(
        "{}:{id}",
        if *kind == ScopeKind::Server {
            "server"
        } else {
            "tenant"
        }
    )
}
fn record(kind: &ScopeKind, id: &str) -> Result<Record, String> {
    let key = key(kind, id);
    Ok(load()?
        .remove(&key)
        .unwrap_or_else(|| Record::legacy(&key, ScopeSettings::default())))
}
pub fn read(kind: &ScopeKind, id: &str) -> Result<ScopeSettings, String> {
    Ok(record(kind, id)?.policy)
}
fn count(users: &[NabimanUser], exclude: &str, m: &Membership) -> usize {
    users
        .iter()
        .filter(|u| {
            u.active()
                && u.id != exclude
                && u.memberships
                    .iter()
                    .any(|a| a.kind == m.kind && a.scope_id == m.scope_id)
        })
        .count()
}
pub fn check_capacity(users: &[NabimanUser], target: &str, m: &Membership) -> Result<(), String> {
    if let Some(limit) = read(&m.kind, &m.scope_id)?.member_limit {
        if count(users, target, m) >= limit {
            return Err("소속의 회원 수 제한을 초과합니다".into());
        }
    }
    Ok(())
}
fn fail(message: &str) -> HttpResponse {
    HttpResponse::BadRequest().json(ApiResponse::<()>::error(message))
}
fn scope(
    req: &HttpRequest,
    actor: &NabimanUser,
    parts: &(String, String),
    write: bool,
) -> Result<Membership, HttpResponse> {
    let kind = match parts.0.as_str() {
        "server" => ScopeKind::Server,
        "tenant" => ScopeKind::Tenant,
        _ => return Err(fail("소속 종류가 올바르지 않습니다")),
    };
    if !(actor.super_admin()
        || if write {
            actor.manages(&kind, &parts.1)
        } else {
            actor.scope_role(&kind, &parts.1).is_some()
        })
    {
        return Err(HttpResponse::Forbidden()
            .json(ApiResponse::<()>::error("이 소속을 관리할 권한이 없습니다")));
    }
    let m = Membership {
        kind,
        scope_id: parts.1.clone(),
        role: UserRole::Viewer,
        enabled: true,
        allow_exec: None,
        expires_at: None,
    };
    crate::membership::validate_scope(req, &m)?;
    Ok(m)
}
async fn get(
    req: HttpRequest,
    parts: web::Path<(String, String)>,
    users: web::Data<UserStore>,
) -> HttpResponse {
    let actor = match crate::membership::actor(&req, &users) {
        Ok(a) => a,
        Err(r) => return r,
    };
    let m = match scope(&req, &actor, &parts, false) {
        Ok(m) => m,
        Err(r) => return r,
    };
    match record(&m.kind, &m.scope_id) {
        Ok(s) => HttpResponse::Ok()
            .insert_header(("Cache-Control", "no-store"))
            .json(ApiResponse::ok(s.view())),
        Err(e) => fail(&e),
    }
}
async fn put(
    req: HttpRequest,
    parts: web::Path<(String, String)>,
    body: web::Json<serde_json::Value>,
    users: web::Data<UserStore>,
) -> HttpResponse {
    let mut request = body.into_inner();
    let expected = request
        .as_object_mut()
        .and_then(|o| o.remove("expected_revision"));
    let reason = request
        .as_object_mut()
        .and_then(|o| o.remove("change_reason"));
    let supplied_mfa = request.get("require_mfa").is_some();
    let mut body: ScopeSettings = match serde_json::from_value(request) {
        Ok(value) => value,
        Err(_) => return fail("운영 설정 형식이 올바르지 않습니다"),
    };
    let actor = match crate::membership::actor(&req, &users) {
        Ok(a) => a,
        Err(r) => return r,
    };
    let m = match scope(&req, &actor, &parts, true) {
        Ok(m) => m,
        Err(r) => return r,
    };
    let Some(expected) = expected
        .as_ref()
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
    else {
        return HttpResponse::PreconditionRequired().json(ApiResponse::<()>::error(
            "최신 설정을 다시 불러온 뒤 저장하세요",
        ));
    };
    let Some(reason) = reason
        .as_ref()
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| (3..=500).contains(&s.chars().count()) && !s.chars().any(char::is_control))
    else {
        return fail("변경 사유를 3~500자로 입력하세요");
    };
    if body.contact_email.len() > 254
        || (!body.contact_email.is_empty()
            && (!body.contact_email.contains('@')
                || body.contact_email.chars().any(char::is_whitespace)))
    {
        return fail("담당자 이메일이 올바르지 않습니다");
    }
    if body.member_notice.len() > 4000 || body.member_limit.is_some_and(|n| n == 0 || n > 10000) {
        return fail("공지는 4000바이트 이하, 회원 수 제한은 1~10000명으로 입력하세요");
    }
    let _guard = SAVE_LOCK.lock().unwrap();
    let users = users.lock().unwrap(); // Keep membership changes and capacity changes consistent.
    let Some(current) = users
        .iter()
        .find(|u| u.id == actor.id && u.active() && u.manages(&m.kind, &m.scope_id))
    else {
        return HttpResponse::Forbidden().finish();
    };
    let Some(claims) = crate::auth::verified_claims(&req) else {
        return HttpResponse::Unauthorized().finish();
    };
    let Some(sessions) = req.app_data::<web::Data<crate::jwt_sessions::SessionStore>>() else {
        return HttpResponse::Unauthorized().finish();
    };
    let sessions = sessions.lock().unwrap();
    let Some(session) = sessions
        .get(&claims.sid)
        .filter(|s| s.user_id == current.id && s.username == current.username)
    else {
        return HttpResponse::Unauthorized().finish();
    };
    if claims.exp as u128 <= chrono::Utc::now().timestamp().max(0) as u128 {
        return HttpResponse::Unauthorized().finish();
    }
    let mut all = match load() {
        Ok(a) => a,
        Err(e) => return fail(&e),
    };
    let scope_key = key(&m.kind, &m.scope_id);
    let mut previous = all
        .remove(&scope_key)
        .unwrap_or_else(|| Record::legacy(&scope_key, ScopeSettings::default()));
    if previous.revision != expected {
        return HttpResponse::Conflict().insert_header(("Cache-Control","no-store"))
            .json(serde_json::json!({"success":false,"data":null,"code":"scope_policy_conflict","message":"다른 관리자가 설정을 변경했습니다. 작성한 내용을 확인하고 최신 설정을 다시 불러오세요"}));
    }
    if !supplied_mfa {
        body.require_mfa = previous.policy.require_mfa;
    }
    if !current.super_admin()
        && (body.require_mfa || crate::mfa_policy::required(current).unwrap_or(true))
    {
        if !current.totp_enabled || !session.mfa_verified {
            return fail("먼저 본인의 2단계 인증을 등록하고 다시 로그인하세요");
        }
    }
    if body
        .member_limit
        .is_some_and(|limit| count(&users, "", &m) > limit)
    {
        return fail("현재 소속 회원 수보다 낮은 제한을 설정할 수 없습니다");
    }
    previous.replace(body, current, reason.into());
    all.insert(scope_key, previous);
    let result = storage::save(&all);
    match result {
        Ok(()) => HttpResponse::Ok()
            .insert_header(("Cache-Control", "no-store"))
            .json(ApiResponse::ok("소속 운영 설정을 저장했습니다")),
        Err(_) => HttpResponse::InternalServerError()
            .json(ApiResponse::<()>::error("소속 설정을 저장하지 못했습니다")),
    }
}
async fn history(
    req: HttpRequest,
    parts: web::Path<(String, String)>,
    users: web::Data<UserStore>,
) -> HttpResponse {
    let actor = match crate::membership::actor(&req, &users) {
        Ok(a) => a,
        Err(r) => return r,
    };
    let m = match scope(&req, &actor, &parts, true) {
        Ok(m) => m,
        Err(r) => return r,
    };
    match record(&m.kind, &m.scope_id) {
        Ok(record) => HttpResponse::Ok()
            .insert_header(("Cache-Control", "no-store"))
            .json(ApiResponse::ok(
                record.history.into_iter().rev().collect::<Vec<_>>(),
            )),
        Err(e) => fail(&e),
    }
}
pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.route(
        "/api/scope-settings/{kind}/{id}/history",
        web::get().to(history),
    )
    .route("/api/scope-settings/{kind}/{id}", web::get().to(get))
    .route("/api/scope-settings/{kind}/{id}", web::put().to(put));
}
