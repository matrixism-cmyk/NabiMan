use actix_web::{web, HttpResponse};
use crate::models::ApiResponse;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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

#[derive(Serialize, Clone)]
pub struct DbQueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub row_count: usize,
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

pub fn db_backup_dir() -> PathBuf {
    let dir = std::env::var("NABIMAN_DATA_DIR").unwrap_or_else(|_| "/var/lib/nabiman".into());
    PathBuf::from(dir).join("db_backups")
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

// --- New backup/restore/query endpoints ---

#[derive(Deserialize)]
struct DbBackupRequest {
    database: String,
}

async fn db_backup(body: web::Json<DbBackupRequest>) -> HttpResponse {
    let db = &body.database;
    if db.contains("..") || db.contains('/') || db.contains(';') {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Invalid database name"));
    }
    let engine = match detect_db() {
        Some(e) => e,
        None => return HttpResponse::Ok().json(ApiResponse::<String>::error("No database running")),
    };
    let dir = db_backup_dir();
    if let Err(e) = std::fs::create_dir_all(&dir) {
        return HttpResponse::Ok().json(ApiResponse::<String>::error(&e.to_string()));
    }
    let ts = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let filename = format!("db_{}_{}.sql.gz", db, ts);
    let dest = dir.join(&filename);

    let cmd = match engine {
        "mysql" => format!("mysqldump '{}' | gzip > '{}'", db, dest.display()),
        "postgresql" => format!(
            "sudo -u postgres pg_dump '{}' | gzip > '{}'", db, dest.display()
        ),
        _ => return HttpResponse::Ok().json(ApiResponse::<String>::error("Unknown engine")),
    };
    let result = std::process::Command::new("sh").args(["-c", &cmd]).output();
    match result {
        Ok(o) if o.status.success() => {
            HttpResponse::Ok().json(ApiResponse::ok(filename))
        }
        Ok(o) => {
            let err = String::from_utf8_lossy(&o.stderr).to_string();
            HttpResponse::Ok().json(ApiResponse::<String>::error(&err))
        }
        Err(e) => HttpResponse::Ok().json(ApiResponse::<String>::error(&e.to_string())),
    }
}

async fn list_db_backups() -> HttpResponse {
    let dir = db_backup_dir();
    if !dir.exists() {
        return HttpResponse::Ok().json(ApiResponse::ok(Vec::<String>::new()));
    }
    let mut files: Vec<String> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for entry in rd.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with(".sql.gz") {
                files.push(name);
            }
        }
    }
    files.sort_by(|a, b| b.cmp(a));
    HttpResponse::Ok().json(ApiResponse::ok(files))
}

#[derive(Deserialize)]
struct DbRestoreRequest {
    database: String,
    filename: String,
}

async fn db_restore(body: web::Json<DbRestoreRequest>) -> HttpResponse {
    let db = &body.database;
    let fname = &body.filename;
    if fname.contains("..") || fname.contains('/') {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Invalid filename"));
    }
    if db.contains("..") || db.contains('/') || db.contains(';') {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Invalid database name"));
    }
    let src = db_backup_dir().join(fname);
    if !src.exists() {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Backup file not found"));
    }
    let engine = match detect_db() {
        Some(e) => e,
        None => return HttpResponse::Ok().json(ApiResponse::<String>::error("No database running")),
    };
    let cmd = match engine {
        "mysql" => format!("gunzip -c '{}' | mysql '{}'", src.display(), db),
        "postgresql" => format!(
            "gunzip -c '{}' | sudo -u postgres psql '{}'", src.display(), db
        ),
        _ => return HttpResponse::Ok().json(ApiResponse::<String>::error("Unknown engine")),
    };
    let result = std::process::Command::new("sh").args(["-c", &cmd]).output();
    match result {
        Ok(o) if o.status.success() => {
            HttpResponse::Ok().json(ApiResponse::ok("Restore completed".to_string()))
        }
        Ok(o) => {
            let err = String::from_utf8_lossy(&o.stderr).to_string();
            HttpResponse::Ok().json(ApiResponse::<String>::error(&err))
        }
        Err(e) => HttpResponse::Ok().json(ApiResponse::<String>::error(&e.to_string())),
    }
}

#[derive(Deserialize)]
struct DbQueryRequest {
    database: String,
    query: String,
}

fn is_safe_query(q: &str) -> bool {
    let trimmed = q.trim().to_uppercase();
    ["SELECT", "SHOW", "DESCRIBE", "EXPLAIN"]
        .iter()
        .any(|prefix| trimmed.starts_with(prefix))
}

async fn db_query(body: web::Json<DbQueryRequest>) -> HttpResponse {
    let db = &body.database;
    let query = &body.query;
    if !is_safe_query(query) {
        return HttpResponse::Ok().json(
            ApiResponse::<DbQueryResult>::error("Only SELECT/SHOW/DESCRIBE/EXPLAIN allowed")
        );
    }
    if db.contains("..") || db.contains('/') || db.contains(';') {
        return HttpResponse::Ok().json(
            ApiResponse::<DbQueryResult>::error("Invalid database name")
        );
    }
    let engine = match detect_db() {
        Some(e) => e,
        None => return HttpResponse::Ok().json(
            ApiResponse::<DbQueryResult>::error("No database running")
        ),
    };
    let output = match engine {
        "mysql" => {
            std::process::Command::new("mysql")
                .args(["-BN", db, "-e", query])
                .output()
        }
        "postgresql" => {
            std::process::Command::new("sudo")
                .args(["-u", "postgres", "psql", "-d", db, "-tA", "-F", "\t", "-c", query])
                .output()
        }
        _ => return HttpResponse::Ok().json(
            ApiResponse::<DbQueryResult>::error("Unknown engine")
        ),
    };
    match output {
        Ok(o) if o.status.success() => {
            let stdout = String::from_utf8_lossy(&o.stdout).to_string();
            let mut rows: Vec<Vec<String>> = Vec::new();
            for line in stdout.lines() {
                if line.trim().is_empty() { continue; }
                rows.push(line.split('\t').map(|s| s.to_string()).collect());
            }
            let col_count = rows.first().map(|r| r.len()).unwrap_or(0);
            let columns: Vec<String> = (0..col_count)
                .map(|i| format!("col{}", i + 1))
                .collect();
            let row_count = rows.len();
            HttpResponse::Ok().json(ApiResponse::ok(DbQueryResult {
                columns, rows, row_count,
            }))
        }
        Ok(o) => {
            let err = String::from_utf8_lossy(&o.stderr).to_string();
            HttpResponse::Ok().json(ApiResponse::<DbQueryResult>::error(&err))
        }
        Err(e) => HttpResponse::Ok().json(
            ApiResponse::<DbQueryResult>::error(&e.to_string())
        ),
    }
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/database")
            .route("/available", web::get().to(available))
            .route("/status", web::get().to(status))
            .route("/backup", web::post().to(db_backup))
            .route("/backups", web::get().to(list_db_backups))
            .route("/restore", web::post().to(db_restore))
            .route("/query", web::post().to(db_query))
    );
}
