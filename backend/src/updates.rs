use actix_web::{web, HttpResponse};
use crate::models::ApiResponse;
use serde::Serialize;

#[derive(Serialize, Clone)]
pub struct UpdatablePackage {
    pub name: String,
    pub current_version: String,
    pub new_version: String,
}

#[derive(Serialize, Clone)]
pub struct UpdateCheckResult {
    pub count: usize,
    pub packages: Vec<UpdatablePackage>,
}

fn detect_pkg_manager() -> Option<&'static str> {
    for cmd in &["apt", "dnf", "yum"] {
        if std::process::Command::new("which").arg(cmd)
            .output().map(|o| o.status.success()).unwrap_or(false) {
            return Some(cmd);
        }
    }
    None
}

async fn check_updates() -> HttpResponse {
    let mgr = match detect_pkg_manager() {
        Some(m) => m,
        None => return HttpResponse::Ok().json(ApiResponse::<UpdateCheckResult>::error("No supported package manager found")),
    };

    let output = match mgr {
        "apt" => {
            let _ = std::process::Command::new("apt").args(["update"]).output();
            std::process::Command::new("apt").args(["list", "--upgradable"]).output()
        }
        "dnf" | "yum" => {
            std::process::Command::new(mgr).args(["check-update", "-q"]).output()
        }
        _ => return HttpResponse::Ok().json(ApiResponse::<UpdateCheckResult>::error("Unsupported")),
    };

    let output = match output {
        Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
        Err(e) => return HttpResponse::Ok().json(ApiResponse::<UpdateCheckResult>::error(&e.to_string())),
    };

    let packages: Vec<UpdatablePackage> = match mgr {
        "apt" => parse_apt_upgradable(&output),
        _ => parse_dnf_updates(&output),
    };

    let count = packages.len();
    HttpResponse::Ok().json(ApiResponse::ok(UpdateCheckResult { count, packages }))
}

fn parse_apt_upgradable(output: &str) -> Vec<UpdatablePackage> {
    output.lines()
        .filter(|l| l.contains("[upgradable"))
        .filter_map(|line| {
            let name = line.split('/').next()?.to_string();
            let rest = line.split_whitespace().collect::<Vec<_>>();
            let new_ver = rest.get(1).unwrap_or(&"").to_string();
            let cur_ver = rest.iter()
                .position(|&w| w == "[upgradable")
                .and_then(|i| rest.get(i + 2))
                .map(|s| s.trim_end_matches(']').to_string())
                .unwrap_or_default();
            Some(UpdatablePackage { name, current_version: cur_ver, new_version: new_ver })
        })
        .collect()
}

fn parse_dnf_updates(output: &str) -> Vec<UpdatablePackage> {
    output.lines()
        .filter(|l| !l.is_empty() && !l.starts_with("Last metadata"))
        .filter_map(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                Some(UpdatablePackage {
                    name: parts[0].to_string(),
                    current_version: String::new(),
                    new_version: parts[1].to_string(),
                })
            } else { None }
        })
        .collect()
}

async fn run_upgrade() -> HttpResponse {
    let mgr = match detect_pkg_manager() {
        Some(m) => m,
        None => return HttpResponse::Ok().json(ApiResponse::<String>::error("No package manager")),
    };

    let result = match mgr {
        "apt" => std::process::Command::new("apt")
            .args(["upgrade", "-y"]).env("DEBIAN_FRONTEND", "noninteractive").output(),
        "dnf" => std::process::Command::new("dnf").args(["upgrade", "-y"]).output(),
        "yum" => std::process::Command::new("yum").args(["update", "-y"]).output(),
        _ => return HttpResponse::Ok().json(ApiResponse::<String>::error("Unsupported")),
    };

    match result {
        Ok(o) => {
            let out = String::from_utf8_lossy(&o.stdout).to_string();
            let err = String::from_utf8_lossy(&o.stderr).to_string();
            let combined = format!("{}\n{}", out, err).trim().to_string();
            if o.status.success() {
                HttpResponse::Ok().json(ApiResponse::ok(combined))
            } else {
                HttpResponse::Ok().json(ApiResponse::<String>::error(&combined))
            }
        }
        Err(e) => HttpResponse::Ok().json(ApiResponse::<String>::error(&e.to_string())),
    }
}

#[derive(serde::Deserialize)]
struct UpgradePackageRequest {
    pub name: String,
}

async fn upgrade_package(body: web::Json<UpgradePackageRequest>) -> HttpResponse {
    let name = &body.name;
    if name.is_empty() || name.len() > 128
        || !name.chars().all(|c| c.is_alphanumeric() || "-_.+:".contains(c)) {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Invalid package name"));
    }
    let mgr = match detect_pkg_manager() {
        Some(m) => m,
        None => return HttpResponse::Ok().json(ApiResponse::<String>::error("No package manager")),
    };
    let result = match mgr {
        "apt" => std::process::Command::new("apt")
            .args(["install", "--only-upgrade", "-y", name])
            .env("DEBIAN_FRONTEND", "noninteractive").output(),
        "dnf" => std::process::Command::new("dnf").args(["upgrade", "-y", name]).output(),
        "yum" => std::process::Command::new("yum").args(["update", "-y", name]).output(),
        _ => return HttpResponse::Ok().json(ApiResponse::<String>::error("Unsupported")),
    };
    match result {
        Ok(o) => {
            let combined = format!("{}\n{}",
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr)).trim().to_string();
            if o.status.success() {
                HttpResponse::Ok().json(ApiResponse::ok(combined))
            } else {
                HttpResponse::Ok().json(ApiResponse::<String>::error(&combined))
            }
        }
        Err(e) => HttpResponse::Ok().json(ApiResponse::<String>::error(&e.to_string())),
    }
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/updates")
            .route("/check", web::get().to(check_updates))
            .route("/upgrade", web::post().to(run_upgrade))
            .route("/upgrade-package", web::post().to(upgrade_package))
    );
}
