use actix_web::{web, HttpResponse};
use crate::models::ApiResponse;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Clone)]
pub struct LoginSession {
    pub user: String,
    pub tty: String,
    pub from: String,
    pub login_time: String,
    pub idle: String,
    pub what: String,
}

async fn list_sessions() -> HttpResponse {
    let output = std::process::Command::new("w").arg("-h").output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();

    let sessions: Vec<LoginSession> = output.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            LoginSession {
                user: parts.first().unwrap_or(&"").to_string(),
                tty: parts.get(1).unwrap_or(&"").to_string(),
                from: parts.get(2).unwrap_or(&"").to_string(),
                login_time: parts.get(3).unwrap_or(&"").to_string(),
                idle: parts.get(4).unwrap_or(&"").to_string(),
                what: parts.get(5..).map(|p| p.join(" ")).unwrap_or_default(),
            }
        })
        .collect();

    HttpResponse::Ok().json(ApiResponse::ok(sessions))
}

#[derive(Deserialize)]
pub struct KillSessionRequest {
    pub tty: String,
}

async fn kill_session(body: web::Json<KillSessionRequest>) -> HttpResponse {
    let tty = &body.tty;
    if tty.is_empty() || tty.contains(|c: char| !c.is_alphanumeric() && c != '/') {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Invalid TTY"));
    }

    // Find PIDs for this TTY
    let output = std::process::Command::new("fuser")
        .arg(format!("/dev/{}", tty)).output();

    match output {
        Ok(o) => {
            let pids = String::from_utf8_lossy(&o.stdout).to_string();
            if pids.trim().is_empty() {
                return HttpResponse::Ok().json(ApiResponse::<String>::error("No process found"));
            }
            for pid in pids.split_whitespace() {
                if let Ok(p) = pid.trim().parse::<u32>() {
                    let _ = std::process::Command::new("kill").arg("-9")
                        .arg(p.to_string()).output();
                }
            }
            HttpResponse::Ok().json(ApiResponse::ok(format!("Session {} terminated", tty)))
        }
        Err(e) => HttpResponse::Ok().json(ApiResponse::<String>::error(&e.to_string())),
    }
}

async fn last_logins() -> HttpResponse {
    let output = std::process::Command::new("last")
        .args(["-n", "20", "-w"]).output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();

    HttpResponse::Ok().json(ApiResponse::ok(output))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/sessions")
            .route("", web::get().to(list_sessions))
            .route("/kill", web::post().to(kill_session))
            .route("/last", web::get().to(last_logins))
    );
}
