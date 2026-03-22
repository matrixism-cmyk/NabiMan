use actix_web::{web, HttpResponse};
use crate::models::{ApiResponse, CronJob, CreateCronRequest, DeleteCronRequest};

fn validate_username(name: &str) -> bool {
    !name.is_empty()
        && name.len() < 64
        && name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_')
}

fn list_user_cron(user: &str) -> Result<Vec<CronJob>, String> {
    let output = std::process::Command::new("crontab")
        .args(["-l", "-u", user])
        .output()
        .map_err(|e| format!("Failed to run crontab: {}", e))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr).to_string();
        if err.contains("no crontab for") {
            return Ok(Vec::new());
        }
        return Err(err);
    }

    let content = String::from_utf8_lossy(&output.stdout);
    let mut jobs = Vec::new();
    let mut id = 1u32;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = trimmed.splitn(6, ' ').collect();
        if parts.len() >= 6 {
            jobs.push(CronJob {
                id,
                user: user.to_string(),
                schedule: parts[..5].join(" "),
                command: parts[5].to_string(),
            });
            id += 1;
        }
    }
    Ok(jobs)
}

async fn get_cron_jobs() -> HttpResponse {
    // Get cron jobs for common users: root + system users with crontabs
    let mut all_jobs = Vec::new();

    // Try to list all user crontabs from /var/spool/cron
    let spool_dirs = ["/var/spool/cron/crontabs", "/var/spool/cron"];
    for dir in &spool_dirs {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                if let Some(user) = entry.file_name().to_str() {
                    if validate_username(user) {
                        if let Ok(jobs) = list_user_cron(user) {
                            all_jobs.extend(jobs);
                        }
                    }
                }
            }
            if !all_jobs.is_empty() {
                break;
            }
        }
    }

    // Fallback: at least try root
    if all_jobs.is_empty() {
        if let Ok(jobs) = list_user_cron("root") {
            all_jobs.extend(jobs);
        }
    }

    HttpResponse::Ok().json(ApiResponse::ok(all_jobs))
}

async fn add_cron_job(body: web::Json<CreateCronRequest>) -> HttpResponse {
    if !validate_username(&body.user) {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Invalid username"));
    }

    // Validate schedule: basic check for 5 fields
    let schedule_parts: Vec<&str> = body.schedule.split_whitespace().collect();
    if schedule_parts.len() != 5 {
        return HttpResponse::Ok().json(ApiResponse::<String>::error(
            "Invalid schedule. Expected 5 fields: min hour dom month dow",
        ));
    }
    for part in &schedule_parts {
        if part.contains(|c: char| !c.is_alphanumeric() && c != '*' && c != '/' && c != '-' && c != ',') {
            return HttpResponse::Ok().json(ApiResponse::<String>::error("Invalid schedule characters"));
        }
    }

    // Reject dangerous shell metacharacters in command
    if body.command.contains('`') || body.command.contains("$(") {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Command contains unsafe characters"));
    }

    // Get existing crontab
    let existing = std::process::Command::new("crontab")
        .args(["-l", "-u", &body.user])
        .output()
        .map(|o| {
            if o.status.success() {
                String::from_utf8_lossy(&o.stdout).to_string()
            } else {
                String::new()
            }
        })
        .unwrap_or_default();

    let new_line = format!("{} {}", body.schedule, body.command);
    let new_content = if existing.trim().is_empty() {
        format!("{}\n", new_line)
    } else {
        format!("{}\n{}\n", existing.trim(), new_line)
    };

    // Write via pipe to crontab
    let mut child = std::process::Command::new("crontab")
        .args(["-u", &body.user, "-"])
        .stdin(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn crontab: {}", e));

    match child {
        Ok(ref mut c) => {
            use std::io::Write;
            if let Some(ref mut stdin) = c.stdin {
                let _ = stdin.write_all(new_content.as_bytes());
            }
            match c.wait() {
                Ok(status) if status.success() => {
                    HttpResponse::Ok().json(ApiResponse::ok("Cron job added".to_string()))
                }
                _ => HttpResponse::Ok().json(ApiResponse::<String>::error("Failed to set crontab")),
            }
        }
        Err(e) => HttpResponse::Ok().json(ApiResponse::<String>::error(&e)),
    }
}

async fn delete_cron_job(body: web::Json<DeleteCronRequest>) -> HttpResponse {
    if !validate_username(&body.user) {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Invalid username"));
    }

    let output = std::process::Command::new("crontab")
        .args(["-l", "-u", &body.user])
        .output();

    let existing = match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
        _ => return HttpResponse::Ok().json(ApiResponse::<String>::error("No crontab found")),
    };

    let mut job_lines = Vec::new();
    let mut other_lines = Vec::new();
    for line in existing.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            other_lines.push(line.to_string());
        } else {
            job_lines.push(line.to_string());
        }
    }

    let idx = (body.id as usize).saturating_sub(1);
    if idx >= job_lines.len() {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Invalid job ID"));
    }
    job_lines.remove(idx);

    let mut new_content = other_lines.join("\n");
    if !new_content.is_empty() {
        new_content.push('\n');
    }
    new_content.push_str(&job_lines.join("\n"));
    if !job_lines.is_empty() {
        new_content.push('\n');
    }

    let mut child = match std::process::Command::new("crontab")
        .args(["-u", &body.user, "-"])
        .stdin(std::process::Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return HttpResponse::Ok().json(ApiResponse::<String>::error(&format!("Failed: {}", e))),
    };

    use std::io::Write;
    if let Some(ref mut stdin) = child.stdin {
        let _ = stdin.write_all(new_content.as_bytes());
    }

    match child.wait() {
        Ok(status) if status.success() => {
            HttpResponse::Ok().json(ApiResponse::ok("Cron job deleted".to_string()))
        }
        _ => HttpResponse::Ok().json(ApiResponse::<String>::error("Failed to update crontab")),
    }
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/cron")
            .route("", web::get().to(get_cron_jobs))
            .route("/add", web::post().to(add_cron_job))
            .route("/delete", web::post().to(delete_cron_job)),
    );
}
