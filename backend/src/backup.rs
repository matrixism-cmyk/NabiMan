use actix_web::{web, HttpResponse};
use crate::models::ApiResponse;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Serialize, Clone)]
pub struct BackupEntry {
    pub id: String,
    pub source: String,
    pub filename: String,
    pub size: u64,
    pub created: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct BackupSchedule {
    pub id: String,
    pub name: String,
    pub paths: Vec<String>,
    pub cron_expr: String,
    pub compression: bool,
    pub enabled: bool,
    pub last_run: Option<String>,
    pub next_run: Option<String>,
}

pub type BackupScheduleStore = Arc<Mutex<Vec<BackupSchedule>>>;

pub fn new_schedule_store() -> BackupScheduleStore {
    let path = schedules_file();
    let schedules = std::fs::read_to_string(&path).ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    Arc::new(Mutex::new(schedules))
}

fn backup_dir() -> std::path::PathBuf {
    let dir = std::env::var("NABIMAN_DATA_DIR").unwrap_or_else(|_| "/var/lib/nabiman".into());
    std::path::PathBuf::from(dir).join("backups")
}

fn schedules_file() -> std::path::PathBuf {
    let dir = std::env::var("NABIMAN_DATA_DIR").unwrap_or_else(|_| "/var/lib/nabiman".into());
    std::path::PathBuf::from(dir).join("backup_schedules.json")
}

fn save_schedules(schedules: &[BackupSchedule]) -> Result<(), String> {
    let json = serde_json::to_string_pretty(schedules).map_err(|e| e.to_string())?;
    std::fs::write(schedules_file(), json).map_err(|e| e.to_string())
}

fn gen_id() -> String {
    use rand::Rng;
    format!("{:016x}", rand::thread_rng().gen::<u64>())
}

async fn list_backups() -> HttpResponse {
    let dir = backup_dir();
    if !dir.exists() {
        return HttpResponse::Ok().json(ApiResponse::ok(Vec::<BackupEntry>::new()));
    }
    let mut entries = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for entry in rd.flatten() {
            let meta = match entry.metadata() {
                Ok(m) => m, Err(_) => continue,
            };
            if !meta.is_file() { continue; }
            let name = entry.file_name().to_string_lossy().to_string();
            let created = meta.modified().ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| chrono::DateTime::from_timestamp(d.as_secs() as i64, 0)
                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                    .unwrap_or_default())
                .unwrap_or_default();
            let source = name.rsplitn(2, '_').last().unwrap_or("").replace("__", "/");
            entries.push(BackupEntry {
                id: name.clone(), source, filename: name, size: meta.len(), created,
            });
        }
    }
    entries.sort_by(|a, b| b.created.cmp(&a.created));
    HttpResponse::Ok().json(ApiResponse::ok(entries))
}

#[derive(Deserialize)]
pub struct BackupRequest {
    pub path: String,
}

async fn create_backup(body: web::Json<BackupRequest>) -> HttpResponse {
    let src = std::path::Path::new(&body.path);
    if !src.exists() || !src.is_file() {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("File not found"));
    }
    let dir = backup_dir();
    if let Err(e) = std::fs::create_dir_all(&dir) {
        return HttpResponse::Ok().json(ApiResponse::<String>::error(&e.to_string()));
    }
    let safe_name = body.path.replace('/', "__").trim_start_matches('_').to_string();
    let ts = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let filename = format!("{}_{}.bak", safe_name, ts);
    let dest = dir.join(&filename);

    match std::fs::copy(src, &dest) {
        Ok(_) => HttpResponse::Ok().json(ApiResponse::ok(format!("Backup created: {}", filename))),
        Err(e) => HttpResponse::Ok().json(ApiResponse::<String>::error(&e.to_string())),
    }
}

#[derive(Deserialize)]
pub struct RestoreRequest {
    pub filename: String,
    pub target: String,
}

async fn restore_backup(body: web::Json<RestoreRequest>) -> HttpResponse {
    let fname = &body.filename;
    if fname.contains("..") || fname.contains('/') {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Invalid filename"));
    }
    let src = backup_dir().join(fname);
    if !src.exists() {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Backup file not found"));
    }
    let target = std::path::Path::new(&body.target);
    match std::fs::copy(&src, target) {
        Ok(_) => HttpResponse::Ok().json(ApiResponse::ok("Restored".to_string())),
        Err(e) => HttpResponse::Ok().json(ApiResponse::<String>::error(&e.to_string())),
    }
}

#[derive(Deserialize)]
pub struct DeleteBackupRequest {
    pub filename: String,
}

async fn delete_backup(body: web::Json<DeleteBackupRequest>) -> HttpResponse {
    let fname = &body.filename;
    if fname.contains("..") || fname.contains('/') {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Invalid filename"));
    }
    let path = backup_dir().join(fname);
    match std::fs::remove_file(&path) {
        Ok(_) => HttpResponse::Ok().json(ApiResponse::ok("Deleted".to_string())),
        Err(e) => HttpResponse::Ok().json(ApiResponse::<String>::error(&e.to_string())),
    }
}

// --- Compressed backup ---

#[derive(Deserialize)]
struct CompressedBackupRequest {
    paths: Vec<String>,
    name: Option<String>,
}

async fn create_compressed(body: web::Json<CompressedBackupRequest>) -> HttpResponse {
    if body.paths.is_empty() {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("No paths provided"));
    }
    let dir = backup_dir();
    if let Err(e) = std::fs::create_dir_all(&dir) {
        return HttpResponse::Ok().json(ApiResponse::<String>::error(&e.to_string()));
    }
    let ts = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let base = body.name.as_deref().unwrap_or("archive");
    let filename = format!("{}_{}.tar.gz", base, ts);
    let dest = dir.join(&filename);

    let mut cmd = std::process::Command::new("tar");
    cmd.arg("-czf").arg(&dest);
    for p in &body.paths {
        if p.contains("..") { continue; }
        cmd.arg(p);
    }
    match cmd.output() {
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

// --- Encrypted backup ---

#[derive(Deserialize)]
struct EncryptedBackupRequest {
    paths: Vec<String>,
    name: Option<String>,
    password: String,
}

async fn create_encrypted(body: web::Json<EncryptedBackupRequest>) -> HttpResponse {
    if body.paths.is_empty() {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("No paths provided"));
    }
    if body.password.len() < 8 {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Password must be at least 8 characters"));
    }
    let dir = backup_dir();
    if let Err(e) = std::fs::create_dir_all(&dir) {
        return HttpResponse::Ok().json(ApiResponse::<String>::error(&e.to_string()));
    }
    let ts = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let base = body.name.as_deref().unwrap_or("archive");
    let tar_name = format!("{}_{}.tar.gz", base, ts);
    let enc_name = format!("{}_{}.tar.gz.enc", base, ts);
    let tar_path = dir.join(&tar_name);
    let enc_path = dir.join(&enc_name);

    // Step 1: Create tar.gz
    let mut cmd = std::process::Command::new("tar");
    cmd.arg("-czf").arg(&tar_path);
    for p in &body.paths {
        if p.contains("..") { continue; }
        cmd.arg(p);
    }
    let tar_result = cmd.output();
    if tar_result.as_ref().map(|o| !o.status.success()).unwrap_or(true) {
        let _ = std::fs::remove_file(&tar_path);
        return HttpResponse::Ok().json(ApiResponse::<String>::error("tar failed"));
    }

    // Step 2: Encrypt with openssl aes-256-cbc
    let enc_result = std::process::Command::new("openssl")
        .args(["enc", "-aes-256-cbc", "-pbkdf2", "-salt",
               "-in", tar_path.to_str().unwrap_or(""),
               "-out", enc_path.to_str().unwrap_or(""),
               "-pass", &format!("pass:{}", body.password)])
        .output();

    let _ = std::fs::remove_file(&tar_path); // Remove unencrypted tar

    match enc_result {
        Ok(o) if o.status.success() => HttpResponse::Ok().json(ApiResponse::ok(enc_name)),
        Ok(o) => {
            let _ = std::fs::remove_file(&enc_path);
            HttpResponse::Ok().json(ApiResponse::<String>::error(&String::from_utf8_lossy(&o.stderr).to_string()))
        }
        Err(e) => HttpResponse::Ok().json(ApiResponse::<String>::error(&e.to_string())),
    }
}

#[derive(Deserialize)]
struct EncryptedRestoreRequest {
    filename: String,
    target_dir: String,
    password: String,
}

async fn restore_encrypted(body: web::Json<EncryptedRestoreRequest>) -> HttpResponse {
    let fname = &body.filename;
    if fname.contains("..") || fname.contains('/') || !fname.ends_with(".enc") {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Invalid encrypted backup filename"));
    }
    let enc_path = backup_dir().join(fname);
    if !enc_path.exists() {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Backup file not found"));
    }

    let tar_name = fname.trim_end_matches(".enc");
    let tar_path = backup_dir().join(tar_name);

    // Step 1: Decrypt
    let dec_result = std::process::Command::new("openssl")
        .args(["enc", "-d", "-aes-256-cbc", "-pbkdf2",
               "-in", enc_path.to_str().unwrap_or(""),
               "-out", tar_path.to_str().unwrap_or(""),
               "-pass", &format!("pass:{}", body.password)])
        .output();

    if dec_result.as_ref().map(|o| !o.status.success()).unwrap_or(true) {
        let _ = std::fs::remove_file(&tar_path);
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Decryption failed (wrong password?)"));
    }

    // Step 2: Extract tar
    let extract = std::process::Command::new("tar")
        .args(["-xzf", tar_path.to_str().unwrap_or(""), "-C", &body.target_dir])
        .output();

    let _ = std::fs::remove_file(&tar_path);

    match extract {
        Ok(o) if o.status.success() => HttpResponse::Ok().json(ApiResponse::ok("Restored and decrypted")),
        Ok(o) => HttpResponse::Ok().json(ApiResponse::<String>::error(&String::from_utf8_lossy(&o.stderr).to_string())),
        Err(e) => HttpResponse::Ok().json(ApiResponse::<String>::error(&e.to_string())),
    }
}

// --- Schedule endpoints ---

async fn list_schedules(store: web::Data<BackupScheduleStore>) -> HttpResponse {
    HttpResponse::Ok().json(ApiResponse::ok(store.lock().unwrap().clone()))
}

async fn create_schedule(
    body: web::Json<BackupSchedule>,
    store: web::Data<BackupScheduleStore>,
) -> HttpResponse {
    let mut sched = body.into_inner();
    sched.id = gen_id();
    sched.last_run = None;
    let mut schedules = store.lock().unwrap();
    schedules.push(sched);
    let _ = save_schedules(&schedules);
    HttpResponse::Ok().json(ApiResponse::ok("Schedule created"))
}

async fn delete_schedule(
    path: web::Path<String>,
    store: web::Data<BackupScheduleStore>,
) -> HttpResponse {
    let id = path.into_inner();
    let mut schedules = store.lock().unwrap();
    schedules.retain(|s| s.id != id);
    let _ = save_schedules(&schedules);
    HttpResponse::Ok().json(ApiResponse::ok("Schedule deleted"))
}

// --- Cron-based scheduler ---

fn cron_matches(expr: &str, now: &chrono::DateTime<chrono::Utc>) -> bool {
    let parts: Vec<&str> = expr.split_whitespace().collect();
    if parts.len() != 5 { return false; }
    let fields = [
        (now.format("%M").to_string(), parts[0]),
        (now.format("%H").to_string(), parts[1]),
        (now.format("%d").to_string(), parts[2]),
        (now.format("%m").to_string(), parts[3]),
        (now.format("%u").to_string(), parts[4]), // 1=Monday .. 7=Sunday
    ];
    fields.iter().all(|(current, pattern)| {
        if *pattern == "*" { return true; }
        let cur: u32 = current.parse().unwrap_or(0);
        pattern.split(',').any(|p| p.trim().parse::<u32>().map_or(false, |v| v == cur))
    })
}

fn run_scheduled_backup(sched: &BackupSchedule) {
    let dir = backup_dir();
    let _ = std::fs::create_dir_all(&dir);
    let ts = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let filename = format!("{}_{}.tar.gz", sched.name, ts);
    let dest = dir.join(&filename);

    let mut cmd = std::process::Command::new("tar");
    if sched.compression { cmd.arg("-czf"); } else { cmd.arg("-cf"); }
    cmd.arg(&dest);
    for p in &sched.paths { cmd.arg(p); }
    let _ = cmd.output();
}

pub fn start_backup_scheduler(store: BackupScheduleStore) {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(60));
            let now = chrono::Utc::now();
            let mut schedules = store.lock().unwrap();
            for sched in schedules.iter_mut() {
                if !sched.enabled { continue; }
                if cron_matches(&sched.cron_expr, &now) {
                    run_scheduled_backup(sched);
                    sched.last_run = Some(now.to_rfc3339());
                }
            }
            let _ = save_schedules(&schedules);
        }
    });
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/backup")
            .route("", web::get().to(list_backups))
            .route("/create", web::post().to(create_backup))
            .route("/restore", web::post().to(restore_backup))
            .route("/delete", web::post().to(delete_backup))
            .route("/create-compressed", web::post().to(create_compressed))
            .route("/create-encrypted", web::post().to(create_encrypted))
            .route("/restore-encrypted", web::post().to(restore_encrypted))
            .route("/schedules", web::get().to(list_schedules))
            .route("/schedules", web::post().to(create_schedule))
            .route("/schedules/{id}", web::delete().to(delete_schedule))
    );
}
