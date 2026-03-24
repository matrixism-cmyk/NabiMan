use actix_web::{web, HttpResponse};
use crate::models::ApiResponse;
use serde::Serialize;

#[derive(Serialize, Clone)]
pub struct DbStatus {
    pub engine: String,
    pub running: bool,
    pub version: String,
    pub uptime: String,
    pub connections: String,
    pub databases: Vec<String>,
    pub slow_queries: String,
}

fn detect_db() -> Option<&'static str> {
    let checks = [
        ("mysql", &["mysqld", "mariadbd"][..]),
        ("postgresql", &["postgres"][..]),
    ];
    for (name, procs) in &checks {
        for p in *procs {
            let ok = std::process::Command::new("pgrep").arg("-x").arg(p)
                .output().map(|o| o.status.success()).unwrap_or(false);
            if ok { return Some(name); }
        }
    }
    None
}

fn mysql_status() -> DbStatus {
    let run = |args: &[&str]| -> String {
        std::process::Command::new("mysql")
            .args(args).output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default()
    };

    let version = run(&["-BNe", "SELECT VERSION()"]);
    let uptime = run(&["-BNe", "SHOW STATUS LIKE 'Uptime'"]);
    let uptime_val = uptime.split('\t').nth(1).unwrap_or("0");
    let secs: u64 = uptime_val.parse().unwrap_or(0);
    let uptime_fmt = format!("{}d {}h {}m", secs / 86400, (secs % 86400) / 3600, (secs % 3600) / 60);

    let conns = run(&["-BNe", "SHOW STATUS LIKE 'Threads_connected'"]);
    let conn_val = conns.split('\t').nth(1).unwrap_or("0").to_string();

    let slow = run(&["-BNe", "SHOW STATUS LIKE 'Slow_queries'"]);
    let slow_val = slow.split('\t').nth(1).unwrap_or("0").to_string();

    let dbs_raw = run(&["-BNe", "SHOW DATABASES"]);
    let databases: Vec<String> = dbs_raw.lines()
        .filter(|d| !["information_schema", "performance_schema", "sys"].contains(d))
        .map(|s| s.to_string()).collect();

    DbStatus {
        engine: "MySQL/MariaDB".into(), running: true, version, uptime: uptime_fmt,
        connections: conn_val, databases, slow_queries: slow_val,
    }
}

fn pg_status() -> DbStatus {
    let run = |args: &[&str]| -> String {
        std::process::Command::new("sudo").arg("-u").arg("postgres")
            .arg("psql").args(args).output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default()
    };

    let version = run(&["-tAc", "SELECT version()"]);
    let short_ver = version.split_whitespace().take(2).collect::<Vec<_>>().join(" ");
    let uptime = run(&["-tAc", "SELECT now() - pg_postmaster_start_time()"]);
    let conns = run(&["-tAc", "SELECT count(*) FROM pg_stat_activity"]);
    let dbs_raw = run(&["-tAc", "SELECT datname FROM pg_database WHERE datistemplate=false"]);
    let databases: Vec<String> = dbs_raw.lines().map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty()).collect();

    DbStatus {
        engine: "PostgreSQL".into(), running: true, version: short_ver,
        uptime: uptime.split('.').next().unwrap_or("").to_string(),
        connections: conns, databases, slow_queries: "N/A".into(),
    }
}

async fn status() -> HttpResponse {
    match detect_db() {
        Some("mysql") => HttpResponse::Ok().json(ApiResponse::ok(mysql_status())),
        Some("postgresql") => HttpResponse::Ok().json(ApiResponse::ok(pg_status())),
        _ => HttpResponse::Ok().json(
            ApiResponse::<DbStatus>::error("No supported database server running")
        ),
    }
}

async fn available() -> HttpResponse {
    HttpResponse::Ok().json(ApiResponse::ok(detect_db().is_some()))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/database")
            .route("/available", web::get().to(available))
            .route("/status", web::get().to(status))
    );
}
