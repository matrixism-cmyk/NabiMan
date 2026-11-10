use crate::models::ApiResponse;
use actix_web::{web, HttpRequest, HttpResponse};
use nabiman_server::privileged::client::Client;
use serde::Serialize;

use crate::bounded_command::{deadline, OutputOffThread};

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

async fn detect_pkg_manager() -> Option<&'static str> {
    for cmd in &["apt", "dnf", "yum"] {
        if std::process::Command::new("which")
            .arg(cmd)
            .output_off_thread(deadline::QUICK).await
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return Some(cmd);
        }
    }
    None
}

async fn check_updates(req: HttpRequest, client: web::Data<Client>) -> HttpResponse {
    let mgr = match detect_pkg_manager().await {
        Some(m) => m,
        None => {
            return HttpResponse::Ok().json(ApiResponse::<UpdateCheckResult>::error(
                "No supported package manager found",
            ))
        }
    };

    let output = match mgr {
        "apt" => {
            if crate::run_mode::is_service() {
                crate::system_helper::refresh_packages(&req, &client).await;
            } else {
                let _ = std::process::Command::new("apt").args(["update"]).output_off_thread(deadline::PACKAGE_QUERY).await;
            }
            std::process::Command::new("apt")
                .args(["list", "--upgradable"])
                .output_off_thread(deadline::PACKAGE_QUERY).await
        }
        "dnf" | "yum" => std::process::Command::new(mgr)
            .args(["check-update", "-q"])
            .output_off_thread(deadline::PACKAGE_QUERY).await,
        _ => {
            return HttpResponse::Ok().json(ApiResponse::<UpdateCheckResult>::error("Unsupported"))
        }
    };

    let output = match output {
        Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
        Err(e) => {
            return HttpResponse::Ok().json(ApiResponse::<UpdateCheckResult>::error(&e.to_string()))
        }
    };

    let packages: Vec<UpdatablePackage> = match mgr {
        "apt" => parse_apt_upgradable(&output),
        _ => parse_dnf_updates(&output),
    };

    let count = packages.len();
    HttpResponse::Ok().json(ApiResponse::ok(UpdateCheckResult { count, packages }))
}

fn parse_apt_upgradable(output: &str) -> Vec<UpdatablePackage> {
    output
        .lines()
        .filter(|l| l.contains("[upgradable"))
        .filter_map(|line| {
            let name = line.split('/').next()?.to_string();
            let rest = line.split_whitespace().collect::<Vec<_>>();
            let new_ver = rest.get(1).unwrap_or(&"").to_string();
            let cur_ver = rest
                .iter()
                .position(|&w| w == "[upgradable")
                .and_then(|i| rest.get(i + 2))
                .map(|s| s.trim_end_matches(']').to_string())
                .unwrap_or_default();
            Some(UpdatablePackage {
                name,
                current_version: cur_ver,
                new_version: new_ver,
            })
        })
        .collect()
}

fn parse_dnf_updates(output: &str) -> Vec<UpdatablePackage> {
    output
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with("Last metadata"))
        .filter_map(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                Some(UpdatablePackage {
                    name: parts[0].to_string(),
                    current_version: String::new(),
                    new_version: parts[1].to_string(),
                })
            } else {
                None
            }
        })
        .collect()
}

async fn run_upgrade(req: HttpRequest, client: web::Data<Client>) -> HttpResponse {
    if crate::run_mode::is_service() {
        return crate::system_helper::upgrade(req, client, None).await;
    }
    let mgr = match detect_pkg_manager().await {
        Some(m) => m,
        None => return HttpResponse::Ok().json(ApiResponse::<String>::error("No package manager")),
    };

    let result = match mgr {
        "apt" => std::process::Command::new("apt")
            .args(["upgrade", "-y"])
            .env("DEBIAN_FRONTEND", "noninteractive")
            .output_off_thread(deadline::SYSTEM_UPGRADE).await,
        "dnf" => std::process::Command::new("dnf")
            .args(["upgrade", "-y"])
            .output_off_thread(deadline::SYSTEM_UPGRADE).await,
        "yum" => std::process::Command::new("yum")
            .args(["update", "-y"])
            .output_off_thread(deadline::SYSTEM_UPGRADE).await,
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

async fn upgrade_package(req: HttpRequest, client: web::Data<Client>, body: web::Json<UpgradePackageRequest>) -> HttpResponse {
    let name = &body.name;
    // The helper's rule: a name, never an option (no leading '-').
    if !nabiman_server::privileged::system_ops::valid_package(name) {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Invalid package name"));
    }
    if crate::run_mode::is_service() {
        return crate::system_helper::upgrade(req, client, Some(name.clone())).await;
    }
    let mgr = match detect_pkg_manager().await {
        Some(m) => m,
        None => return HttpResponse::Ok().json(ApiResponse::<String>::error("No package manager")),
    };
    let Some(args) = upgrade_one_args(mgr, name) else {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Unsupported"));
    };
    let mut command = std::process::Command::new(mgr);
    command.args(&args);
    if mgr == "apt" {
        command.env("DEBIAN_FRONTEND", "noninteractive");
    }
    let result = command.output_off_thread(deadline::PACKAGE).await;
    match result {
        Ok(o) => {
            let combined = format!(
                "{}\n{}",
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr)
            )
            .trim()
            .to_string();
            if o.status.success() {
                HttpResponse::Ok().json(ApiResponse::ok(combined))
            } else {
                HttpResponse::Ok().json(ApiResponse::<String>::error(&combined))
            }
        }
        Err(e) => HttpResponse::Ok().json(ApiResponse::<String>::error(&e.to_string())),
    }
}

/// The argv after the program for upgrading one package; `--` ends the
/// options, so the name is only ever read as a name.
fn upgrade_one_args<'a>(mgr: &str, name: &'a str) -> Option<Vec<&'a str>> {
    match mgr {
        "apt" => Some(vec!["install", "--only-upgrade", "-y", "--", name]),
        "dnf" => Some(vec!["upgrade", "-y", "--", name]),
        "yum" => Some(vec!["update", "-y", "--", name]),
        _ => None,
    }
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/updates")
            .route("/check", web::get().to(check_updates))
            .route("/upgrade", web::post().to(run_upgrade))
            .route("/upgrade-package", web::post().to(upgrade_package)),
    );
}

#[cfg(test)]
mod package_name_tests {
    use super::upgrade_one_args;

    #[test]
    fn one_package_is_upgraded_by_name_after_the_end_of_options() {
        assert_eq!(upgrade_one_args("apt", "nginx").unwrap(), ["install", "--only-upgrade", "-y", "--", "nginx"]);
        assert_eq!(upgrade_one_args("dnf", "nginx").unwrap(), ["upgrade", "-y", "--", "nginx"]);
        assert_eq!(upgrade_one_args("yum", "nginx").unwrap(), ["update", "-y", "--", "nginx"]);
        assert!(upgrade_one_args("pacman", "nginx").is_none());
    }
}
