use crate::models::ApiResponse;
use actix_web::{get, post, web, HttpResponse};
use serde::Deserialize;
use std::process::Command;

use crate::bounded_command::{deadline, OutputOffThread};

#[derive(Deserialize)]
pub struct PackageRequest {
    pub name: String,
}

#[derive(Deserialize)]
pub struct SearchRequest {
    pub query: String,
}

/// Detect package manager: apt, yum, or dnf
async fn detect_pkg_manager() -> Option<&'static str> {
    for cmd in &["apt-get", "dnf", "yum"] {
        if Command::new("which")
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

#[get("/api/packages/installed")]
async fn list_installed() -> HttpResponse {
    let pm = detect_pkg_manager().await;

    let (cmd, args): (&str, &[&str]) = match pm {
        Some("apt-get") => ("dpkg", &["--get-selections"]),
        Some("dnf") | Some("yum") => ("rpm", &["-qa", "--qf", "%{NAME}\t%{VERSION}-%{RELEASE}\n"]),
        _ => {
            return HttpResponse::Ok().json(ApiResponse::<Vec<String>>::error(
                "No supported package manager found",
            ));
        }
    };

    match Command::new(cmd).args(args).output_off_thread(deadline::PACKAGE_QUERY).await {
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
    // Validate query: alphanumeric, hyphen, underscore, dot only, and not an option
    if body.query.starts_with('-')
        || !body
            .query
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.')
    {
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error("Invalid search query"));
    }

    let pm = detect_pkg_manager().await;

    let output = match pm {
        Some("apt-get") => Command::new("apt-cache")
            .args(["search", "--", &body.query])
            .output_off_thread(deadline::PACKAGE_QUERY).await,
        Some(yum_or_dnf) => Command::new(yum_or_dnf)
            .args(["search", "--", &body.query])
            .output_off_thread(deadline::PACKAGE_QUERY).await,
        _ => {
            return HttpResponse::Ok().json(ApiResponse::<()>::error(
                "No supported package manager found",
            ));
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
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error("Invalid package name"));
    }

    let pm = match detect_pkg_manager().await {
        Some(pm) => pm,
        None => {
            return HttpResponse::Ok().json(ApiResponse::<()>::error(
                "No supported package manager found",
            ))
        }
    };

    let output = match pm {
        "apt-get" => Command::new("apt-get")
            .args(package_args("install", &body.name))
            .env("DEBIAN_FRONTEND", "noninteractive")
            .output_off_thread(deadline::PACKAGE).await,
        yum_or_dnf => Command::new(yum_or_dnf)
            .args(package_args("install", &body.name))
            .output_off_thread(deadline::PACKAGE).await,
    };

    match output {
        Ok(result) if result.status.success() => {
            HttpResponse::Ok().json(ApiResponse::ok(format!("{} installed", body.name)))
        }
        Ok(result) => {
            let stderr = String::from_utf8_lossy(&result.stderr);
            HttpResponse::InternalServerError().json(ApiResponse::<()>::error(&format!(
                "Install failed: {}",
                stderr
            )))
        }
        Err(e) => HttpResponse::InternalServerError()
            .json(ApiResponse::<()>::error(&format!("Failed: {}", e))),
    }
}

#[post("/api/packages/remove")]
async fn remove_package(body: web::Json<PackageRequest>) -> HttpResponse {
    if !validate_package_name(&body.name) {
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error("Invalid package name"));
    }

    let pm = match detect_pkg_manager().await {
        Some(pm) => pm,
        None => {
            return HttpResponse::Ok().json(ApiResponse::<()>::error(
                "No supported package manager found",
            ))
        }
    };

    let output = match pm {
        "apt-get" => Command::new("apt-get")
            .args(package_args("remove", &body.name))
            .env("DEBIAN_FRONTEND", "noninteractive")
            .output_off_thread(deadline::PACKAGE).await,
        yum_or_dnf => Command::new(yum_or_dnf)
            .args(package_args("remove", &body.name))
            .output_off_thread(deadline::PACKAGE).await,
    };

    match output {
        Ok(result) if result.status.success() => {
            HttpResponse::Ok().json(ApiResponse::ok(format!("{} removed", body.name)))
        }
        Ok(result) => {
            let stderr = String::from_utf8_lossy(&result.stderr);
            HttpResponse::InternalServerError().json(ApiResponse::<()>::error(&format!(
                "Remove failed: {}",
                stderr
            )))
        }
        Err(e) => HttpResponse::InternalServerError()
            .json(ApiResponse::<()>::error(&format!("Failed: {}", e))),
    }
}

/// One rule with the helper: starts with a letter or digit (never an
/// option such as `--allow-unauthenticated`), then `[A-Za-z0-9-_.+:]`.
fn validate_package_name(name: &str) -> bool {
    nabiman_server::privileged::system_ops::valid_package(name)
}

/// The argv after the program to install or remove one package (`verb`);
/// `--` ends the options, so the name is only ever read as a name.
fn package_args<'a>(verb: &'a str, name: &'a str) -> [&'a str; 4] {
    [verb, "-y", "--", name]
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(list_installed)
        .service(search_package)
        .service(install_package)
        .service(remove_package);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_package_name_is_never_an_option() {
        for bad in ["--allow-unauthenticated", "-oAPT::Get::AllowUnauthenticated=1", "-y", "./evil.deb", "/tmp/x.rpm", "nginx curl", "nginx;id", "", "가나"] {
            assert!(!validate_package_name(bad), "{bad}");
        }
        for good in ["nginx", "g++-12", "libssl3:amd64", "python3.12", "lib_foo"] {
            assert!(validate_package_name(good), "{good}");
        }
        assert!(validate_package_name(&"a".repeat(128)) && !validate_package_name(&"a".repeat(129)));
        // The same rule the helper applies.
        assert_eq!(validate_package_name("-x"), nabiman_server::privileged::system_ops::valid_package("-x"));
    }

    #[test]
    fn install_and_remove_end_the_options_before_the_name() {
        assert_eq!(package_args("install", "nginx"), ["install", "-y", "--", "nginx"]);
        assert_eq!(package_args("remove", "nginx"), ["remove", "-y", "--", "nginx"]);
    }
}
