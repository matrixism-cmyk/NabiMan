use actix_web::{web, HttpResponse};
use crate::models::{ApiResponse, SystemService, ServiceActionRequest};

fn run_cmd(cmd: &str, args: &[&str]) -> Result<String, String> {
    let output = std::process::Command::new(cmd)
        .args(args)
        .output()
        .map_err(|e| format!("Failed to run {}: {}", cmd, e))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Err(if stderr.is_empty() { stdout } else { stderr })
    }
}

fn validate_service_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() < 256
        && name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '@')
}

async fn list_services() -> HttpResponse {
    let output = match run_cmd("systemctl", &[
        "list-units", "--type=service", "--all", "--no-pager", "--no-legend",
    ]) {
        Ok(o) => o,
        Err(e) => return HttpResponse::Ok().json(ApiResponse::<Vec<SystemService>>::error(&e)),
    };

    let enabled_output = run_cmd("systemctl", &[
        "list-unit-files", "--type=service", "--no-pager", "--no-legend",
    ])
    .unwrap_or_default();

    let enabled_map: std::collections::HashMap<String, bool> = enabled_output
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let name = parts[0].trim_end_matches(".service").to_string();
                let enabled = parts[1] == "enabled";
                Some((name, enabled))
            } else {
                None
            }
        })
        .collect();

    let services: Vec<SystemService> = output
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 {
                let full_name = parts[0].to_string();
                let name = full_name.trim_end_matches(".service").to_string();
                let load_state = parts[1].to_string();
                let active_state = parts[2].to_string();
                let sub_state = parts[3].to_string();
                let description = if parts.len() > 4 {
                    parts[4..].join(" ")
                } else {
                    String::new()
                };
                let enabled = enabled_map.get(&name).copied().unwrap_or(false);
                Some(SystemService {
                    name,
                    description,
                    load_state,
                    active_state,
                    sub_state,
                    enabled,
                })
            } else {
                None
            }
        })
        .collect();

    HttpResponse::Ok().json(ApiResponse::ok(services))
}

async fn service_action(body: web::Json<ServiceActionRequest>) -> HttpResponse {
    if !validate_service_name(&body.name) {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Invalid service name"));
    }

    let valid_actions = ["start", "stop", "restart", "enable", "disable"];
    if !valid_actions.contains(&body.action.as_str()) {
        return HttpResponse::Ok().json(ApiResponse::<String>::error(
            "Invalid action. Use: start, stop, restart, enable, disable",
        ));
    }

    match run_cmd("systemctl", &[&body.action, &body.name]) {
        Ok(_) => HttpResponse::Ok().json(ApiResponse::ok(format!(
            "Service {} {}ed successfully",
            body.name, body.action
        ))),
        Err(e) => HttpResponse::Ok().json(ApiResponse::<String>::error(&e)),
    }
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/services")
            .route("", web::get().to(list_services))
            .route("/action", web::post().to(service_action)),
    );
}
