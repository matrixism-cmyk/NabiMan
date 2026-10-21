use super::{archive, backup_dir};
use crate::models::ApiResponse;
use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
#[path = "backup_cron.rs"]
mod cron;
use cron::Cron;
#[derive(Serialize, Deserialize, Clone)]
pub struct RunResult {
    pub state: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub filename: Option<String>,
    pub error: Option<String>,
}
#[derive(Serialize, Deserialize, Clone)]
pub struct Schedule {
    pub id: String,
    pub name: String,
    pub paths: Vec<String>,
    pub cron_expr: String,
    pub compression: bool,
    pub enabled: bool,
    #[serde(default)]
    pub last_run: Option<String>,
    #[serde(default)]
    pub next_run: Option<String>,
    #[serde(default)]
    pub last_result: Option<RunResult>,
    #[serde(default)]
    pub last_slot: Option<i64>,
}
pub struct Store {
    path: PathBuf,
    rows: Vec<Schedule>,
    unavailable: bool,
}
pub type BackupScheduleStore = Arc<Mutex<Store>>;
fn save(path: &std::path::Path, rows: &[Schedule]) -> Result<(), String> {
    let temporary = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)?;
        file.write_all(&serde_json::to_vec_pretty(rows)?)?;
        file.sync_all()?;
        std::fs::rename(&temporary, path)?;
        std::fs::File::open(path.parent().ok_or("Missing parent")?)?.sync_all()?;
        Ok(())
    })();
    let _ = std::fs::remove_file(temporary);
    result.map_err(|_| "Backup schedule storage unavailable".into())
}
impl Store {
    fn load(path: PathBuf) -> Self {
        let loaded = match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice::<Vec<Schedule>>(&bytes).map_err(|_| ()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(_) => Err(()),
        };
        let mut store = Self {
            path,
            rows: Vec::new(),
            unavailable: false,
        };
        match loaded {
            Ok(rows) => store.rows = rows,
            Err(_) => {
                store.unavailable = true;
                return store;
            }
        }
        let mut recovered = store.rows.clone();
        let mut changed = false;
        for row in &mut recovered {
            if let Some(run) = &mut row.last_result {
                if run.state == "running" {
                    run.state = "interrupted".into();
                    run.finished_at = Some(chrono::Utc::now().to_rfc3339());
                    run.error=Some("Service restarted before completion was recorded; inspect artifacts before retrying".into());
                    changed = true;
                }
            }
        }
        if changed && store.commit(recovered).is_err() {
            store.unavailable = true;
        }
        store
    }
    fn commit(&mut self, rows: Vec<Schedule>) -> Result<(), String> {
        if self.unavailable {
            return Err("Backup schedule storage unavailable".into());
        }
        if let Err(error) = save(&self.path, &rows) {
            self.unavailable = true;
            return Err(error);
        }
        self.rows = rows;
        Ok(())
    }
}
pub fn new_schedule_store() -> BackupScheduleStore {
    Arc::new(Mutex::new(Store::load(
        nabiman_server::data_dir()
            .join("backup_schedules.json"),
    )))
}
fn failed(message: &str) -> HttpResponse {
    HttpResponse::ServiceUnavailable()
        .insert_header(("Cache-Control", "no-store"))
        .json(ApiResponse::<()>::error(message))
}
pub(super) async fn list(store: web::Data<BackupScheduleStore>) -> HttpResponse {
    let store = store.lock().unwrap();
    if store.unavailable {
        return failed("Backup schedule storage unavailable; existing file preserved");
    }
    HttpResponse::Ok()
        .insert_header(("Cache-Control", "no-store"))
        .json(ApiResponse::ok(store.rows.clone()))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Create {
    name: String,
    paths: Vec<String>,
    cron_expr: String,
    #[serde(default = "yes")]
    compression: bool,
    #[serde(default = "yes")]
    enabled: bool,
}
fn yes() -> bool {
    true
}
pub(super) async fn create(
    body: web::Json<Create>,
    store: web::Data<BackupScheduleStore>,
) -> HttpResponse {
    if let Err(error) = archive::validate(&body.name, &body.paths)
        .and_then(|_| Cron::parse(&body.cron_expr).map(|_| ()))
    {
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error(&error));
    }
    let mut store = store.lock().unwrap();
    let mut rows = store.rows.clone();
    if rows.len() >= 100 {
        return HttpResponse::Conflict().json(ApiResponse::<()>::error(
            "At most 100 schedules are supported",
        ));
    }
    let row = Schedule {
        id: uuid::Uuid::new_v4().to_string(),
        name: body.name.trim().into(),
        paths: body.paths.clone(),
        cron_expr: body.cron_expr.clone(),
        compression: body.compression,
        enabled: body.enabled,
        last_run: None,
        next_run: None,
        last_result: None,
        last_slot: None,
    };
    rows.push(row.clone());
    if let Err(error) = store.commit(rows) {
        return failed(&error);
    }
    HttpResponse::Created().json(ApiResponse::ok(row))
}
pub(super) async fn delete(
    id: web::Path<String>,
    store: web::Data<BackupScheduleStore>,
) -> HttpResponse {
    let mut store = store.lock().unwrap();
    let Some(row) = store.rows.iter().find(|r| r.id == *id) else {
        return HttpResponse::NotFound().json(ApiResponse::<()>::error("Schedule not found"));
    };
    if row
        .last_result
        .as_ref()
        .is_some_and(|r| r.state == "running")
    {
        return HttpResponse::Conflict().json(ApiResponse::<()>::error("Backup is running"));
    }
    let mut rows = store.rows.clone();
    rows.retain(|r| r.id != *id);
    if let Err(error) = store.commit(rows) {
        return failed(&error);
    }
    HttpResponse::Ok().json(ApiResponse::ok("Schedule deleted"))
}
// Persist intent before spawning. There is one archive worker for all schedules.
fn claim(store: &BackupScheduleStore, id: &str, slot: Option<i64>) -> Result<Schedule, String> {
    let mut store = store.lock().unwrap();
    if store.unavailable {
        return Err("Backup schedule storage unavailable".into());
    }
    if store
        .rows
        .iter()
        .any(|r| r.last_result.as_ref().is_some_and(|s| s.state == "running"))
    {
        return Err("Another backup is running".into());
    }
    let mut rows = store.rows.clone();
    let row = rows
        .iter_mut()
        .find(|r| r.id == id)
        .ok_or("Schedule not found")?;
    if slot.is_some()
        && (!row.enabled
            || row
                .last_slot
                .is_some_and(|last| slot.is_some_and(|value| value <= last)))
    {
        return Err("Schedule is not due".into());
    }
    row.last_slot = Some(slot.unwrap_or_else(|| chrono::Utc::now().timestamp() / 60));
    row.last_result = Some(RunResult {
        state: "running".into(),
        started_at: chrono::Utc::now().to_rfc3339(),
        finished_at: None,
        filename: None,
        error: None,
    });
    let result = row.clone();
    store.commit(rows)?;
    Ok(result)
}
fn execute(store: BackupScheduleStore, row: Schedule) {
    let outcome = archive::create(&backup_dir(), &row.name, &row.paths, row.compression);
    let mut store = store.lock().unwrap();
    let mut rows = store.rows.clone();
    if let Some(target) = rows.iter_mut().find(|r| r.id == row.id) {
        let run = target.last_result.as_mut().unwrap();
        run.finished_at = Some(chrono::Utc::now().to_rfc3339());
        match outcome {
            Ok(filename) => {
                run.state = "succeeded".into();
                run.filename = Some(filename);
                target.last_run = run.finished_at.clone();
            }
            Err(error) => {
                run.state = "failed".into();
                run.error = Some(error);
            }
        }
        // Failure leaves a durable running record, recovered as interrupted at startup.
        if store.commit(rows).is_err() {
            eprintln!("Backup completion could not be persisted; scheduler disabled");
        }
    }
}
pub(super) async fn run(
    id: web::Path<String>,
    store: web::Data<BackupScheduleStore>,
) -> HttpResponse {
    let row = match claim(&store, &id, None) {
        Ok(row) => row,
        Err(error) => return HttpResponse::Conflict().json(ApiResponse::<()>::error(&error)),
    };
    let accepted = row.clone();
    let store = store.get_ref().clone();
    std::thread::spawn(move || execute(store, row));
    HttpResponse::Accepted().json(ApiResponse::ok(accepted))
}
pub fn start_backup_scheduler(store: BackupScheduleStore) {
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(5));
        let now = chrono::Utc::now();
        let slot = now.timestamp() / 60;
        let ids = {
            let state = store.lock().unwrap();
            if state.unavailable {
                continue;
            }
            state
                .rows
                .iter()
                .filter(|r| {
                    r.enabled
                        && r.last_slot != Some(slot)
                        && Cron::parse(&r.cron_expr).is_ok_and(|c| c.matches(now))
                })
                .map(|r| r.id.clone())
                .collect::<Vec<_>>()
        };
        for id in ids {
            if chrono::Utc::now().timestamp() / 60 != slot {
                break;
            }
            if let Ok(row) = claim(&store, &id, Some(slot)) {
                execute(store.clone(), row);
            }
        }
    });
}
#[cfg(test)]
#[path = "backup_schedule_tests.rs"]
mod tests;
