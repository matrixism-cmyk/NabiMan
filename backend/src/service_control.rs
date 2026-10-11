//! API authentication remains here; the root helper independently restricts its peer and actions.
use crate::{
    auth,
    jwt_sessions::SessionStore,
    models::{ApiResponse, ServiceActionRequest},
    terminal_tickets::ConnectionGrant,
    users::UserStore,
};
use actix_web::{http::StatusCode, web, HttpRequest, HttpResponse};
use nabiman_server::privileged::{
    client::{Client, ClientError},
    policy::canonical_unit,
    protocol::{self, Operation, Receipt, Request, ServiceAction, State, Status, VERSION},
};
use serde::Serialize;

#[derive(Serialize)]
struct ControlResult {
    operation_id: String,
    receipt: Option<Receipt>,
}
fn result(
    status: StatusCode,
    success: bool,
    message: &str,
    id: String,
    receipt: Option<Receipt>,
) -> HttpResponse {
    HttpResponse::build(status)
        .insert_header(("Cache-Control", "no-store"))
        .json(ApiResponse {
            success,
            message: message.into(),
            data: Some(ControlResult {
                operation_id: id,
                receipt,
            }),
        })
}
async fn dispatch(
    req: HttpRequest,
    operation: Operation,
    id: String,
    client: web::Data<Client>,
    users: web::Data<UserStore>,
    sessions: web::Data<SessionStore>,
) -> HttpResponse {
    let Some(actor_id) = auth::verified_user_id(&req) else {
        return HttpResponse::Unauthorized().finish();
    };
    let Some(grant) =
        ConnectionGrant::from_request(&req, sessions.get_ref().clone(), users.get_ref().clone())
    else {
        return HttpResponse::Unauthorized().finish();
    };
    let inspect = matches!(operation, Operation::Inspect { .. });
    let method = if inspect { "GET" } else { "POST" };
    let operation_id = match &operation {
        Operation::Inspect { operation_id } => operation_id.clone(),
        _ => id.clone(),
    };
    if unsafe { libc::geteuid() } == 0 {
        return result(
            StatusCode::SERVICE_UNAVAILABLE,
            false,
            "Service control requires a reviewed unprivileged API runtime",
            operation_id,
            None,
        );
    }
    let request = Request {
        version: VERSION,
        request_id: id,
        actor_id,
        expires_at: chrono::Utc::now().timestamp() + 3,
        operation,
    };
    let current = grant.clone();
    let outcome = web::block(move || {
        if !current.valid_for_method(method) {
            return None;
        }
        Some(client.request(&request))
    })
    .await;
    if !grant.valid_for_method(method) {
        return HttpResponse::Unauthorized().finish();
    }
    let reply=match outcome {
        Ok(Some(Ok(reply)))=>reply,
        Ok(None)=>return HttpResponse::Unauthorized().finish(),
        Ok(Some(Err(ClientError::Unavailable)))=>return result(StatusCode::SERVICE_UNAVAILABLE,false,"Service helper unavailable; no direct execution fallback",operation_id,None),
        _=>return result(StatusCode::BAD_GATEWAY,false,"Helper response unavailable. Inspect this operation before another change; do not replay",operation_id,None),
    };
    let id = reply
        .receipt
        .as_ref()
        .map_or(operation_id, |r| r.operation_id.clone());
    match reply.status {
        Status::Rejected => result(
            StatusCode::FORBIDDEN,
            false,
            "Helper policy denied this operation",
            id,
            None,
        ),
        Status::Expired => result(
            StatusCode::CONFLICT,
            false,
            "Request expired before helper dispatch",
            id,
            None,
        ),
        Status::NotFound => result(
            StatusCode::NOT_FOUND,
            false,
            "Operation not found for this account",
            id,
            None,
        ),
        Status::Conflict => result(
            StatusCode::CONFLICT,
            false,
            "Operation already recorded or unit has an unresolved result; no command replay",
            id,
            reply.receipt,
        ),
        Status::Completed if inspect => result(
            StatusCode::OK,
            true,
            "Recorded command outcome; service health requires separate verification",
            id,
            reply.receipt,
        ),
        Status::Completed => {
            let state = reply.receipt.as_ref().map(|r| r.state);
            match state {
                Some(State::CommandSucceeded) => result(
                    StatusCode::OK,
                    true,
                    "Service control command completed; verify service state",
                    id,
                    reply.receipt,
                ),
                Some(State::UnknownResult | State::Running) => result(
                    StatusCode::ACCEPTED,
                    false,
                    "Command outcome unknown; inspect before another change",
                    id,
                    reply.receipt,
                ),
                _ => result(
                    StatusCode::BAD_GATEWAY,
                    false,
                    "Service control command failed or was not dispatched; inspect the receipt",
                    id,
                    reply.receipt,
                ),
            }
        }
    }
}
pub(crate) async fn action(
    req: HttpRequest,
    body: web::Json<ServiceActionRequest>,
    client: web::Data<Client>,
    users: web::Data<UserStore>,
    sessions: web::Data<SessionStore>,
) -> HttpResponse {
    let Some(unit) = canonical_unit(&body.name) else {
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error("Invalid service name"));
    };
    let action: ServiceAction =
        match serde_json::from_value(serde_json::Value::String(body.action.clone())) {
            Ok(value) => value,
            Err(_) => {
                return HttpResponse::BadRequest()
                    .json(ApiResponse::<()>::error("Invalid service action"))
            }
        };
    let id = match req.headers().get("Idempotency-Key") {
        Some(value) => match value.to_str().ok().filter(|v| protocol::is_id(v)) {
            Some(value) => value.into(),
            None => {
                return HttpResponse::BadRequest().json(ApiResponse::<()>::error(
                    "Idempotency-Key must be a canonical UUID",
                ))
            }
        },
        None => uuid::Uuid::new_v4().to_string(),
    };
    dispatch(
        req,
        Operation::Execute { unit, action },
        id,
        client,
        users,
        sessions,
    )
    .await
}
pub(crate) async fn inspect(
    req: HttpRequest,
    path: web::Path<String>,
    client: web::Data<Client>,
    users: web::Data<UserStore>,
    sessions: web::Data<SessionStore>,
) -> HttpResponse {
    let id = path.into_inner();
    if !protocol::is_id(&id) {
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error("Invalid operation ID"));
    }
    dispatch(
        req,
        Operation::Inspect { operation_id: id },
        uuid::Uuid::new_v4().to_string(),
        client,
        users,
        sessions,
    )
    .await
}
