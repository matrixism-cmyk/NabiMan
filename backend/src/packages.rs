use actix_web::{get, post, web, HttpResponse};
use serde::Deserialize;
use std::process::Command;
use crate::models::ApiResponse;

#[derive(Deserialize)]
pub struct PackageRequest {
    pub name: String,
}

#[derive(Deserialize)]
pub struct SearchRequest {
    pub query: String,
}

/// Detect package manager: apt, yum, or dnf
fn detect_pkg_manager() -> Option<&'static str> {
    for cmd in &["apt-get", "dnf", "yum"] {
        if Command::new("which").arg(cmd).output()
            .map(|o| o.status.success()).unwrap_or(false)
        {
            return Some(cmd);
        }
    }
    None
}

#[get("/api/packages/installed")]
async fn list_installed() -> HttpResponse {
    let pm = detect_pkg_manager();

    let (cmd, args): (&str, &[&str]) = match pm {
        Some("apt-get") => ("dpkg", &["--get-selections"]),
        Some("dnf") | Some("yum") => ("rpm", &["-qa", "--qf", "%{NAME}\t%{VERSION}-%{RELEASE}\n"]),
        _ => {
            return HttpResponse::Ok()
                .json(ApiResponse::<Vec<String>>::error("No supported package manager found"));
        }
    };

    match Command::new(cmd).args(args).output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let packages: Vec<serde_json::Value> = stdout
                .lines()
                .filter(|l| !l.is_empty())
                .take(500) // limit response size
                .map(|line| {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    serde_json::json!({
                        "name": parts.first().unwrap_or(&""),
                        "status": parts.get(1).unwrap_or(&""),
                    })
                })
                .collect();
            HttpResponse::Ok().json(ApiResponse::ok(packages))
        }
        Err(e) => HttpResponse::InternalServerError()
            .json(ApiResponse::<()>::error(&format!("Failed: {}", e))),
    }
}

#[post("/api/packages/search")]
async fn search_package(body: web::Json<SearchRequest>) -> HttpResponse {
    // Validate query: alphanumeric, hyphen, underscore, dot only
    if !body.query.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.') {
        return HttpResponse::BadRequest()
            .json(ApiResponse::<()>::error("Invalid search query"));
    }

    let pm = detect_pkg_manager();

    let output = match pm {
        Some("apt-get") => Command::new("apt-cache")
            .args(["search", &body.query])
            .output(),
        Some(yum_or_dnf) => Command::new(yum_or_dnf)
            .args(["search", &body.query])
            .output(),
        _ => {
            return HttpResponse::Ok()
                .json(ApiResponse::<()>::error("No supported package manager found"));
        }
    };

    match output {
        Ok(result) => {
            let stdout = String::from_utf8_lossy(&result.stdout);
            let results: Vec<serde_json::Value> = stdout
                .lines()
                .filter(|l| !l.is_empty() && !l.starts_with("="))
                .take(100)
                .map(|line| {
                    let parts: Vec<&str> = line.splitn(2, " - ").collect();
                    serde_json::json!({
                        "name": parts.first().unwrap_or(&"").trim(),
                        "description": parts.get(1).unwrap_or(&"").trim(),
                    })
                })
                .collect();
            HttpResponse::Ok().json(ApiResponse::ok(results))
        }
        Err(e) => HttpResponse::InternalServerError()
            .json(ApiResponse::<()>::error(&format!("Search failed: {}", e))),
    }
}

#[post("/api/packages/install")]
async fn install_package(body: web::Json<PackageRequest>) -> HttpResponse {
    if !validate_package_name(&body.name) {
        return HttpResponse::BadRequest()
            .json(ApiResponse::<()>::error("Invalid package name"));
    }

    let pm = match detect_pkg_manager() {
        Some(pm) => pm,
        None => return HttpResponse::Ok()
            .json(ApiResponse::<()>::error("No supported package manager found")),
    };

    let output = match pm {
        "apt-get" => Command::new("apt-get")
            .args(["install", "-y", &body.name])
            .env("DEBIAN_FRONTEND", "noninteractive")
            .output(),
        yum_or_dnf => Command::new(yum_or_dnf)
            .args(["install", "-y", &body.name])
            .output(),
    };

    match output {
        Ok(result) if result.status.success() => {
            HttpResponse::Ok().json(ApiResponse::ok(format!("{} installed", body.name)))
        }
        Ok(result) => {
            let stderr = String::from_utf8_lossy(&result.stderr);
            HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::error(&format!("Install failed: {}", stderr)))
        }
        Err(e) => HttpResponse::InternalServerError()
            .json(ApiResponse::<()>::error(&format!("Failed: {}", e))),
    }
}

#[post("/api/packages/remove")]
async fn remove_package(body: web::Json<PackageRequest>) -> HttpResponse {
    if !validate_package_name(&body.name) {
        return HttpResponse::BadRequest()
            .json(ApiResponse::<()>::error("Invalid package name"));
    }

    let pm = match detect_pkg_manager() {
        Some(pm) => pm,
        None => return HttpResponse::Ok()
            .json(ApiResponse::<()>::error("No supported package manager found")),
    };

    let output = match pm {
        "apt-get" => Command::new("apt-get")
            .args(["remove", "-y", &body.name])
            .env("DEBIAN_FRONTEND", "noninteractive")
            .output(),
        yum_or_dnf => Command::new(yum_or_dnf)
            .args(["remove", "-y", &body.name])
            .output(),
    };

    match output {
        Ok(result) if result.status.success() => {
            HttpResponse::Ok().json(ApiResponse::ok(format!("{} removed", body.name)))
        }
        Ok(result) => {
            let stderr = String::from_utf8_lossy(&result.stderr);
            HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::error(&format!("Remove failed: {}", stderr)))
        }
        Err(e) => HttpResponse::InternalServerError()
            .json(ApiResponse::<()>::error(&format!("Failed: {}", e))),
    }
}

fn validate_package_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() < 128
        && name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '+')
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(list_installed)
        .service(search_package)
        .service(install_package)
        .service(remove_package);
}
