use actix_web::{web, HttpResponse};
use std::fs;
use std::process::Command;
use crate::models::{ApiResponse, ServiceConfig, ServiceDefinition, UpdateConfigRequest};

/// Service registry - add new services here only.
/// No endpoint or function changes needed.
fn service_registry() -> Vec<ServiceDefinition> {
    vec![
        ServiceDefinition {
            id: "apache".into(),
            display_name: "Apache HTTP Server".into(),
            config_paths: vec![
                "/etc/httpd/conf/httpd.conf".into(),
                "/etc/apache2/apache2.conf".into(),
                "/etc/apache2/sites-enabled/000-default.conf".into(),
            ],
            systemd_names: vec!["httpd".into(), "apache2".into()],
            process_name: "apache".into(),
            binary_names: vec!["httpd".into(), "apache2".into(), "apache2ctl".into()],
            is_running: None, is_installed: None, config_found: None, detected_by: None, version: None,
        },
        ServiceDefinition {
            id: "tomcat".into(),
            display_name: "Apache Tomcat".into(),
            config_paths: vec![
                "/etc/tomcat/server.xml".into(),
                "/opt/tomcat/conf/server.xml".into(),
                "/usr/share/tomcat/conf/server.xml".into(),
                "/etc/tomcat9/server.xml".into(),
                "/etc/tomcat10/server.xml".into(),
            ],
            systemd_names: vec!["tomcat".into(), "tomcat9".into(), "tomcat10".into()],
            process_name: "tomcat".into(),
            binary_names: vec!["catalina.sh".into()],
            is_running: None, is_installed: None, config_found: None, detected_by: None, version: None,
        },
        ServiceDefinition {
            id: "nginx".into(),
            display_name: "Nginx".into(),
            config_paths: vec![
                "/etc/nginx/nginx.conf".into(),
                "/etc/nginx/sites-enabled/default".into(),
            ],
            systemd_names: vec!["nginx".into()],
            process_name: "nginx".into(),
            binary_names: vec!["nginx".into()],
            is_running: None, is_installed: None, config_found: None, detected_by: None, version: None,
        },
        ServiceDefinition {
            id: "mysql".into(),
            display_name: "MySQL / MariaDB".into(),
            config_paths: vec![
                "/etc/mysql/mysql.conf.d/mysqld.cnf".into(),
                "/etc/mysql/my.cnf".into(),
                "/etc/my.cnf".into(),
                "/etc/mysql/mariadb.conf.d/50-server.cnf".into(),
            ],
            systemd_names: vec!["mysql".into(), "mysqld".into(), "mariadb".into()],
            process_name: "mysql".into(),
            binary_names: vec!["mysql".into(), "mysqld".into(), "mariadb".into(), "mariadbd".into()],
            is_running: None, is_installed: None, config_found: None, detected_by: None, version: None,
        },
        ServiceDefinition {
            id: "postgresql".into(),
            display_name: "PostgreSQL".into(),
            config_paths: vec![
                "/etc/postgresql/16/main/postgresql.conf".into(),
                "/etc/postgresql/15/main/postgresql.conf".into(),
                "/etc/postgresql/14/main/postgresql.conf".into(),
                "/var/lib/pgsql/data/postgresql.conf".into(),
            ],
            systemd_names: vec![
                "postgresql".into(),
                "postgresql@16-main".into(),
                "postgresql@15-main".into(),
            ],
            process_name: "postgres".into(),
            binary_names: vec!["psql".into(), "postgres".into(), "pg_isready".into()],
            is_running: None, is_installed: None, config_found: None, detected_by: None, version: None,
        },
        ServiceDefinition {
            id: "redis".into(),
            display_name: "Redis".into(),
            config_paths: vec![
                "/etc/redis/redis.conf".into(),
                "/etc/redis.conf".into(),
            ],
            systemd_names: vec!["redis".into(), "redis-server".into()],
            process_name: "redis".into(),
            binary_names: vec!["redis-server".into(), "redis-cli".into()],
            is_running: None, is_installed: None, config_found: None, detected_by: None, version: None,
        },
        ServiceDefinition {
            id: "php-fpm".into(),
            display_name: "PHP-FPM".into(),
            config_paths: vec![
                "/etc/php-fpm.d/www.conf".into(),
                "/etc/php/8.3/fpm/pool.d/www.conf".into(),
                "/etc/php/8.2/fpm/pool.d/www.conf".into(),
                "/etc/php/8.1/fpm/pool.d/www.conf".into(),
            ],
            systemd_names: vec![
                "php-fpm".into(),
                "php8.3-fpm".into(),
                "php8.2-fpm".into(),
                "php8.1-fpm".into(),
            ],
            process_name: "php-fpm".into(),
            binary_names: vec!["php-fpm".into(), "php-fpm8.3".into(), "php-fpm8.2".into(), "php-fpm8.1".into()],
            is_running: None, is_installed: None, config_found: None, detected_by: None, version: None,
        },
        ServiceDefinition {
            id: "sshd".into(),
            display_name: "SSH Server".into(),
            config_paths: vec!["/etc/ssh/sshd_config".into()],
            systemd_names: vec!["sshd".into(), "ssh".into()],
            process_name: "sshd".into(),
            binary_names: vec!["sshd".into()],
            is_running: None, is_installed: None, config_found: None, detected_by: None, version: None,
        },
    ]
}

// --- Detection helpers ---

fn check_binary_exists(names: &[String]) -> Option<String> {
    for name in names {
        if let Ok(output) = Command::new("which").arg(name).output() {
            if output.status.success() {
                return Some(name.clone());
            }
        }
    }
    None
}

fn check_systemd_registered(names: &[String]) -> Option<String> {
    for name in names {
        if let Ok(output) = Command::new("systemctl")
            .args(["list-unit-files", &format!("{}.service", name)])
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if stdout.contains(&format!("{}.service", name)) {
                return Some(name.clone());
            }
        }
    }
    None
}

fn check_service_running(def: &ServiceDefinition) -> bool {
    for svc in &def.systemd_names {
        if let Ok(output) = Command::new("systemctl").args(["is-active", svc]).output() {
            if String::from_utf8_lossy(&output.stdout).trim() == "active" {
                return true;
            }
        }
    }
    if let Ok(output) = Command::new("pgrep").args(["-f", &def.process_name]).output() {
        return output.status.success();
    }
    false
}

fn detect_version(def: &ServiceDefinition) -> Option<String> {
    let version_cmds: Vec<(&str, &[&str])> = match def.id.as_str() {
        "apache" => vec![("httpd", &["-v"][..]), ("apache2", &["-v"][..])],
        "nginx" => vec![("nginx", &["-v"][..])],
        "mysql" => vec![("mysql", &["--version"][..]), ("mariadb", &["--version"][..])],
        "postgresql" => vec![("psql", &["--version"][..])],
        "redis" => vec![("redis-server", &["--version"][..])],
        "php-fpm" => vec![("php", &["--version"][..])],
        "sshd" => vec![("sshd", &["-V"][..])],
        "tomcat" => vec![],
        _ => vec![],
    };

    for (cmd, args) in version_cmds {
        if let Ok(output) = Command::new(cmd).args(args).output() {
            // Some programs output version to stderr (nginx -v, sshd -V)
            let text = if output.stdout.is_empty() {
                String::from_utf8_lossy(&output.stderr).to_string()
            } else {
                String::from_utf8_lossy(&output.stdout).to_string()
            };
            let first_line = text.lines().next().unwrap_or("").trim().to_string();
            if !first_line.is_empty() {
                return Some(first_line);
            }
        }
    }

    // Tomcat: read RELEASE-NOTES or version.sh
    if def.id == "tomcat" {
        let dirs = ["/opt/tomcat", "/usr/share/tomcat", "/usr/share/tomcat9", "/usr/share/tomcat10"];
        for dir in &dirs {
            let release = format!("{}/RELEASE-NOTES", dir);
            if let Ok(content) = fs::read_to_string(&release) {
                for line in content.lines().take(10) {
                    if line.contains("Apache Tomcat Version") || line.contains("Release Notes") {
                        return Some(line.trim().to_string());
                    }
                }
            }
        }
    }

    None
}

/// Comprehensive install detection: binary + systemd + config + running process
fn detect_service(def: &mut ServiceDefinition) {
    let mut reasons: Vec<String> = Vec::new();

    // 1. Binary check (which)
    if let Some(bin) = check_binary_exists(&def.binary_names) {
        reasons.push(format!("binary: {}", bin));
    }

    // 2. systemd unit registered
    if let Some(unit) = check_systemd_registered(&def.systemd_names) {
        reasons.push(format!("systemd: {}.service", unit));
    }

    // 3. Config file exists
    let config_exists = def.config_paths.iter().any(|p| fs::metadata(p).is_ok());
    if config_exists {
        reasons.push("config file found".into());
    }

    // 4. Running process
    let running = check_service_running(def);
    if running {
        reasons.push("process running".into());
    }

    // 5. Version info
    let version = if !reasons.is_empty() {
        detect_version(def)
    } else {
        None
    };

    def.is_running = Some(running);
    def.config_found = Some(config_exists);
    def.is_installed = Some(!reasons.is_empty());
    def.detected_by = if reasons.is_empty() { None } else { Some(reasons) };
    def.version = version;
}

fn find_service(service_id: &str) -> Option<ServiceDefinition> {
    service_registry().into_iter().find(|s| s.id == service_id)
}

fn validate_service_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() < 64
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
        .map(|mut def| {
            detect_service(&mut def);
            def
        })
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

async fn update_config(
    path: web::Path<String>,
    body: web::Json<UpdateConfigRequest>,
) -> HttpResponse {
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

    // Create backup
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
            } else {
                Err("Config file not found".into())
            }
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

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/config")
            .route("/services", web::get().to(list_services))
            .route("/{service_id}", web::get().to(get_config))
            .route("/{service_id}", web::post().to(update_config))
            .route("/{service_id}/restart", web::post().to(restart_service_handler))
            .route("/{service_id}/validate", web::post().to(validate_config)),
    );
}
