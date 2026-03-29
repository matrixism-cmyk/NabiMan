use actix_web::{web, HttpResponse};
use crate::models::ApiResponse;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Clone)]
pub struct VhostInfo {
    pub domain: String,
    pub vhost_type: String,
    pub target: String,
    pub ssl: bool,
    pub enabled: bool,
    pub config_file: String,
    pub server: String, // "apache" or "nginx"
}

fn detect_web_server() -> &'static str {
    if std::path::Path::new("/etc/nginx/sites-available").exists() { "nginx" }
    else { "apache" }
}

async fn list_vhosts() -> HttpResponse {
    let mut vhosts = Vec::new();
    // Apache
    list_apache_vhosts(&mut vhosts);
    // Nginx
    list_nginx_vhosts(&mut vhosts);
    vhosts.sort_by(|a, b| a.domain.cmp(&b.domain));
    HttpResponse::Ok().json(ApiResponse::ok(vhosts))
}

fn list_apache_vhosts(vhosts: &mut Vec<VhostInfo>) {
    let avail = std::path::Path::new("/etc/apache2/sites-available");
    let enabled = std::path::Path::new("/etc/apache2/sites-enabled");
    if let Ok(entries) = std::fs::read_dir(avail) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.ends_with(".conf") || name.contains("-le-ssl") { continue; }
            let content = std::fs::read_to_string(entry.path()).unwrap_or_default();
            let domain = extract_apache_field(&content, "ServerName").unwrap_or_default();
            if domain.is_empty() || domain == "www.example.com" { continue; }
            let (vtype, target) = detect_apache_type(&content);
            let has_ssl = avail.join(format!("{}-le-ssl.conf", name.trim_end_matches(".conf"))).exists();
            vhosts.push(VhostInfo {
                domain, vhost_type: vtype, target, ssl: has_ssl,
                enabled: enabled.join(&name).exists(), config_file: name, server: "apache".into(),
            });
        }
    }
}

fn list_nginx_vhosts(vhosts: &mut Vec<VhostInfo>) {
    let avail = std::path::Path::new("/etc/nginx/sites-available");
    let enabled = std::path::Path::new("/etc/nginx/sites-enabled");
    if let Ok(entries) = std::fs::read_dir(avail) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name == "default" { continue; }
            let content = std::fs::read_to_string(entry.path()).unwrap_or_default();
            let domain = extract_nginx_field(&content, "server_name").unwrap_or_default();
            if domain.is_empty() || domain == "_" { continue; }
            let (vtype, target) = detect_nginx_type(&content);
            let has_ssl = content.contains("ssl_certificate") || content.contains("listen 443");
            vhosts.push(VhostInfo {
                domain, vhost_type: vtype, target, ssl: has_ssl,
                enabled: enabled.join(&name).exists(), config_file: name, server: "nginx".into(),
            });
        }
    }
}

fn extract_apache_field(content: &str, field: &str) -> Option<String> {
    content.lines().find(|l| l.trim().starts_with(field))
        .map(|l| l.split_whitespace().nth(1).unwrap_or("").to_string())
}

fn extract_nginx_field(content: &str, field: &str) -> Option<String> {
    content.lines().find(|l| l.trim().starts_with(field))
        .map(|l| l.split_whitespace().nth(1).unwrap_or("").trim_end_matches(';').to_string())
}

fn detect_apache_type(content: &str) -> (String, String) {
    if content.contains("ProxyPass") {
        let t = content.lines().find(|l| l.trim().starts_with("ProxyPass /") && !l.contains("ws://"))
            .and_then(|l| l.split_whitespace().nth(2)).unwrap_or("").to_string();
        ("proxy".into(), t)
    } else if content.contains("Redirect ") {
        let t = content.lines().find(|l| l.trim().starts_with("Redirect"))
            .and_then(|l| l.split_whitespace().last()).unwrap_or("").to_string();
        ("redirect".into(), t)
    } else {
        ("static".into(), extract_apache_field(content, "DocumentRoot").unwrap_or_default())
    }
}

fn detect_nginx_type(content: &str) -> (String, String) {
    if content.contains("proxy_pass") {
        let t = content.lines().find(|l| l.trim().starts_with("proxy_pass"))
            .map(|l| l.split_whitespace().nth(1).unwrap_or("").trim_end_matches(';').to_string())
            .unwrap_or_default();
        ("proxy".into(), t)
    } else if content.contains("return 301") || content.contains("return 302") {
        let t = content.lines().find(|l| l.trim().starts_with("return 30"))
            .and_then(|l| l.split_whitespace().nth(2))
            .map(|s| s.trim_end_matches(';').to_string()).unwrap_or_default();
        ("redirect".into(), t)
    } else {
        let root = extract_nginx_field(content, "root").unwrap_or_default();
        ("static".into(), root)
    }
}

// --- Create ---

#[derive(Deserialize)]
pub struct CreateVhostRequest {
    pub domain: String,
    pub vhost_type: String,
    pub target: String,
    pub auto_ssl: Option<bool>,
    pub websocket_path: Option<String>,
    #[serde(default)]
    pub server: Option<String>, // "apache" or "nginx", auto-detected if omitted
}

fn validate_domain(d: &str) -> bool {
    !d.is_empty() && d.len() <= 253 && d.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '-')
}

async fn create_vhost(body: web::Json<CreateVhostRequest>) -> HttpResponse {
    if !validate_domain(&body.domain) {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Invalid domain"));
    }
    let srv = body.server.as_deref().unwrap_or_else(|| detect_web_server());

    match srv {
        "nginx" => create_nginx_vhost(&body),
        _ => create_apache_vhost(&body),
    }
}

fn create_apache_vhost(body: &CreateVhostRequest) -> HttpResponse {
    let conf = format!("/etc/apache2/sites-available/{}.conf", body.domain);
    if std::path::Path::new(&conf).exists() {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Vhost already exists"));
    }
    let config = match body.vhost_type.as_str() {
        "static" => gen_apache_static(&body.domain, &body.target),
        "proxy" => gen_apache_proxy(&body.domain, &body.target, body.websocket_path.as_deref()),
        "redirect" => gen_apache_redirect(&body.domain, &body.target),
        _ => return HttpResponse::Ok().json(ApiResponse::<String>::error("Invalid type")),
    };
    if let Err(e) = std::fs::write(&conf, &config) {
        return HttpResponse::Ok().json(ApiResponse::<String>::error(&e.to_string()));
    }
    let _ = std::process::Command::new("a2ensite").arg(format!("{}.conf", body.domain)).output();
    if body.vhost_type == "static" { let _ = std::fs::create_dir_all(&body.target); }
    let _ = std::process::Command::new("systemctl").args(["reload", "apache2"]).output();
    let mut msg = format!("{} created (Apache)", body.domain);
    if body.auto_ssl.unwrap_or(false) { msg += &run_certbot(&body.domain, "apache"); }
    HttpResponse::Ok().json(ApiResponse::ok(msg))
}

fn create_nginx_vhost(body: &CreateVhostRequest) -> HttpResponse {
    let conf = format!("/etc/nginx/sites-available/{}", body.domain);
    if std::path::Path::new(&conf).exists() {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Vhost already exists"));
    }
    let config = match body.vhost_type.as_str() {
        "static" => gen_nginx_static(&body.domain, &body.target),
        "proxy" => gen_nginx_proxy(&body.domain, &body.target, body.websocket_path.as_deref()),
        "redirect" => gen_nginx_redirect(&body.domain, &body.target),
        _ => return HttpResponse::Ok().json(ApiResponse::<String>::error("Invalid type")),
    };
    if let Err(e) = std::fs::write(&conf, &config) {
        return HttpResponse::Ok().json(ApiResponse::<String>::error(&e.to_string()));
    }
    let enabled_link = format!("/etc/nginx/sites-enabled/{}", body.domain);
    let _ = std::os::unix::fs::symlink(&conf, &enabled_link);
    if body.vhost_type == "static" { let _ = std::fs::create_dir_all(&body.target); }
    let _ = std::process::Command::new("nginx").args(["-s", "reload"]).output();
    let mut msg = format!("{} created (Nginx)", body.domain);
    if body.auto_ssl.unwrap_or(false) { msg += &run_certbot(&body.domain, "nginx"); }
    HttpResponse::Ok().json(ApiResponse::ok(msg))
}

fn run_certbot(domain: &str, plugin: &str) -> String {
    let flag = format!("--{}", plugin);
    match std::process::Command::new("timeout")
        .args(["120", "certbot", &flag, "-d", domain, "--non-interactive", "--agree-tos", "--no-eff-email", "--email", "webmaster@xos.kr"])
        .output() {
        Ok(o) if o.status.success() => " + SSL issued".into(),
        Ok(o) => format!(" (SSL failed: {})", String::from_utf8_lossy(&o.stderr).lines().last().unwrap_or("")),
        _ => " (certbot error)".into(),
    }
}

// --- Apache config generators ---
fn gen_apache_static(d: &str, root: &str) -> String {
    format!("<VirtualHost *:80>\n\tServerName {d}\n\tDocumentRoot {root}\n\t<Directory {root}>\n\t\tOptions Indexes FollowSymLinks\n\t\tAllowOverride All\n\t\tRequire all granted\n\t</Directory>\n\tErrorLog ${{APACHE_LOG_DIR}}/{d}-error.log\n\tCustomLog ${{APACHE_LOG_DIR}}/{d}-access.log combined\n</VirtualHost>\n")
}
fn gen_apache_proxy(d: &str, t: &str, ws: Option<&str>) -> String {
    let ws_block = ws.map(|p| { let h = t.trim_start_matches("http://").trim_start_matches("https://").trim_end_matches('/');
        format!("\tProxyPass {p} ws://{h}{p}\n\tProxyPassReverse {p} ws://{h}{p}\n") }).unwrap_or_default();
    format!("<VirtualHost *:80>\n\tServerName {d}\n\tProxyPreserveHost On\n{ws_block}\tProxyPass / {t}\n\tProxyPassReverse / {t}\n\tErrorLog ${{APACHE_LOG_DIR}}/{d}-error.log\n\tCustomLog ${{APACHE_LOG_DIR}}/{d}-access.log combined\n</VirtualHost>\n")
}
fn gen_apache_redirect(d: &str, t: &str) -> String {
    format!("<VirtualHost *:80>\n\tServerName {d}\n\tRedirect permanent / {t}\n</VirtualHost>\n")
}

// --- Nginx config generators ---
fn gen_nginx_static(d: &str, root: &str) -> String {
    format!("server {{\n    listen 80;\n    server_name {d};\n    root {root};\n    index index.html index.htm;\n\n    location / {{\n        try_files $uri $uri/ =404;\n    }}\n\n    access_log /var/log/nginx/{d}-access.log;\n    error_log /var/log/nginx/{d}-error.log;\n}}\n")
}
fn gen_nginx_proxy(d: &str, t: &str, ws: Option<&str>) -> String {
    let ws_block = ws.map(|p| format!("\n    location {p} {{\n        proxy_pass {t};\n        proxy_http_version 1.1;\n        proxy_set_header Upgrade $http_upgrade;\n        proxy_set_header Connection \"upgrade\";\n    }}\n")).unwrap_or_default();
    format!("server {{\n    listen 80;\n    server_name {d};\n\n    location / {{\n        proxy_pass {t};\n        proxy_set_header Host $host;\n        proxy_set_header X-Real-IP $remote_addr;\n        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;\n        proxy_set_header X-Forwarded-Proto $scheme;\n    }}{ws_block}\n\n    access_log /var/log/nginx/{d}-access.log;\n    error_log /var/log/nginx/{d}-error.log;\n}}\n")
}
fn gen_nginx_redirect(d: &str, t: &str) -> String {
    format!("server {{\n    listen 80;\n    server_name {d};\n    return 301 {t}$request_uri;\n}}\n")
}

// --- Delete ---
#[derive(Deserialize)]
pub struct DeleteVhostRequest { pub domain: String, pub server: Option<String> }

async fn delete_vhost(body: web::Json<DeleteVhostRequest>) -> HttpResponse {
    if !validate_domain(&body.domain) {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Invalid domain"));
    }
    let srv = body.server.as_deref().unwrap_or_else(|| detect_web_server());
    if srv == "nginx" {
        let _ = std::fs::remove_file(format!("/etc/nginx/sites-enabled/{}", body.domain));
        let _ = std::fs::remove_file(format!("/etc/nginx/sites-available/{}", body.domain));
        let _ = std::process::Command::new("nginx").args(["-s", "reload"]).output();
    } else {
        let _ = std::process::Command::new("a2dissite").arg(format!("{}.conf", body.domain)).output();
        let _ = std::process::Command::new("a2dissite").arg(format!("{}-le-ssl.conf", body.domain)).output();
        let _ = std::fs::remove_file(format!("/etc/apache2/sites-available/{}.conf", body.domain));
        let _ = std::fs::remove_file(format!("/etc/apache2/sites-available/{}-le-ssl.conf", body.domain));
        let _ = std::process::Command::new("systemctl").args(["reload", "apache2"]).output();
    }
    HttpResponse::Ok().json(ApiResponse::ok(format!("{} removed", body.domain)))
}

// --- Read/Save config ---
#[derive(Serialize)]
struct VhostConfig { domain: String, http_config: String, ssl_config: Option<String>, server: String }

async fn read_config(path: web::Path<String>) -> HttpResponse {
    let domain = path.into_inner();
    if !validate_domain(&domain) { return HttpResponse::Ok().json(ApiResponse::<VhostConfig>::error("Invalid")); }
    // Try Nginx first, then Apache
    let nginx = format!("/etc/nginx/sites-available/{}", domain);
    if std::path::Path::new(&nginx).exists() {
        let content = std::fs::read_to_string(&nginx).unwrap_or_default();
        return HttpResponse::Ok().json(ApiResponse::ok(VhostConfig { domain, http_config: content, ssl_config: None, server: "nginx".into() }));
    }
    let apache = format!("/etc/apache2/sites-available/{}.conf", domain);
    let http = std::fs::read_to_string(&apache).unwrap_or_default();
    if http.is_empty() { return HttpResponse::Ok().json(ApiResponse::<VhostConfig>::error("Not found")); }
    let ssl = std::fs::read_to_string(format!("/etc/apache2/sites-available/{}-le-ssl.conf", domain)).ok();
    HttpResponse::Ok().json(ApiResponse::ok(VhostConfig { domain, http_config: http, ssl_config: ssl, server: "apache".into() }))
}

#[derive(Deserialize)]
struct SaveConfigRequest { domain: String, http_config: String, ssl_config: Option<String>, server: Option<String> }

async fn save_config(body: web::Json<SaveConfigRequest>) -> HttpResponse {
    if !validate_domain(&body.domain) { return HttpResponse::Ok().json(ApiResponse::<String>::error("Invalid")); }
    let srv = body.server.as_deref().unwrap_or("apache");
    if srv == "nginx" {
        let _ = std::fs::write(format!("/etc/nginx/sites-available/{}", body.domain), &body.http_config);
        let test = std::process::Command::new("nginx").arg("-t").output();
        if let Ok(o) = &test { if !o.status.success() {
            return HttpResponse::Ok().json(ApiResponse::<String>::error(&format!("Syntax error: {}", String::from_utf8_lossy(&o.stderr))));
        }}
        let _ = std::process::Command::new("nginx").args(["-s", "reload"]).output();
        HttpResponse::Ok().json(ApiResponse::ok("Nginx config saved".to_string()))
    } else {
        let _ = std::fs::write(format!("/etc/apache2/sites-available/{}.conf", body.domain), &body.http_config);
        if let Some(ref ssl) = body.ssl_config {
            let _ = std::fs::write(format!("/etc/apache2/sites-available/{}-le-ssl.conf", body.domain), ssl);
        }
        let test = std::process::Command::new("apache2ctl").arg("configtest").output();
        if let Ok(o) = &test { if !o.status.success() {
            return HttpResponse::Ok().json(ApiResponse::<String>::error(&format!("Syntax error: {}", String::from_utf8_lossy(&o.stderr))));
        }}
        let _ = std::process::Command::new("systemctl").args(["reload", "apache2"]).output();
        HttpResponse::Ok().json(ApiResponse::ok("Apache config saved".to_string()))
    }
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/vhost")
            .route("", web::get().to(list_vhosts))
            .route("", web::post().to(create_vhost))
            .route("/delete", web::post().to(delete_vhost))
            .route("/config/{domain}", web::get().to(read_config))
            .route("/config", web::post().to(save_config))
    );
}
