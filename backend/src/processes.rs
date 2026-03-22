use actix_web::{web, HttpResponse};
use crate::models::{ApiResponse, ProcessInfo, KillProcessRequest};

async fn list_processes() -> HttpResponse {
    let output = match std::process::Command::new("ps")
        .args(["aux", "--sort=-%cpu"])
        .output()
    {
        Ok(o) => o,
        Err(e) => return HttpResponse::Ok().json(ApiResponse::<Vec<ProcessInfo>>::error(&format!("Failed: {}", e))),
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let processes: Vec<ProcessInfo> = stdout
        .lines()
        .skip(1) // skip header
        .take(200) // limit
        .filter_map(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 11 {
                Some(ProcessInfo {
                    user: parts[0].to_string(),
                    pid: parts[1].parse().unwrap_or(0),
                    cpu: parts[2].parse().unwrap_or(0.0),
                    memory: parts[3].parse().unwrap_or(0.0),
                    vsz: parts[4].parse().unwrap_or(0),
                    rss: parts[5].parse().unwrap_or(0),
                    started: parts[8].to_string(),
                    command: parts[10..].join(" "),
                })
            } else {
                None
            }
        })
        .collect();

    HttpResponse::Ok().json(ApiResponse::ok(processes))
}

async fn kill_process(body: web::Json<KillProcessRequest>) -> HttpResponse {
    if body.pid == 0 || body.pid == 1 {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Cannot kill PID 0 or 1"));
    }

    let signal = body.signal.as_deref().unwrap_or("TERM");
    // Validate signal
    let valid_signals = ["TERM", "KILL", "HUP", "INT", "STOP", "CONT",
        "9", "15", "1", "2", "19", "18"];
    if !valid_signals.contains(&signal) {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Invalid signal"));
    }

    let pid_str = body.pid.to_string();
    match std::process::Command::new("kill")
        .args([&format!("-{}", signal), &pid_str])
        .output()
    {
        Ok(output) if output.status.success() => {
            HttpResponse::Ok().json(ApiResponse::ok(format!("Signal {} sent to PID {}", signal, body.pid)))
        }
        Ok(output) => {
            let err = String::from_utf8_lossy(&output.stderr).to_string();
            HttpResponse::Ok().json(ApiResponse::<String>::error(&err))
        }
        Err(e) => HttpResponse::Ok().json(ApiResponse::<String>::error(&format!("Failed: {}", e))),
    }
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/processes")
            .route("", web::get().to(list_processes))
            .route("/kill", web::post().to(kill_process)),
    );
}
