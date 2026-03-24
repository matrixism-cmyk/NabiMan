use actix_web::{web, HttpResponse};
use crate::models::ApiResponse;
use serde::Serialize;

#[derive(Serialize, Clone)]
pub struct SwapInfo {
    pub filename: String,
    pub swap_type: String,
    pub size: u64,
    pub used: u64,
    pub priority: i32,
}

#[derive(Serialize)]
pub struct SwapStatus {
    pub total: u64,
    pub used: u64,
    pub free: u64,
    pub swappiness: u32,
    pub entries: Vec<SwapInfo>,
}

async fn status() -> HttpResponse {
    let proc_swaps = std::fs::read_to_string("/proc/swaps").unwrap_or_default();
    let mut entries = Vec::new();
    for line in proc_swaps.lines().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 5 {
            entries.push(SwapInfo {
                filename: parts[0].to_string(),
                swap_type: parts[1].to_string(),
                size: parts[2].parse().unwrap_or(0),
                used: parts[3].parse().unwrap_or(0),
                priority: parts[4].parse().unwrap_or(0),
            });
        }
    }

    let meminfo = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
    let mut total: u64 = 0;
    let mut free: u64 = 0;
    for line in meminfo.lines() {
        if line.starts_with("SwapTotal:") {
            total = line.split_whitespace().nth(1).and_then(|v| v.parse().ok()).unwrap_or(0);
        } else if line.starts_with("SwapFree:") {
            free = line.split_whitespace().nth(1).and_then(|v| v.parse().ok()).unwrap_or(0);
        }
    }

    let swappiness: u32 = std::fs::read_to_string("/proc/sys/vm/swappiness")
        .unwrap_or_default().trim().parse().unwrap_or(60);

    HttpResponse::Ok().json(ApiResponse::ok(SwapStatus {
        total, used: total.saturating_sub(free), free, swappiness, entries,
    }))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.route("/api/swap/status", web::get().to(status));
}
