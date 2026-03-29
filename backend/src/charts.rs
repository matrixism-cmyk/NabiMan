use actix_web::{web, HttpResponse};
use crate::models::ApiResponse;
use serde::Serialize;
use std::sync::{Arc, Mutex};

#[derive(Serialize, Clone)]
pub struct DataPoint {
    pub timestamp: i64,
    pub cpu: f32,
    pub memory_pct: f32,
    pub rx_bytes_sec: u64,
    pub tx_bytes_sec: u64,
    pub disk_read_bytes_sec: u64,
    pub disk_write_bytes_sec: u64,
}

pub type HistoryStore = Arc<Mutex<Vec<DataPoint>>>;

pub fn new_history_store() -> HistoryStore {
    Arc::new(Mutex::new(Vec::new()))
}

pub fn start_collector(store: HistoryStore) {
    std::thread::spawn(move || {
        let mut prev_rx: u64 = 0;
        let mut prev_tx: u64 = 0;
        let mut prev_disk_read: u64 = 0;
        let mut prev_disk_write: u64 = 0;
        loop {
            let cpu = read_cpu_usage();
            let mem = read_memory_pct();
            let (rx, tx) = read_net_bytes();
            let rx_sec = if prev_rx > 0 { rx.saturating_sub(prev_rx) / 10 } else { 0 };
            let tx_sec = if prev_tx > 0 { tx.saturating_sub(prev_tx) / 10 } else { 0 };
            prev_rx = rx;
            prev_tx = tx;

            let (dr, dw) = read_disk_bytes();
            let dr_sec = if prev_disk_read > 0 { dr.saturating_sub(prev_disk_read) / 10 } else { 0 };
            let dw_sec = if prev_disk_write > 0 { dw.saturating_sub(prev_disk_write) / 10 } else { 0 };
            prev_disk_read = dr;
            prev_disk_write = dw;

            let point = DataPoint {
                timestamp: chrono::Utc::now().timestamp(),
                cpu, memory_pct: mem, rx_bytes_sec: rx_sec, tx_bytes_sec: tx_sec,
                disk_read_bytes_sec: dr_sec, disk_write_bytes_sec: dw_sec,
            };

            let mut data = store.lock().unwrap();
            data.push(point);
            // Keep last 8640 points (~24h at 10s interval)
            let len = data.len();
            if len > 8640 { data.drain(0..len - 8640); }
            drop(data);
            std::thread::sleep(std::time::Duration::from_secs(10));
        }
    });
}

fn read_cpu_usage() -> f32 {
    let s = std::fs::read_to_string("/proc/loadavg").unwrap_or_default();
    let load1: f64 = s.split_whitespace().next()
        .and_then(|v| v.parse().ok()).unwrap_or(0.0);
    let cpus = num_cpus() as f64;
    ((load1 / cpus) * 100.0).min(100.0) as f32
}

fn read_memory_pct() -> f32 {
    let s = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
    let mut total: u64 = 0;
    let mut avail: u64 = 0;
    for line in s.lines() {
        if line.starts_with("MemTotal:") {
            total = parse_meminfo_val(line);
        } else if line.starts_with("MemAvailable:") {
            avail = parse_meminfo_val(line);
        }
    }
    if total > 0 { ((total - avail) as f32 / total as f32) * 100.0 } else { 0.0 }
}

fn parse_meminfo_val(line: &str) -> u64 {
    line.split_whitespace().nth(1).and_then(|v| v.parse().ok()).unwrap_or(0)
}

fn read_net_bytes() -> (u64, u64) {
    let s = std::fs::read_to_string("/proc/net/dev").unwrap_or_default();
    let mut rx = 0u64;
    let mut tx = 0u64;
    for line in s.lines().skip(2) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 10 {
            let iface = parts[0].trim_end_matches(':');
            if iface == "lo" { continue; }
            rx += parts[1].parse::<u64>().unwrap_or(0);
            tx += parts[9].parse::<u64>().unwrap_or(0);
        }
    }
    (rx, tx)
}

fn read_disk_bytes() -> (u64, u64) {
    let s = std::fs::read_to_string("/proc/diskstats").unwrap_or_default();
    let mut read_bytes = 0u64;
    let mut write_bytes = 0u64;
    for line in s.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 14 {
            let name = parts[2];
            // Only real disks (sd*, nvme*, vd*), skip partitions (sda1, nvme0n1p1)
            let is_whole_disk = (name.starts_with("sd") && name.len() == 3)
                || (name.starts_with("nvme") && !name.contains('p'))
                || (name.starts_with("vd") && name.len() == 3);
            if is_whole_disk {
                // field 5 = sectors read, field 9 = sectors written (512 bytes each)
                read_bytes += parts[5].parse::<u64>().unwrap_or(0) * 512;
                write_bytes += parts[9].parse::<u64>().unwrap_or(0) * 512;
            }
        }
    }
    (read_bytes, write_bytes)
}

fn num_cpus() -> usize {
    std::fs::read_to_string("/proc/cpuinfo").unwrap_or_default()
        .lines().filter(|l| l.starts_with("processor")).count().max(1)
}

async fn get_history(store: web::Data<HistoryStore>, query: web::Query<HistoryQuery>) -> HttpResponse {
    let data = store.lock().unwrap();
    let now = chrono::Utc::now().timestamp();
    let since = now - query.hours.unwrap_or(1) as i64 * 3600;
    let filtered: Vec<DataPoint> = data.iter()
        .filter(|p| p.timestamp >= since)
        .cloned().collect();
    HttpResponse::Ok().json(ApiResponse::ok(filtered))
}

#[derive(serde::Deserialize)]
struct HistoryQuery {
    hours: Option<u32>,
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.route("/api/server/history", web::get().to(get_history));
}
