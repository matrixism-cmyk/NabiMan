use super::*;

pub(super) fn no_store(mut r: HttpResponse) -> HttpResponse {
    r.headers_mut().insert(
        actix_web::http::header::CACHE_CONTROL,
        actix_web::http::header::HeaderValue::from_static("no-store"),
    );
    r
}
pub async fn inventory(
    req: HttpRequest,
    path: web::Path<String>,
    state: web::Data<MecState>,
    users: web::Data<UserStore>,
    sessions: web::Data<SessionStore>,
) -> HttpResponse {
    if let Err(r) = super::super::handlers::members::current_writer(&req, &users, &sessions) {
        return r;
    }
    match list(&state.db, Some(&path)) {
        Ok(rows) => no_store(super::super::handlers::util::list_response(rows)),
        Err(e) => super::super::handlers::util::from_service_error(e),
    }
}
#[derive(Serialize)]
pub struct PendingManagedGrant {
    pub operation: membership_store::Operation,
    pub local_user_id: String,
    pub source_role: String,
}
pub(super) fn pending(
    db: &DbHandle,
    tenant: Option<&str>,
) -> ServiceResult<Vec<PendingManagedGrant>> {
    let conn = db.lock().unwrap();
    let mut query=conn.prepare("SELECT p.operation_id,o.tenant_id,p.local_user_id,p.source_role FROM requested_managed_grants p JOIN membership_operations o ON o.id=p.operation_id WHERE o.state='unknown_result' AND (?1 IS NULL OR o.tenant_id=?1) ORDER BY o.created_at,o.id")?;
    let ids = query
        .query_map(params![tenant], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    drop(query);
    drop(conn);
    let mut records = Vec::new();
    for (id, tenant, local, role) in ids {
        let operation = membership_store::get(db, &tenant, &id)?;
        // Completion may happen between the ID query and the snapshot read.
        if operation.state == "unknown_result" {
            records.push(PendingManagedGrant {
                operation,
                local_user_id: local,
                source_role: role,
            });
        }
    }
    Ok(records)
}
pub async fn pending_inventory(
    req: HttpRequest,
    path: web::Path<String>,
    state: web::Data<MecState>,
    users: web::Data<UserStore>,
    sessions: web::Data<SessionStore>,
) -> HttpResponse {
    if let Err(r) = super::super::handlers::members::current_writer(&req, &users, &sessions) {
        return r;
    }
    match pending(&state.db, Some(&path)) {
        Ok(rows) => no_store(super::super::handlers::util::list_response(rows)),
        Err(e) => super::super::handlers::util::from_service_error(e),
    }
}
