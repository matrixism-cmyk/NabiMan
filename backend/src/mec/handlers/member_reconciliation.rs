use super::*;

pub async fn operations(
    req: HttpRequest,
    path: web::Path<String>,
    users: web::Data<UserStore>,
    sessions: web::Data<SessionStore>,
    state: web::Data<MecState>,
) -> HttpResponse {
    if let Err(r) = current_writer(&req, &users, &sessions) {
        return r;
    }
    match membership_store::list(&state.db, &path) {
        Ok(operations) => {
            let mut response = list_response(operations);
            response.headers_mut().insert(
                actix_web::http::header::CACHE_CONTROL,
                actix_web::http::header::HeaderValue::from_static("no-store"),
            );
            response
        }
        Err(e) => from_service_error(e),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReconcileRequest {
    pub confirm_operation_id: String,
    pub expected_binding_id: String,
    pub expected_updated_at: String,
    pub reason: String,
}
/// Confirm already observed outcomes; never replays a provider mutation.
pub async fn reconcile(
    req: HttpRequest,
    path: web::Path<(String, String)>,
    body: web::Json<ReconcileRequest>,
    users: web::Data<UserStore>,
    sessions: web::Data<SessionStore>,
    state: web::Data<MecState>,
) -> HttpResponse {
    if let Err(r) = current_writer(&req, &users, &sessions) {
        return r;
    }
    let (tenant, id) = path.into_inner();
    let reason = body.reason.trim();
    if body.confirm_operation_id != id
        || reason.len() < 3
        || reason.len() > 1024
        || reason.chars().any(char::is_control)
    {
        return from_service_error(ServiceError::InvalidInput(
            "Confirm the exact operation ID and provide a bounded review reason".into(),
        ));
    }
    let op = match membership_store::get(&state.db, &tenant, &id) {
        Ok(op) => op,
        Err(e) => return from_service_error(e),
    };
    if op.state != "unknown_result"
        || op.binding_id.as_deref() != Some(body.expected_binding_id.as_str())
        || op.updated_at != body.expected_updated_at
        || !membership_store::valid_binding_id(&body.expected_binding_id)
    {
        return from_service_error(ServiceError::Conflict("Operation changed or no durable binding ID is available; do not infer ownership from a matching username or role".into()));
    }
    let realm = match state.services.rancher.identity_realm() {
        Ok(realm) => realm,
        Err(e) => return from_service_error(e),
    };
    if realm != op.realm {
        return from_service_error(ServiceError::Conflict(
            "Provider realm changed since the operation".into(),
        ));
    }
    if let Err(e) = current_project(&state, &tenant, &op.project_id, false) {
        return from_service_error(e);
    }
    let bindings = match state.services.rancher.list_prtb(&op.project_id).await {
        Ok(bindings) => bindings,
        Err(_) => {
            return from_service_error(ServiceError::upstream(
                "rancher",
                "Reconciliation inventory could not be verified",
            ))
        }
    };
    if bindings.iter().any(|b| b.project_id != op.project_id) {
        return from_service_error(ServiceError::upstream(
            "rancher",
            "Reconciliation project scope could not be verified",
        ));
    }
    let matching = bindings.iter().find(|b| b.id == body.expected_binding_id);
    let confirmed = match op.operation.as_str() {
        "create" => matching
            .is_some_and(|b| b.user_id == op.remote_user_id && b.role_template == op.role_template),
        "delete" => matching.is_none(),
        _ => false,
    };
    if !confirmed {
        return from_service_error(ServiceError::Conflict("Current provider inventory does not confirm the recorded outcome; no mutation was replayed".into()));
    }
    let observed_at = chrono::Utc::now().to_rfc3339();
    // Recheck authority and tenant mapping after upstream IO, before local commit.
    if let Err(r) = current_writer(&req, &users, &sessions) {
        return r;
    }
    if let Err(e) = current_project(&state, &tenant, &op.project_id, false) {
        return from_service_error(e);
    }
    let actor = match current_user(&req) {
        Ok(actor) => actor,
        Err(r) => return r,
    };
    // Keep local revocation from interleaving with the final metadata commit.
    // These guards span no upstream IO and follow the identity-link lock order.
    let current_users = users.lock().unwrap();
    let active_sessions = sessions.lock().unwrap();
    let Some(claims) = crate::auth::verified_claims(&req) else {
        return HttpResponse::Unauthorized().finish();
    };
    let now = chrono::Utc::now();
    let valid_session = active_sessions.get(&claims.sid).is_some_and(|session| {
        session.username == claims.sub
            && Some(session.user_id.as_str()) == crate::auth::verified_user_id(&req).as_deref()
            && chrono::NaiveDateTime::parse_from_str(&session.created_at, "%Y-%m-%d %H:%M:%S")
                .ok()
                .is_some_and(|created| {
                    let age = now.signed_duration_since(created.and_utc());
                    age >= chrono::Duration::zero() && age < chrono::Duration::days(7)
                })
    });
    if !valid_session || claims.exp as u128 <= now.timestamp().max(0) as u128 {
        return HttpResponse::Unauthorized().finish();
    }
    if !current_users.iter().any(|user| {
        user.username == claims.sub
            && Some(user.id.as_str()) == crate::auth::verified_user_id(&req).as_deref()
            && user.active()
            && user.super_admin()
    }) {
        return HttpResponse::Forbidden().finish();
    }
    let review = membership_store::Reconciliation {
        actor: &actor,
        reason,
        observed_at: &observed_at,
        source_ip: super::super::util::source_ip(&req),
        user_agent: super::super::util::user_agent(&req),
    };
    if let Err(error) = membership_store::reconcile_complete(&state.db, &op, review) {
        return match error {
            ServiceError::Conflict(message) => from_service_error(ServiceError::Conflict(message)),
            _ => from_service_error(ServiceError::Internal("Reconciliation commit was not confirmed by this request; refresh the operation before another review".into())),
        };
    }
    let mut response = super::super::util::ok_response(
        serde_json::json!({"operation_id":op.id,"state":"completed","observed_at":observed_at,"evidence_scope":"direct_project_binding_only","provider_write_replayed":false}),
    );
    response.headers_mut().insert(
        actix_web::http::header::CACHE_CONTROL,
        actix_web::http::header::HeaderValue::from_static("no-store"),
    );
    response
}
