use super::*;

// --- Read/Save config ---
#[derive(Serialize)]
pub(super) struct VhostConfig {
    domain: String,
    http_config: String,
    ssl_config: Option<String>,
    server: String,
}

pub(super) async fn read_config(path: web::Path<String>) -> HttpResponse {
    let domain = path.into_inner();
    if !validate_domain(&domain) {
        return HttpResponse::Ok().json(ApiResponse::<VhostConfig>::error("Invalid"));
    }
    // Try Nginx first, then Apache
    let nginx = format!("/etc/nginx/sites-available/{}", domain);
    if std::path::Path::new(&nginx).exists() {
        let content = std::fs::read_to_string(&nginx).unwrap_or_default();
        return HttpResponse::Ok().json(ApiResponse::ok(VhostConfig {
            domain,
            http_config: content,
            ssl_config: None,
            server: "nginx".into(),
        }));
    }
    let apache = format!("/etc/apache2/sites-available/{}.conf", domain);
    let http = std::fs::read_to_string(&apache).unwrap_or_default();
    if http.is_empty() {
        return HttpResponse::Ok().json(ApiResponse::<VhostConfig>::error("Not found"));
    }
    let ssl = std::fs::read_to_string(format!(
        "/etc/apache2/sites-available/{}-le-ssl.conf",
        domain
    ))
    .ok();
    HttpResponse::Ok().json(ApiResponse::ok(VhostConfig {
        domain,
        http_config: http,
        ssl_config: ssl,
        server: "apache".into(),
    }))
}

#[derive(Deserialize)]
pub(super) struct SaveConfigRequest {
    domain: String,
    http_config: String,
    ssl_config: Option<String>,
    server: Option<String>,
}

pub(super) async fn save_config(body: web::Json<SaveConfigRequest>) -> HttpResponse {
    if !validate_domain(&body.domain) {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Invalid"));
    }
    let srv = body.server.as_deref().unwrap_or("apache");
    if srv == "nginx" {
        let _ = std::fs::write(
            format!("/etc/nginx/sites-available/{}", body.domain),
            &body.http_config,
        );
        let test = std::process::Command::new("nginx").arg("-t").output();
        if let Ok(o) = &test {
            if !o.status.success() {
                return HttpResponse::Ok().json(ApiResponse::<String>::error(&format!(
                    "Syntax error: {}",
                    String::from_utf8_lossy(&o.stderr)
                )));
            }
        }
        let _ = std::process::Command::new("nginx")
            .args(["-s", "reload"])
            .output();
        HttpResponse::Ok().json(ApiResponse::ok("Nginx config saved".to_string()))
    } else {
        let _ = std::fs::write(
            format!("/etc/apache2/sites-available/{}.conf", body.domain),
            &body.http_config,
        );
        if let Some(ref ssl) = body.ssl_config {
            let _ = std::fs::write(
                format!("/etc/apache2/sites-available/{}-le-ssl.conf", body.domain),
                ssl,
            );
        }
        let test = std::process::Command::new("apache2ctl")
            .arg("configtest")
            .output();
        if let Ok(o) = &test {
            if !o.status.success() {
                return HttpResponse::Ok().json(ApiResponse::<String>::error(&format!(
                    "Syntax error: {}",
                    String::from_utf8_lossy(&o.stderr)
                )));
            }
        }
        let _ = std::process::Command::new("systemctl")
            .args(["reload", "apache2"])
            .output();
        HttpResponse::Ok().json(ApiResponse::ok("Apache config saved".to_string()))
    }
}
