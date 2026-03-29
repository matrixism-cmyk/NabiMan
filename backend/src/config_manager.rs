use actix_web::{web, HttpResponse};
use std::fs;
use std::process::Command;
use crate::models::{ApiResponse, ServiceConfig, ServiceDefinition, UpdateConfigRequest};
use crate::service_registry::{service_registry, find_service, detect_service, check_service_running};

fn validate_service_id(id: &str) -> bool {
    !id.is_empty() && id.len() < 64
        && id.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_')
}

fn read_config(def: &ServiceDefinition) -> ServiceConfig {
    let mut config_path = String::new();
    let mut content = String::from("Configuration file not found");

    for path in &def.config_paths {
        if let Ok(c) = fs::read_to_string(path) {
            config_path = path.clone();
            content = c;
            break;
        }
    }

    ServiceConfig {
        service_name: def.id.clone(),
        config_path,
        content,
        is_running: check_service_running(def),
    }
}

// --- Handlers ---

async fn list_services() -> HttpResponse {
    let services: Vec<ServiceDefinition> = service_registry()
        .into_iter()
        .map(|mut def| { detect_service(&mut def); def })
        .collect();
    HttpResponse::Ok().json(ApiResponse::ok(services))
}

async fn get_config(path: web::Path<String>) -> HttpResponse {
    let service_id = path.into_inner();
    if !validate_service_id(&service_id) {
        return HttpResponse::Ok().json(ApiResponse::<ServiceConfig>::error("Invalid service ID"));
    }
    match find_service(&service_id) {
        Some(def) => HttpResponse::Ok().json(ApiResponse::ok(read_config(&def))),
        None => HttpResponse::Ok().json(ApiResponse::<ServiceConfig>::error("Service not registered")),
    }
}

async fn update_config(path: web::Path<String>, body: web::Json<UpdateConfigRequest>) -> HttpResponse {
    let service_id = path.into_inner();
    if !validate_service_id(&service_id) {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Invalid service ID"));
    }
    let def = match find_service(&service_id) {
        Some(d) => d,
        None => return HttpResponse::Ok().json(ApiResponse::<String>::error("Service not registered")),
    };

    let config_path = match def.config_paths.iter().find(|p| fs::metadata(p).is_ok()) {
        Some(p) => p.clone(),
        None => return HttpResponse::Ok().json(ApiResponse::<String>::error("Config file not found on disk")),
    };

    // Save timestamped version history
    let ts = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let history_dir = format!("{}.history", config_path);
    let _ = fs::create_dir_all(&history_dir);
    let version_path = format!("{}/{}.conf", history_dir, ts);
    let _ = fs::copy(&config_path, &version_path);

    // Also keep .bak for compatibility
    let backup_path = format!("{}.bak", config_path);
    if let Err(e) = fs::copy(&config_path, &backup_path) {
        return HttpResponse::Ok().json(ApiResponse::<String>::error(&format!("Backup failed: {}", e)));
    }

    match fs::write(&config_path, &body.content) {
        Ok(_) => HttpResponse::Ok().json(ApiResponse::ok(format!(
            "{} config updated (backup: {})", def.display_name, backup_path
        ))),
        Err(e) => {
            let _ = fs::copy(&backup_path, &config_path);
            HttpResponse::Ok().json(ApiResponse::<String>::error(&format!("Write failed: {}", e)))
        }
    }
}

async fn restart_service_handler(path: web::Path<String>) -> HttpResponse {
    let service_id = path.into_inner();
    if !validate_service_id(&service_id) {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Invalid service ID"));
    }
    let def = match find_service(&service_id) {
        Some(d) => d,
        None => return HttpResponse::Ok().json(ApiResponse::<String>::error("Service not registered")),
    };

    for svc in &def.systemd_names {
        if let Ok(result) = Command::new("systemctl").args(["restart", svc]).output() {
            if result.status.success() {
                return HttpResponse::Ok().json(ApiResponse::ok(format!("{} restarted", def.display_name)));
            }
        }
    }
    HttpResponse::Ok().json(ApiResponse::<String>::error(&format!("Failed to restart {}", def.display_name)))
}

async fn validate_config(path: web::Path<String>) -> HttpResponse {
    let service_id = path.into_inner();
    let def = match find_service(&service_id) {
        Some(d) => d,
        None => return HttpResponse::Ok().json(ApiResponse::<String>::error("Service not registered")),
    };

    let result = match service_id.as_str() {
        "apache" => try_commands(&[("httpd", &["-t"]), ("apache2ctl", &["configtest"])]),
        "nginx" => try_commands(&[("nginx", &["-t"])]),
        "mysql" => {
            if let Some(p) = def.config_paths.iter().find(|p| fs::metadata(p).is_ok()) {
                let arg = format!("--defaults-file={}", p);
                try_commands(&[("mysqld", &["--validate-config", &arg]), ("mariadbd", &["--validate-config", &arg])])
            } else { Err("Config file not found".into()) }
        }
        "sshd" => try_commands(&[("sshd", &["-t"])]),
        "php-fpm" => try_commands(&[("php-fpm", &["-t"])]),
        _ => Ok(format!("Config validation not available for {}", def.display_name)),
    };

    match result {
        Ok(msg) => HttpResponse::Ok().json(ApiResponse::ok(msg)),
        Err(e) => HttpResponse::Ok().json(ApiResponse::<String>::error(&e)),
    }
}

fn try_commands(cmds: &[(&str, &[&str])]) -> Result<String, String> {
    let mut last_err = String::from("No matching binary found");
    for (cmd, args) in cmds {
        match Command::new(cmd).args(*args).output() {
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                if output.status.success() {
                    let msg = if stderr.is_empty() { stdout } else { stderr };
                    return Ok(if msg.trim().is_empty() { "Syntax OK".into() } else { msg });
                }
                last_err = if stderr.is_empty() { stdout } else { stderr };
            }
            Err(_) => continue,
        }
    }
    Err(last_err)
}

// --- Version history, diff, rollback ---

use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct ConfigVersion { filename: String, timestamp: String, size: u64 }

async fn list_versions(path: web::Path<String>) -> HttpResponse {
    let service_id = path.into_inner();
    let def = match find_service(&service_id) {
        Some(d) => d,
        None => return HttpResponse::Ok().json(ApiResponse::<Vec<ConfigVersion>>::error("Service not found")),
    };
    let config_path = match def.config_paths.iter().find(|p| fs::metadata(p).is_ok()) {
        Some(p) => p.clone(),
        None => return HttpResponse::Ok().json(ApiResponse::ok(Vec::<ConfigVersion>::new())),
    };
    let history_dir = format!("{}.history", config_path);
    let mut versions = Vec::new();
    if let Ok(rd) = fs::read_dir(&history_dir) {
        for entry in rd.flatten() {
            let meta = match entry.metadata() { Ok(m) => m, Err(_) => continue };
            if !meta.is_file() { continue; }
            let name = entry.file_name().to_string_lossy().to_string();
            // Parse timestamp from filename (YYYYMMDD_HHMMSS.conf)
            let ts_part = name.trim_end_matches(".conf");
            let timestamp = if ts_part.len() >= 15 {
                format!("{}-{}-{} {}:{}:{}", &ts_part[0..4], &ts_part[4..6], &ts_part[6..8],
                    &ts_part[9..11], &ts_part[11..13], &ts_part[13..15])
            } else { ts_part.to_string() };
            versions.push(ConfigVersion { filename: name, timestamp, size: meta.len() });
        }
    }
    versions.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    HttpResponse::Ok().json(ApiResponse::ok(versions))
}

#[derive(Deserialize)]
struct DiffQuery { version: String }

#[derive(Serialize)]
struct DiffResult { version_content: String, current_content: String }

async fn get_diff(path: web::Path<String>, query: web::Query<DiffQuery>) -> HttpResponse {
    let service_id = path.into_inner();
    let def = match find_service(&service_id) {
        Some(d) => d,
        None => return HttpResponse::Ok().json(ApiResponse::<DiffResult>::error("Service not found")),
    };
    let config_path = match def.config_paths.iter().find(|p| fs::metadata(p).is_ok()) {
        Some(p) => p.clone(),
        None => return HttpResponse::Ok().json(ApiResponse::<DiffResult>::error("Config not found")),
    };
    let version_file = query.version.replace("..", "").replace('/', "");
    let version_path = format!("{}.history/{}", config_path, version_file);
    let current = fs::read_to_string(&config_path).unwrap_or_default();
    let version = fs::read_to_string(&version_path).unwrap_or_else(|_| "Version not found".into());
    HttpResponse::Ok().json(ApiResponse::ok(DiffResult { version_content: version, current_content: current }))
}

#[derive(Deserialize)]
struct RollbackRequest { version: String }

async fn rollback(path: web::Path<String>, body: web::Json<RollbackRequest>) -> HttpResponse {
    let service_id = path.into_inner();
    let def = match find_service(&service_id) {
        Some(d) => d,
        None => return HttpResponse::Ok().json(ApiResponse::<String>::error("Service not found")),
    };
    let config_path = match def.config_paths.iter().find(|p| fs::metadata(p).is_ok()) {
        Some(p) => p.clone(),
        None => return HttpResponse::Ok().json(ApiResponse::<String>::error("Config not found")),
    };
    let version_file = body.version.replace("..", "").replace('/', "");
    let version_path = format!("{}.history/{}", config_path, version_file);
    if !std::path::Path::new(&version_path).exists() {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Version not found"));
    }

    // Save current as new history entry before rollback
    let ts = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let pre_rollback = format!("{}.history/{}_pre_rollback.conf", config_path, ts);
    let _ = fs::copy(&config_path, &pre_rollback);

    match fs::copy(&version_path, &config_path) {
        Ok(_) => HttpResponse::Ok().json(ApiResponse::ok(format!("Rolled back to {}", version_file))),
        Err(e) => HttpResponse::Ok().json(ApiResponse::<String>::error(&e.to_string())),
    }
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/config")
            .route("/services", web::get().to(list_services))
            .route("/{service_id}", web::get().to(get_config))
            .route("/{service_id}", web::post().to(update_config))
            .route("/{service_id}/restart", web::post().to(restart_service_handler))
            .route("/{service_id}/validate", web::post().to(validate_config))
            .route("/{service_id}/versions", web::get().to(list_versions))
            .route("/{service_id}/diff", web::get().to(get_diff))
            .route("/{service_id}/rollback", web::post().to(rollback)),
    );
}
