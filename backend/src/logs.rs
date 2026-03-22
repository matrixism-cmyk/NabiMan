use actix_web::{web, HttpResponse};
use crate::models::{ApiResponse, LogEntry, LogQueryRequest};

fn run_cmd(cmd: &str, args: &[&str]) -> Result<String, String> {
    let output = std::process::Command::new(cmd)
        .args(args)
        .output()
        .map_err(|e| format!("Failed to run {}: {}", cmd, e))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

fn validate_unit(unit: &str) -> bool {
    !unit.is_empty()
        && unit.len() < 256
        && unit.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '@')
}

fn validate_priority(p: &str) -> bool {
    matches!(p, "emerg" | "alert" | "crit" | "err" | "warning" | "notice" | "info" | "debug"
        | "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7")
}

async fn get_logs(body: web::Json<LogQueryRequest>) -> HttpResponse {
    let lines = body.lines.unwrap_or(100).min(1000);
    let mut args: Vec<String> = vec![
        "--no-pager".to_string(),
        "-o".to_string(),
        "short-iso".to_string(),
        "-n".to_string(),
        lines.to_string(),
    ];

    if let Some(ref unit) = body.unit {
        if !validate_unit(unit) {
            return HttpResponse::Ok().json(ApiResponse::<Vec<LogEntry>>::error("Invalid unit name"));
        }
        args.push("-u".to_string());
        args.push(unit.clone());
    }

    if let Some(ref priority) = body.priority {
        if !validate_priority(priority) {
            return HttpResponse::Ok().json(ApiResponse::<Vec<LogEntry>>::error("Invalid priority"));
        }
        args.push("-p".to_string());
        args.push(priority.clone());
    }

    if let Some(ref since) = body.since {
        // Validate basic date format (YYYY-MM-DD or similar)
        if since.len() > 30 || since.contains(|c: char| !c.is_alphanumeric() && c != '-' && c != ':' && c != ' ' && c != 'T') {
            return HttpResponse::Ok().json(ApiResponse::<Vec<LogEntry>>::error("Invalid since format"));
        }
        args.push("--since".to_string());
        args.push(since.clone());
    }

    let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    match run_cmd("journalctl", &args_ref) {
        Ok(output) => {
            let entries: Vec<LogEntry> = output
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(|line| {
                    // Format: 2024-01-15T10:30:00+0900 hostname unit[pid]: message
                    let parts: Vec<&str> = line.splitn(4, ' ').collect();
                    if parts.len() >= 4 {
                        LogEntry {
                            timestamp: parts[0].to_string(),
                            unit: parts[2].trim_end_matches(':').to_string(),
                            priority: String::new(),
                            message: parts[3..].join(" "),
                        }
                    } else {
                        LogEntry {
                            timestamp: String::new(),
                            unit: String::new(),
                            priority: String::new(),
                            message: line.to_string(),
                        }
                    }
                })
                .collect();
            HttpResponse::Ok().json(ApiResponse::ok(entries))
        }
        Err(e) => HttpResponse::Ok().json(ApiResponse::<Vec<LogEntry>>::error(&e)),
    }
}

async fn list_units() -> HttpResponse {
    match run_cmd("journalctl", &["--no-pager", "-F", "_SYSTEMD_UNIT"]) {
        Ok(output) => {
            let units: Vec<String> = output
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| l.trim().to_string())
                .collect();
            HttpResponse::Ok().json(ApiResponse::ok(units))
        }
        Err(e) => HttpResponse::Ok().json(ApiResponse::<Vec<String>>::error(&e)),
    }
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/logs")
            .route("", web::post().to(get_logs))
            .route("/units", web::get().to(list_units)),
    );
}
