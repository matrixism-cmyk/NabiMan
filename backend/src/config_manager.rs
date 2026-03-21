use actix_web::{get, post, web, HttpResponse};
use std::fs;
use std::process::Command;
use crate::models::{ApiResponse, ServiceConfig, UpdateConfigRequest};

const APACHE_CONFIGS: &[&str] = &[
    "/etc/httpd/conf/httpd.conf",
    "/etc/apache2/apache2.conf",
    "/etc/apache2/sites-enabled/000-default.conf",
];

const TOMCAT_CONFIGS: &[&str] = &[
    "/etc/tomcat/server.xml",
    "/opt/tomcat/conf/server.xml",
    "/usr/share/tomcat/conf/server.xml",
    "/etc/tomcat9/server.xml",
];

#[get("/api/config/apache")]
async fn get_apache_config() -> HttpResponse {
    let config = read_service_config("apache", APACHE_CONFIGS);
    HttpResponse::Ok().json(ApiResponse::ok(config))
}

#[get("/api/config/tomcat")]
async fn get_tomcat_config() -> HttpResponse {
    let config = read_service_config("tomcat", TOMCAT_CONFIGS);
    HttpResponse::Ok().json(ApiResponse::ok(config))
}

#[post("/api/config/apache")]
async fn update_apache_config(body: web::Json<UpdateConfigRequest>) -> HttpResponse {
    update_service_config("apache", APACHE_CONFIGS, &body.content).await
}

#[post("/api/config/tomcat")]
async fn update_tomcat_config(body: web::Json<UpdateConfigRequest>) -> HttpResponse {
    update_service_config("tomcat", TOMCAT_CONFIGS, &body.content).await
}

#[post("/api/config/apache/restart")]
async fn restart_apache() -> HttpResponse {
    restart_service("apache").await
}

#[post("/api/config/tomcat/restart")]
async fn restart_tomcat() -> HttpResponse {
    restart_service("tomcat").await
}

fn read_service_config(service_name: &str, paths: &[&str]) -> ServiceConfig {
    let mut config_path = String::new();
    let mut content = String::from("Configuration file not found");

    for path in paths {
        if let Ok(c) = fs::read_to_string(path) {
            config_path = path.to_string();
            content = c;
            break;
        }
    }

    let is_running = check_service_running(service_name);

    ServiceConfig {
        service_name: service_name.to_string(),
        config_path,
        content,
        is_running,
    }
}

fn check_service_running(service_name: &str) -> bool {
    // Try systemctl first
    let service_names = match service_name {
        "apache" => vec!["httpd", "apache2"],
        "tomcat" => vec!["tomcat", "tomcat9", "tomcat10"],
        _ => vec![service_name],
    };

    for svc in &service_names {
        if let Ok(output) = Command::new("systemctl")
            .args(["is-active", svc])
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if stdout.trim() == "active" {
                return true;
            }
        }
    }

    // Fallback: check process list
    if let Ok(output) = Command::new("pgrep").args(["-f", service_name]).output() {
        return output.status.success();
    }

    false
}

async fn update_service_config(
    service_name: &str,
    paths: &[&str],
    new_content: &str,
) -> HttpResponse {
    // Find existing config path
    let config_path = paths.iter().find(|p| fs::metadata(p).is_ok());

    let path = match config_path {
        Some(p) => *p,
        None => {
            return HttpResponse::NotFound()
                .json(ApiResponse::<()>::error("Config file not found"));
        }
    };

    // Create backup
    let backup_path = format!("{}.bak", path);
    if let Err(e) = fs::copy(path, &backup_path) {
        return HttpResponse::InternalServerError()
            .json(ApiResponse::<()>::error(&format!("Backup failed: {}", e)));
    }

    // Write new config
    match fs::write(path, new_content) {
        Ok(_) => HttpResponse::Ok().json(ApiResponse::ok(format!(
            "{} config updated (backup: {})",
            service_name, backup_path
        ))),
        Err(e) => {
            // Restore backup on failure
            let _ = fs::copy(&backup_path, path);
            HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::error(&format!("Write failed: {}", e)))
        }
    }
}

async fn restart_service(service_name: &str) -> HttpResponse {
    let service_names = match service_name {
        "apache" => vec!["httpd", "apache2"],
        "tomcat" => vec!["tomcat", "tomcat9"],
        _ => vec![service_name],
    };

    for svc in &service_names {
        let output = Command::new("systemctl")
            .args(["restart", svc])
            .output();

        if let Ok(result) = output {
            if result.status.success() {
                return HttpResponse::Ok()
                    .json(ApiResponse::ok(format!("{} restarted", svc)));
            }
        }
    }

    HttpResponse::InternalServerError()
        .json(ApiResponse::<()>::error(&format!("Failed to restart {}", service_name)))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(get_apache_config)
        .service(get_tomcat_config)
        .service(update_apache_config)
        .service(update_tomcat_config)
        .service(restart_apache)
        .service(restart_tomcat);
}
