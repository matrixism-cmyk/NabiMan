use actix_web::{web, HttpResponse};
use crate::models::{ApiResponse, DiskStatus, DiskPartition, DiskIo};

async fn get_disk_status() -> HttpResponse {
    let partitions = get_partitions();
    let io = get_disk_io();
    HttpResponse::Ok().json(ApiResponse::ok(DiskStatus { partitions, io }))
}

fn get_partitions() -> Vec<DiskPartition> {
    let output = match std::process::Command::new("df")
        .args(["-BK", "--output=source,target,fstype,size,used,avail,pcent"])
        .output()
    {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
        _ => return Vec::new(),
    };

    output
        .lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .filter_map(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 7 {
                let fs = parts[0];
                // Skip pseudo filesystems
                if fs.starts_with("tmpfs") || fs.starts_with("devtmpfs")
                    || fs == "none" || fs == "udev" || fs == "overlay"
                {
                    return None;
                }
                let parse_kb = |s: &str| -> u64 {
                    s.trim_end_matches('K').parse::<u64>().unwrap_or(0) * 1024
                };
                let use_pct = parts[6].trim_end_matches('%').parse::<f32>().unwrap_or(0.0);
                Some(DiskPartition {
                    filesystem: fs.to_string(),
                    mount_point: parts[1].to_string(),
                    fs_type: parts[2].to_string(),
                    total: parse_kb(parts[3]),
                    used: parse_kb(parts[4]),
                    available: parse_kb(parts[5]),
                    use_percent: use_pct,
                })
            } else {
                None
            }
        })
        .collect()
}

fn get_disk_io() -> Vec<DiskIo> {
    // Try iostat first
    let output = std::process::Command::new("iostat")
        .args(["-dxy", "1", "1"])
        .output();

    if let Ok(o) = output {
        if o.status.success() {
            let stdout = String::from_utf8_lossy(&o.stdout);
            return parse_iostat(&stdout);
        }
    }

    // Fallback: read /proc/diskstats
    if let Ok(content) = std::fs::read_to_string("/proc/diskstats") {
        return parse_proc_diskstats(&content);
    }

    Vec::new()
}

fn parse_iostat(output: &str) -> Vec<DiskIo> {
    let mut devices = Vec::new();
    let mut in_device_section = false;

    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("Device") {
            in_device_section = true;
            continue;
        }
        if !in_device_section || trimmed.is_empty() {
            continue;
        }
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() >= 6 {
            devices.push(DiskIo {
                device: parts[0].to_string(),
                reads_per_sec: parts[1].parse().unwrap_or(0.0),
                writes_per_sec: parts[2].parse().unwrap_or(0.0),
                read_bytes_per_sec: (parts[3].parse::<f64>().unwrap_or(0.0) * 1024.0) as u64,
                write_bytes_per_sec: (parts[4].parse::<f64>().unwrap_or(0.0) * 1024.0) as u64,
            });
        }
    }
    devices
}

fn parse_proc_diskstats(content: &str) -> Vec<DiskIo> {
    content
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 14 {
                let name = parts[2];
                // Only show real disk devices (sd*, nvme*, vd*)
                if name.starts_with("sd") || name.starts_with("nvme") || name.starts_with("vd") {
                    // fields 3=reads, 7=writes, 5=sectors_read, 9=sectors_written
                    let reads: f64 = parts[3].parse().unwrap_or(0.0);
                    let writes: f64 = parts[7].parse().unwrap_or(0.0);
                    let read_sectors: u64 = parts[5].parse().unwrap_or(0);
                    let write_sectors: u64 = parts[9].parse().unwrap_or(0);
                    Some(DiskIo {
                        device: name.to_string(),
                        reads_per_sec: reads,
                        writes_per_sec: writes,
                        read_bytes_per_sec: read_sectors * 512,
                        write_bytes_per_sec: write_sectors * 512,
                    })
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect()
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/disks")
            .route("/status", web::get().to(get_disk_status)),
    );
}
