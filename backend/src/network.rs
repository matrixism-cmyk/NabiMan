use actix_web::{get, web, HttpResponse};
use sysinfo::Networks;
use std::fs;
use crate::models::{ApiResponse, NetworkInterface, NetworkStatus};

#[get("/api/network/status")]
async fn get_network_status() -> HttpResponse {
    let networks = Networks::new_with_refreshed_list();
    let mut interfaces = Vec::new();

    for (name, data) in &networks {
        let ip_address = get_interface_ip(name);
        let mac_address = get_interface_mac(name);

        interfaces.push(NetworkInterface {
            name: name.clone(),
            ip_address,
            mac_address,
            rx_bytes: data.total_received(),
            tx_bytes: data.total_transmitted(),
            is_up: true,
        });
    }

    let open_connections = count_connections();
    let dns_servers = read_dns_servers();

    let status = NetworkStatus {
        interfaces,
        open_connections,
        dns_servers,
    };

    HttpResponse::Ok().json(ApiResponse::ok(status))
}

fn get_interface_ip(name: &str) -> String {
    if let Ok(output) = std::process::Command::new("ip")
        .args(["addr", "show", name])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("inet ") {
                if let Some(addr) = trimmed.split_whitespace().nth(1) {
                    return addr.split('/').next().unwrap_or("").to_string();
                }
            }
        }
    }
    "N/A".to_string()
}

fn get_interface_mac(name: &str) -> String {
    let path = format!("/sys/class/net/{}/address", name);
    fs::read_to_string(&path)
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "N/A".to_string())
}

fn count_connections() -> u32 {
    if let Ok(content) = fs::read_to_string("/proc/net/tcp") {
        // Subtract 1 for header line
        return content.lines().count().saturating_sub(1) as u32;
    }
    0
}

fn read_dns_servers() -> Vec<String> {
    if let Ok(content) = fs::read_to_string("/etc/resolv.conf") {
        return content
            .lines()
            .filter(|l| l.starts_with("nameserver"))
            .filter_map(|l| l.split_whitespace().nth(1))
            .map(String::from)
            .collect();
    }
    Vec::new()
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(get_network_status);
}
