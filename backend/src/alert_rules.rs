use actix_web::{web, HttpResponse};
use crate::models::ApiResponse;
use crate::notifications::{ChannelStore, send_notification};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Serialize, Deserialize, Clone)]
pub struct AlertRule {
    pub id: String,
    pub name: String,
    pub metric: String,     // "cpu", "memory", "disk", "service_down"
    pub threshold: f64,     // percentage threshold
    pub duration_secs: u64, // how long before alerting
    pub enabled: bool,
    #[serde(default)]
    pub last_triggered: Option<String>,
}

pub type RuleStore = Arc<Mutex<Vec<AlertRule>>>;

pub fn new_rule_store() -> RuleStore {
    let path = rules_file();
    let rules = std::fs::read_to_string(&path).ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    Arc::new(Mutex::new(rules))
}

fn rules_file() -> std::path::PathBuf {
    let dir = std::env::var("NABIMAN_DATA_DIR").unwrap_or_else(|_| "/var/lib/nabiman".into());
    std::path::PathBuf::from(dir).join("alert_rules.json")
}

fn save_rules(rules: &[AlertRule]) -> Result<(), String> {
    let json = serde_json::to_string_pretty(rules).map_err(|e| e.to_string())?;
    std::fs::write(rules_file(), json).map_err(|e| e.to_string())
}

fn gen_id() -> String {
    use rand::Rng;
    format!("{:016x}", rand::thread_rng().gen::<u64>())
}

pub fn start_alert_checker(rules: RuleStore, channels: ChannelStore) {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(30));
            check_alerts(&rules, &channels);
        }
    });
}

fn check_alerts(rules: &RuleStore, channels: &ChannelStore) {
    let cpu = read_cpu();
    let mem = read_memory();
    let disk = read_disk();

    let mut rules = rules.lock().unwrap();
    let channels = channels.lock().unwrap();
    let now = chrono::Utc::now();

    for rule in rules.iter_mut().filter(|r| r.enabled) {
        let value = match rule.metric.as_str() {
            "cpu" => cpu, "memory" => mem, "disk" => disk, _ => continue,
        };
        if value > rule.threshold {
            // Cooldown: don't alert again within duration_secs
            let should_alert = rule.last_triggered.as_ref().map_or(true, |ts| {
                chrono::DateTime::parse_from_rfc3339(ts).map_or(true, |last| {
                    (now - last.with_timezone(&chrono::Utc)).num_seconds() as u64 > rule.duration_secs
                })
            });
            if should_alert {
                let subject = format!("[NabiMan] {} Alert: {:.1}%", rule.name, value);
                let body = format!("{} is at {:.1}%, threshold is {:.1}%", rule.metric, value, rule.threshold);
                send_notification(&channels, &subject, &body);
                rule.last_triggered = Some(now.to_rfc3339());
            }
        }
    }
    let _ = save_rules(&rules);
}

fn read_cpu() -> f64 {
    let s = std::fs::read_to_string("/proc/loadavg").unwrap_or_default();
    let load: f64 = s.split_whitespace().next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
    let cpus = std::fs::read_to_string("/proc/cpuinfo").unwrap_or_default()
        .lines().filter(|l| l.starts_with("processor")).count().max(1) as f64;
    (load / cpus * 100.0).min(100.0)
}

fn read_memory() -> f64 {
    let s = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
    let (mut total, mut avail) = (0u64, 0u64);
    for l in s.lines() {
        if l.starts_with("MemTotal:") { total = parse_kb(l); }
        else if l.starts_with("MemAvailable:") { avail = parse_kb(l); }
    }
    if total > 0 { (total - avail) as f64 / total as f64 * 100.0 } else { 0.0 }
}

fn read_disk() -> f64 {
    let output = std::process::Command::new("df").args(["--output=pcent", "/"]).output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string()).unwrap_or_default();
    output.lines().last().and_then(|l| l.trim().trim_end_matches('%').parse().ok()).unwrap_or(0.0)
}

fn parse_kb(line: &str) -> u64 {
    line.split_whitespace().nth(1).and_then(|v| v.parse().ok()).unwrap_or(0)
}

async fn list_rules(store: web::Data<RuleStore>) -> HttpResponse {
    HttpResponse::Ok().json(ApiResponse::ok(store.lock().unwrap().clone()))
}

async fn add_rule(body: web::Json<AlertRule>, store: web::Data<RuleStore>) -> HttpResponse {
    let mut rule = body.into_inner();
    rule.id = gen_id();
    rule.last_triggered = None;
    let mut rules = store.lock().unwrap();
    rules.push(rule);
    let _ = save_rules(&rules);
    HttpResponse::Ok().json(ApiResponse::ok("Rule added"))
}

async fn delete_rule(path: web::Path<String>, store: web::Data<RuleStore>) -> HttpResponse {
    let id = path.into_inner();
    let mut rules = store.lock().unwrap();
    rules.retain(|r| r.id != id);
    let _ = save_rules(&rules);
    HttpResponse::Ok().json(ApiResponse::ok("Rule deleted"))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/alerts")
            .route("/rules", web::get().to(list_rules))
            .route("/rules", web::post().to(add_rule))
            .route("/rules/{id}", web::delete().to(delete_rule))
    );
}
