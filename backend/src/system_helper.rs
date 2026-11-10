//! The API's side of the helper's system operations, used in service mode
//! (`run_mode`): firewall rules and package updates go to the root helper
//! instead of running here. Answers keep the shapes the root-mode handlers
//! give, so the web app reads them the same way.
use crate::models::{ApiResponse, FirewallAddRuleRequest, FirewallDeleteRuleRequest, FirewallStatus};
use actix_web::{http::StatusCode, web, HttpRequest, HttpResponse};
use nabiman_server::privileged::{
    client::{Client, ClientError},
    protocol::{Operation, Outcome, Protocol, Request, State, Status, Verdict, VERSION},
};

fn refusal(status: StatusCode, message: &str) -> HttpResponse {
    HttpResponse::build(status).insert_header(("Cache-Control", "no-store")).json(ApiResponse::<()>::error(message))
}

/// Asks the helper; `Err` is the answer to give when it did not complete.
pub(crate) async fn call(req: &HttpRequest, client: &web::Data<Client>, operation: Operation) -> Result<Outcome, HttpResponse> {
    let Some(actor_id) = crate::auth::verified_user_id(req) else {
        return Err(HttpResponse::Unauthorized().finish());
    };
    let request = Request {
        version: VERSION,
        request_id: uuid::Uuid::new_v4().to_string(),
        actor_id,
        expires_at: chrono::Utc::now().timestamp() + 3,
        operation,
    };
    let client = client.clone();
    let reply = match web::block(move || client.request_system(&request)).await {
        Ok(Ok(reply)) => reply,
        Ok(Err(ClientError::Unavailable)) => {
            return Err(refusal(StatusCode::SERVICE_UNAVAILABLE, "The privileged helper is not running or not reachable; nothing was changed (no direct fallback in service mode)"))
        }
        _ => {
            return Err(refusal(StatusCode::BAD_GATEWAY, "The helper's answer was lost; the outcome is unknown. Check the state before trying again"))
        }
    };
    let reason = reply.reason.as_deref().unwrap_or("");
    match (reply.status, reply.outcome) {
        (Status::Completed, Some(outcome)) => Ok(outcome),
        (Status::Rejected, _) => Err(refusal(StatusCode::FORBIDDEN, &format!("The helper refused this operation: {reason}"))),
        (Status::Expired, _) => Err(refusal(StatusCode::CONFLICT, "The request expired before the helper took it; nothing was run")),
        (Status::Conflict, _) => Err(refusal(StatusCode::CONFLICT, reason)),
        _ => Err(refusal(StatusCode::BAD_GATEWAY, "Unexpected helper answer; the outcome is unknown")),
    }
}

/// The command's text when it did not succeed, as the root handlers report errors.
fn failed(outcome: &Outcome) -> Option<String> {
    match outcome.state {
        State::CommandSucceeded => None,
        State::NotDispatched if outcome.output.is_empty() => Some("Not run: the helper's approval or the API's identity changed".into()),
        State::NotDispatched => Some(outcome.output.trim().to_string()),
        State::UnknownResult | State::Running => Some("The command did not finish in time; its outcome is unknown".into()),
        State::CommandFailed => Some(outcome.output.trim().to_string()),
    }
}

pub(crate) async fn firewall_status(req: HttpRequest, client: web::Data<Client>) -> HttpResponse {
    let outcome = match call(&req, &client, Operation::FirewallStatus {}).await {
        Ok(outcome) => outcome,
        Err(answer) => return answer,
    };
    let Some(backend) = outcome.backend.clone().filter(|_| failed(&outcome).is_none()) else {
        let why = failed(&outcome).unwrap_or_default();
        return HttpResponse::Ok().json(ApiResponse::<FirewallStatus>::error(if why.is_empty() { "No firewall backend found" } else { &why }));
    };
    let text = &outcome.output;
    let (active, rules) = match backend.as_str() {
        "ufw" => (text.contains("Status: active"), crate::firewall::parse_ufw_rules(text)),
        "firewalld" => (text.lines().next().is_some_and(|l| l.trim() == "running"), crate::firewall::parse_firewalld_ports(text)),
        _ => (true, crate::firewall::parse_iptables_rules(text)),
    };
    HttpResponse::Ok().json(ApiResponse::ok(FirewallStatus { backend, active, rules }))
}

/// `80`, `8000:8100`; `tcp`, `udp`, `any`; `allow`, `deny` (as the root handler reads them).
pub(crate) fn firewall_rule(body: &FirewallAddRuleRequest) -> Result<Operation, &'static str> {
    let protocol = match body.protocol.as_str() {
        "tcp" => Protocol::Tcp,
        "udp" => Protocol::Udp,
        "any" => Protocol::Any,
        _ => return Err("Invalid protocol (tcp/udp/any)"),
    };
    let (port, port_end) = match body.port.split_once(':') {
        Some((a, b)) => (a.parse().map_err(|_| "Invalid port")?, Some(b.parse().map_err(|_| "Invalid port")?)),
        None => (body.port.parse().map_err(|_| "Invalid port")?, None),
    };
    let verdict = if body.action == "deny" { Verdict::Deny } else { Verdict::Allow };
    Ok(Operation::FirewallAdd { verdict, protocol, port, port_end })
}

pub(crate) async fn firewall_add(req: HttpRequest, client: web::Data<Client>, body: &FirewallAddRuleRequest) -> HttpResponse {
    let operation = match firewall_rule(body) {
        Ok(operation) => operation,
        Err(why) => return HttpResponse::Ok().json(ApiResponse::<String>::error(why)),
    };
    let port_proto = if body.protocol == "any" { body.port.clone() } else { format!("{}/{}", body.port, body.protocol) };
    match call(&req, &client, operation).await {
        Ok(outcome) => match failed(&outcome) {
            None => HttpResponse::Ok().json(ApiResponse::ok(format!("Rule added: {} {}", body.action, port_proto))),
            Some(why) => HttpResponse::Ok().json(ApiResponse::<String>::error(&why)),
        },
        Err(answer) => answer,
    }
}

pub(crate) async fn firewall_delete(req: HttpRequest, client: web::Data<Client>, body: &FirewallDeleteRuleRequest) -> HttpResponse {
    match call(&req, &client, Operation::FirewallDelete { number: body.number }).await {
        Ok(outcome) => match failed(&outcome) {
            None => HttpResponse::Ok().json(ApiResponse::ok(format!("Rule {} deleted", body.number))),
            Some(why) => HttpResponse::Ok().json(ApiResponse::<String>::error(&why)),
        },
        Err(answer) => answer,
    }
}

/// `apt update` before a check: through the helper; a failure only means older lists.
pub(crate) async fn refresh_packages(req: &HttpRequest, client: &web::Data<Client>) {
    let _ = call(req, client, Operation::PackageRefresh {}).await;
}

/// A whole upgrade (`None`) or one package's, with the root handlers' answer.
pub(crate) async fn upgrade(req: HttpRequest, client: web::Data<Client>, package: Option<String>) -> HttpResponse {
    match call(&req, &client, Operation::PackageUpgrade { package }).await {
        Ok(outcome) => match failed(&outcome) {
            None => HttpResponse::Ok().json(ApiResponse::ok(outcome.output.trim().to_string())),
            Some(why) => HttpResponse::Ok().json(ApiResponse::<String>::error(&why)),
        },
        Err(answer) => answer,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(port: &str, protocol: &str, action: &str) -> Result<Operation, &'static str> {
        firewall_rule(&FirewallAddRuleRequest { port: port.into(), protocol: protocol.into(), action: action.into() })
    }

    #[test]
    fn a_rule_from_the_web_becomes_a_typed_operation() {
        assert!(matches!(rule("22", "tcp", "allow"), Ok(Operation::FirewallAdd { verdict: Verdict::Allow, protocol: Protocol::Tcp, port: 22, port_end: None })));
        assert!(matches!(rule("8000:8100", "udp", "deny"), Ok(Operation::FirewallAdd { verdict: Verdict::Deny, port: 8000, port_end: Some(8100), .. })));
        for (port, protocol) in [("22; reboot", "tcp"), ("-1", "tcp"), ("70000", "tcp"), ("22", "icmp"), ("1:2:3", "tcp")] {
            assert!(rule(port, protocol, "allow").is_err(), "{port} {protocol}");
        }
    }

    #[test]
    fn a_failure_reports_the_command_text_and_success_none() {
        let outcome = |state, output: &str| Outcome { state, exit_code: None, backend: Some("ufw".into()), output: output.into(), truncated: false };
        assert_eq!(failed(&outcome(State::CommandSucceeded, "ok")), None);
        assert_eq!(failed(&outcome(State::CommandFailed, " ERROR: bad port \n")).as_deref(), Some("ERROR: bad port"));
        assert!(failed(&outcome(State::UnknownResult, "")).unwrap().contains("unknown"));
        assert!(failed(&outcome(State::NotDispatched, "")).unwrap().contains("Not run"));
    }
}
