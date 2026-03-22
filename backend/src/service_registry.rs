use std::fs;
use std::process::Command;
use crate::models::ServiceDefinition;

pub fn service_registry() -> Vec<ServiceDefinition> {
    vec![
        svc("apache", "Apache HTTP Server",
            &["/etc/httpd/conf/httpd.conf", "/etc/apache2/apache2.conf", "/etc/apache2/sites-enabled/000-default.conf"],
            &["httpd", "apache2"], "apache", &["httpd", "apache2", "apache2ctl"]),
        svc("tomcat", "Apache Tomcat",
            &["/etc/tomcat/server.xml", "/opt/tomcat/conf/server.xml", "/usr/share/tomcat/conf/server.xml", "/etc/tomcat9/server.xml", "/etc/tomcat10/server.xml"],
            &["tomcat", "tomcat9", "tomcat10"], "tomcat", &["catalina.sh"]),
        svc("nginx", "Nginx",
            &["/etc/nginx/nginx.conf", "/etc/nginx/sites-enabled/default"],
            &["nginx"], "nginx", &["nginx"]),
        svc("mysql", "MySQL / MariaDB",
            &["/etc/mysql/mysql.conf.d/mysqld.cnf", "/etc/mysql/my.cnf", "/etc/my.cnf", "/etc/mysql/mariadb.conf.d/50-server.cnf"],
            &["mysql", "mysqld", "mariadb"], "mysql", &["mysql", "mysqld", "mariadb", "mariadbd"]),
        svc("postgresql", "PostgreSQL",
            &["/etc/postgresql/16/main/postgresql.conf", "/etc/postgresql/15/main/postgresql.conf", "/etc/postgresql/14/main/postgresql.conf", "/var/lib/pgsql/data/postgresql.conf"],
            &["postgresql", "postgresql@16-main", "postgresql@15-main"], "postgres", &["psql", "postgres", "pg_isready"]),
        svc("redis", "Redis",
            &["/etc/redis/redis.conf", "/etc/redis.conf"],
            &["redis", "redis-server"], "redis", &["redis-server", "redis-cli"]),
        svc("php-fpm", "PHP-FPM",
            &["/etc/php-fpm.d/www.conf", "/etc/php/8.3/fpm/pool.d/www.conf", "/etc/php/8.2/fpm/pool.d/www.conf", "/etc/php/8.1/fpm/pool.d/www.conf"],
            &["php-fpm", "php8.3-fpm", "php8.2-fpm", "php8.1-fpm"], "php-fpm", &["php-fpm", "php-fpm8.3", "php-fpm8.2", "php-fpm8.1"]),
        svc("sshd", "SSH Server",
            &["/etc/ssh/sshd_config"],
            &["sshd", "ssh"], "sshd", &["sshd"]),
    ]
}

fn svc(id: &str, name: &str, configs: &[&str], systemd: &[&str], proc_name: &str, bins: &[&str]) -> ServiceDefinition {
    ServiceDefinition {
        id: id.into(), display_name: name.into(),
        config_paths: configs.iter().map(|s| s.to_string()).collect(),
        systemd_names: systemd.iter().map(|s| s.to_string()).collect(),
        process_name: proc_name.into(),
        binary_names: bins.iter().map(|s| s.to_string()).collect(),
        is_running: None, is_installed: None, config_found: None, detected_by: None, version: None,
    }
}

pub fn find_service(service_id: &str) -> Option<ServiceDefinition> {
    service_registry().into_iter().find(|s| s.id == service_id)
}

pub fn detect_service(def: &mut ServiceDefinition) {
    let mut reasons: Vec<String> = Vec::new();

    if let Some(bin) = check_binary_exists(&def.binary_names) {
        reasons.push(format!("binary: {}", bin));
    }
    if let Some(unit) = check_systemd_registered(&def.systemd_names) {
        reasons.push(format!("systemd: {}.service", unit));
    }
    let config_exists = def.config_paths.iter().any(|p| fs::metadata(p).is_ok());
    if config_exists { reasons.push("config file found".into()); }

    let running = check_service_running(def);
    if running { reasons.push("process running".into()); }

    let version = if !reasons.is_empty() { detect_version(def) } else { None };

    def.is_running = Some(running);
    def.config_found = Some(config_exists);
    def.is_installed = Some(!reasons.is_empty());
    def.detected_by = if reasons.is_empty() { None } else { Some(reasons) };
    def.version = version;
}

pub fn check_service_running(def: &ServiceDefinition) -> bool {
    for svc in &def.systemd_names {
        if let Ok(output) = Command::new("systemctl").args(["is-active", svc]).output() {
            if String::from_utf8_lossy(&output.stdout).trim() == "active" { return true; }
        }
    }
    Command::new("pgrep").args(["-f", &def.process_name]).output()
        .map(|o| o.status.success()).unwrap_or(false)
}

fn check_binary_exists(names: &[String]) -> Option<String> {
    for name in names {
        if let Ok(output) = Command::new("which").arg(name).output() {
            if output.status.success() { return Some(name.clone()); }
        }
    }
    None
}

fn check_systemd_registered(names: &[String]) -> Option<String> {
    for name in names {
        if let Ok(output) = Command::new("systemctl").args(["list-unit-files", &format!("{}.service", name)]).output() {
            if String::from_utf8_lossy(&output.stdout).contains(&format!("{}.service", name)) {
                return Some(name.clone());
            }
        }
    }
    None
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
            let text = if output.stdout.is_empty() {
                String::from_utf8_lossy(&output.stderr).to_string()
            } else {
                String::from_utf8_lossy(&output.stdout).to_string()
            };
            let first_line = text.lines().next().unwrap_or("").trim().to_string();
            if !first_line.is_empty() { return Some(first_line); }
        }
    }

    if def.id == "tomcat" {
        for dir in &["/opt/tomcat", "/usr/share/tomcat", "/usr/share/tomcat9", "/usr/share/tomcat10"] {
            if let Ok(content) = fs::read_to_string(format!("{}/RELEASE-NOTES", dir)) {
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
