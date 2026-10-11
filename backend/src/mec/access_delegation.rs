//! Explicit, expiring delegation from the highest administrator; never inferred from provider roles.
#[path = "access_delegation_storage.rs"]
mod storage;
pub use storage::save;

use super::{
    db::{audit_store::AuditStore, identity_store, DbHandle},
    handlers::{members, util::*},
    services::{ServiceError, ServiceResult},
    MecState,
};
use crate::{
    jwt_sessions::SessionStore,
    models::{
        mec::{AuditStatus, MecAuditLog, TenantStatus},
        NabimanUser, ScopeKind, UserRole,
    },
    users::UserStore,
};
use actix_web::{web, HttpRequest, HttpResponse};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
#[derive(Clone, Serialize, Deserialize)]
pub struct Delegation {
    pub tenant_id: String,
    pub version: String,
    pub realm: String,
    pub project_id: String,
    pub enabled: bool,
    pub manager_ids: Vec<String>,
    pub allowed_roles: Vec<String>,
    #[serde(default = "default_actions")]
    pub allowed_actions: Vec<String>,
    pub expires_at: Option<String>,
    pub updated_by: String,
    pub updated_at: String,
    pub reason: String,
}
pub fn read_on(conn: &rusqlite::Connection, tenant: &str) -> ServiceResult<Option<Delegation>> {
    let record:Option<(String,String,String,bool,String,String,Option<String>,String,String,String)>=conn.query_row("SELECT version,realm,project_id,enabled,manager_ids,allowed_roles,expires_at,updated_by,updated_at,reason FROM tenant_access_delegations WHERE tenant_id=?1",[tenant],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?,r.get(8)?,r.get(9)?))).optional()?;
    record
        .map(
            |(
                version,
                realm,
                project_id,
                enabled,
                managers,
                roles,
                expires_at,
                updated_by,
                updated_at,
                reason,
            )| {
                let allowed_actions = read_actions(conn, tenant, &version)?;
                Ok(Delegation {
                    tenant_id: tenant.into(),
                    version,
                    realm,
                    project_id,
                    enabled,
                    manager_ids: serde_json::from_str(&managers).map_err(|_| {
                        ServiceError::Internal("Invalid delegation manager checkpoint".into())
                    })?,
                    allowed_roles: serde_json::from_str(&roles).map_err(|_| {
                        ServiceError::Internal("Invalid delegation role checkpoint".into())
                    })?,
                    allowed_actions,
                    expires_at,
                    updated_by,
                    updated_at,
                    reason,
                })
            },
        )
        .transpose()
}
fn default_actions() -> Vec<String> {
    vec!["grant".into()]
}
fn read_actions(
    conn: &rusqlite::Connection,
    tenant: &str,
    version: &str,
) -> ServiceResult<Vec<String>> {
    let row: Option<(String, String)> = conn
        .query_row(
            "SELECT version,actions FROM tenant_access_delegation_actions WHERE tenant_id=?1",
            [tenant],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    match row {
        None => Err(ServiceError::Internal(
            "Missing delegation action checkpoint".into(),
        )),
        Some((current, actions)) if current == version => serde_json::from_str(&actions)
            .map_err(|_| ServiceError::Internal("Invalid delegation action checkpoint".into())),
        Some(_) => Err(ServiceError::Conflict(
            "Delegation action version changed".into(),
        )),
    }
}
pub fn read(db: &DbHandle, tenant: &str) -> ServiceResult<Option<Delegation>> {
    read_on(&db.lock().unwrap(), tenant)
}
impl Delegation {
    pub fn permits(&self, manager: &str, realm: &str, project: &str, role: &str) -> bool {
        self.permits_action(manager, realm, project, role, "grant")
    }
    pub fn permits_action(
        &self,
        manager: &str,
        realm: &str,
        project: &str,
        role: &str,
        action: &str,
    ) -> bool {
        ["grant", "revoke"].contains(&action)
            && self.allowed_actions.iter().any(|a| a == action)
            && self.enabled
            && self.realm == realm
            && self.project_id == project
            && self.manager_ids.iter().any(|m| m == manager)
            && self.allowed_roles.iter().any(|r| r == role)
            && ["read-only", "project-member"].contains(&role)
            && self
                .expires_at
                .as_ref()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .is_some_and(|t| t > chrono::Utc::now())
    }
}
pub fn current_actor(
    req: &HttpRequest,
    users: &UserStore,
    sessions: &SessionStore,
) -> Result<NabimanUser, HttpResponse> {
    let claims =
        crate::auth::verified_claims(req).ok_or_else(|| HttpResponse::Unauthorized().finish())?;
    if claims.exp as u128 <= chrono::Utc::now().timestamp().max(0) as u128
        || !crate::terminal_tickets::ConnectionGrant::from_request(
            req,
            sessions.clone(),
            users.clone(),
        )
        .is_some_and(|g| g.valid_for_method(req.method().as_str()))
    {
        return Err(HttpResponse::Unauthorized().finish());
    }
    users
        .lock()
        .unwrap()
        .iter()
        .find(|u| {
            u.username == claims.sub
                && Some(u.id.as_str()) == crate::auth::verified_user_id(&req).as_deref()
                && u.active()
        })
        .cloned()
        .ok_or_else(|| HttpResponse::Unauthorized().finish())
}
pub fn lower_member(actor: &NabimanUser, target: &NabimanUser, tenant: &str) -> bool {
    actor.active()
        && actor.manages(&ScopeKind::Tenant, tenant)
        && actor.id != target.id
        && target.active()
        && !target.super_admin()
        && target
            .scope_role(&ScopeKind::Tenant, tenant)
            .is_some_and(|r| *r != UserRole::Admin)
}
pub fn lower_association(actor: &NabimanUser, target: &NabimanUser, tenant: &str) -> bool {
    actor.active()
        && actor.manages(&ScopeKind::Tenant, tenant)
        && actor.id != target.id
        && target.role != UserRole::Admin
        && target.memberships.iter().any(|m| {
            m.kind == ScopeKind::Tenant && m.scope_id == tenant && m.role != UserRole::Admin
        })
}
pub fn authorize(
    db: &DbHandle,
    actor: &NabimanUser,
    target: &NabimanUser,
    tenant: &str,
    realm: &str,
    project: &str,
    role: &str,
    version: Option<&str>,
) -> ServiceResult<Option<Delegation>> {
    if actor.super_admin() {
        return Ok(None);
    }
    let policy = read(db, tenant)?.ok_or_else(|| {
        ServiceError::Forbidden("Highest-administrator delegation is required".into())
    })?;
    if !lower_member(actor, target, tenant) || !policy.permits(&actor.id, realm, project, role) {
        return Err(ServiceError::Forbidden(
            "Delegated access is outside the approved manager, member or role scope".into(),
        ));
    }
    if version != Some(policy.version.as_str()) {
        return Err(ServiceError::Conflict(
            "Delegation changed; reload before granting access".into(),
        ));
    }
    Ok(Some(policy))
}
fn no_store(mut response: HttpResponse) -> HttpResponse {
    response.headers_mut().insert(
        actix_web::http::header::CACHE_CONTROL,
        actix_web::http::header::HeaderValue::from_static("no-store"),
    );
    response
}
pub async fn get(
    req: HttpRequest,
    path: web::Path<String>,
    state: web::Data<MecState>,
    users: web::Data<UserStore>,
    sessions: web::Data<SessionStore>,
) -> HttpResponse {
    if let Err(r) = members::current_writer(&req, &users, &sessions) {
        return r;
    }
    let result = (|| {
        let profile = state
            .tenants
            .get(&path)?
            .ok_or_else(|| ServiceError::NotFound("tenant".into()))?;
        Ok::<_, ServiceError>(
            serde_json::json!({"policy":read(&state.db,&path)?,"realm":state.services.rancher.identity_realm()?,"project_id":profile.rancher_project_id}),
        )
    })();
    match result {
        Ok(data) => no_store(ok_response(data)),
        Err(e) => from_service_error(e),
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateRequest {
    pub expected_version: Option<String>,
    pub expected_realm: String,
    pub expected_project_id: String,
    pub confirm_tenant_id: String,
    pub enabled: bool,
    pub manager_ids: Vec<String>,
    pub allowed_roles: Vec<String>,
    #[serde(default = "default_actions")]
    pub allowed_actions: Vec<String>,
    pub expires_at: Option<String>,
    pub reason: String,
    pub acknowledge_project_role_scope: bool,
}
pub async fn put(
    req: HttpRequest,
    path: web::Path<String>,
    body: web::Json<UpdateRequest>,
    state: web::Data<MecState>,
    users: web::Data<UserStore>,
    sessions: web::Data<SessionStore>,
) -> HttpResponse {
    if let Err(r) = members::current_writer(&req, &users, &sessions) {
        return r;
    }
    if state.services.rancher.identity_realm().ok().as_deref() != Some(body.expected_realm.as_str())
    {
        return from_service_error(ServiceError::Conflict("Provider realm changed".into()));
    }
    let current = users.lock().unwrap();
    let active_sessions = sessions.lock().unwrap();
    let Some(claims) = crate::auth::verified_claims(&req) else {
        return HttpResponse::Unauthorized().finish();
    };
    if claims.exp as u128 <= chrono::Utc::now().timestamp().max(0) as u128
        || !active_sessions.get(&claims.sid).is_some_and(|s| {
            s.username == claims.sub
                && Some(s.user_id.as_str()) == crate::auth::verified_user_id(&req).as_deref()
        })
    {
        return HttpResponse::Unauthorized().finish();
    }
    if !current.iter().any(|u| {
        u.username == claims.sub
            && Some(u.id.as_str()) == crate::auth::verified_user_id(&req).as_deref()
            && u.active()
            && u.super_admin()
    }) {
        return HttpResponse::Forbidden().finish();
    }
    if body.enabled
        && body.manager_ids.iter().any(|id| {
            !current.iter().any(|u| {
                u.id == *id
                    && u.active()
                    && !u.super_admin()
                    && u.manages(&ScopeKind::Tenant, &path)
            })
        })
    {
        return from_service_error(ServiceError::Conflict(
            "Each delegate must be a current active tenant administrator".into(),
        ));
    }
    match save(
        &state.db,
        &path,
        &body,
        &claims.sub,
        source_ip(&req),
        user_agent(&req),
    ) {
        Ok(p) => no_store(ok_response(p)),
        Err(e) => from_service_error(e),
    }
}
pub async fn context(
    req: HttpRequest,
    path: web::Path<String>,
    state: web::Data<MecState>,
    users: web::Data<UserStore>,
    sessions: web::Data<SessionStore>,
) -> HttpResponse {
    let actor = match current_actor(&req, &users, &sessions) {
        Ok(a) => a,
        Err(r) => return r,
    };
    let result = (|| {
        let policy = read(&state.db, &path)?
            .ok_or_else(|| ServiceError::Forbidden("Delegation is not configured".into()))?;
        let profile = state
            .tenants
            .get(&path)?
            .ok_or_else(|| ServiceError::NotFound("tenant".into()))?;
        let realm = state.services.rancher.identity_realm()?;
        let project = profile
            .rancher_project_id
            .ok_or_else(|| ServiceError::Conflict("No tenant project".into()))?;
        if profile.status != TenantStatus::Active
            || !actor.manages(&ScopeKind::Tenant, &path)
            || !policy
                .allowed_roles
                .iter()
                .any(|r| policy.permits(&actor.id, &realm, &project, r))
        {
            return Err(ServiceError::Forbidden(
                "Delegation is inactive or outside the manager scope".into(),
            ));
        }
        let current = users.lock().unwrap();
        let current_actor = current
            .iter()
            .find(|u| u.id == actor.id && u.active() && u.manages(&ScopeKind::Tenant, &path))
            .ok_or_else(|| ServiceError::Forbidden("Manager scope changed".into()))?;
        let mut candidates = Vec::new();
        for target in current
            .iter()
            .filter(|u| lower_member(current_actor, u, &path))
        {
            let link = identity_store::get(&state.db, &target.id, &realm)?.map(|l|serde_json::json!({"remote_user_id":l.remote_user_id,"remote_username":l.remote_username,"verified_at":l.verified_at}));
            let role = target
                .scope_role(&ScopeKind::Tenant, &path)
                .unwrap()
                .as_str();
            candidates.push(serde_json::json!({"id":target.id,"username":target.username,"source_role":role,"link":link}));
        }
        Ok(
            serde_json::json!({"version":policy.version,"realm":realm,"project_id":project,"allowed_roles":policy.allowed_roles,"allowed_actions":policy.allowed_actions,"expires_at":policy.expires_at,"candidates":candidates}),
        )
    })();
    match result {
        Ok(data) => no_store(ok_response(data)),
        Err(e) => from_service_error(e),
    }
}

#[cfg(test)]
#[path = "access_delegation_tests.rs"]
mod tests;
