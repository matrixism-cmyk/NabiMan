use actix_web::{get, web, HttpResponse};
use sysinfo::Networks;
use std::fs;
use crate::models::{ApiResponse, TrafficSnapshot};

#[get("/api/traffic/current")]
async fn get_current_traffic() -> HttpResponse {
    let mut networks = Networks::new_with_refreshed_list();
    // Wait briefly and refresh for per-second rates
    std::thread::sleep(std::time::Duration::from_secs(1));
    networks.refresh();

    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let active_connections = count_active_connections();

    let mut snapshots = Vec::new();
    for (name, data) in &networks {
        snapshots.push(TrafficSnapshot {
            timestamp: timestamp.clone(),
            rx_bytes_per_sec: data.received(),
            tx_bytes_per_sec: data.transmitted(),
            active_connections,
            interface: name.clone(),
        });
    }

    HttpResponse::Ok().json(ApiResponse::ok(snapshots))
}

#[get("/api/traffic/summary")]
async fn get_traffic_summary() -> HttpResponse {
    let networks = Networks::new_with_refreshed_list();
    let mut total_rx: u64 = 0;
    let mut total_tx: u64 = 0;

    for (_name, data) in &networks {
        total_rx += data.total_received();
        total_tx += data.total_transmitted();
    }

    let summary = serde_json::json!({
        "total_rx_bytes": total_rx,
        "total_tx_bytes": total_tx,
        "total_rx_mb": total_rx / 1_048_576,
        "total_tx_mb": total_tx / 1_048_576,
        "active_connections": count_active_connections(),
        "tcp_connections": count_tcp_connections(),
    });

    HttpResponse::Ok().json(ApiResponse::ok(summary))
}

fn count_active_connections() -> u32 {
    let mut count = 0u32;
    if let Ok(content) = fs::read_to_string("/proc/net/tcp") {
        for line in content.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() > 3 {
                // State 01 = ESTABLISHED
                if parts[3] == "01" {
                    count += 1;
                }
            }
        }
    }
    count
}

fn count_tcp_connections() -> serde_json::Value {
    let mut established = 0u32;
    let mut listen = 0u32;
    let mut time_wait = 0u32;
    let mut other = 0u32;

    if let Ok(content) = fs::read_to_string("/proc/net/tcp") {
        for line in content.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() > 3 {
                match parts[3] {
                    "01" => established += 1,
                    "0A" => listen += 1,
                    "06" => time_wait += 1,
                    _ => other += 1,
                }
            }
        }
    }

    serde_json::json!({
        "established": established,
        "listen": listen,
        "time_wait": time_wait,
        "other": other,
    })
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(get_current_traffic)
        .service(get_traffic_summary);
}
