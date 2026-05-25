use crate::mec::audit_logger::AuditBuilder;
use crate::mec::MecState;
use crate::models::mec::AuditStatus;
use actix_web::{web, HttpRequest};
use serde_json::Value;

/// Create an audit builder pre-populated with source IP and user agent from
/// the HTTP request. Callers must finalize with `.success(...)` or `.failure(...)`.
pub fn begin_audit(
    req: &HttpRequest,
    state: &web::Data<MecState>,
    user: &str,
    action: &str,
    resource_type: &str,
    resource_id: &str,
) -> AuditBuilder {
    state
        .audit
        .begin(user, action, resource_type, resource_id)
        .with_source_ip(super::util::source_ip(req))
        .with_user_agent(super::util::user_agent(req))
}

/// Helper for the common "finalize with one-shot success/failure" pattern.
pub fn finalize<T, E>(
    builder: AuditBuilder,
    result: Result<T, E>,
    success_payload: impl FnOnce(&T) -> Option<Value>,
) -> Result<T, E>
where
    E: ToString,
{
    match result {
        Ok(v) => {
            let payload = success_payload(&v);
            builder.complete(AuditStatus::Success, payload, None);
            Ok(v)
        }
        Err(e) => {
            let msg = e.to_string();
            builder.complete(AuditStatus::Failed, None, Some(msg));
            Err(e)
        }
    }
}
