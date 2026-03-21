use actix_web::{get, web, HttpResponse};
use sysinfo::System;
use crate::models::{ApiResponse, ServerStatus};

#[get("/api/server/status")]
async fn get_status() -> HttpResponse {
    let mut sys = System::new_all();
    sys.refresh_all();

    let hostname = System::host_name().unwrap_or_else(|| "unknown".into());
    let os = format!(
        "{} {}",
        System::name().unwrap_or_else(|| "unknown".into()),
        System::os_version().unwrap_or_else(|| "".into())
    );
    let uptime = System::uptime();
    let cpu_usage = sys.global_cpu_info().cpu_usage();
    let memory_total = sys.total_memory();
    let memory_used = sys.used_memory();

    let (disk_total, disk_used) = {
        let disks = sysinfo::Disks::new_with_refreshed_list();
        let mut total = 0u64;
        let mut used = 0u64;
        for disk in disks.list() {
            total += disk.total_space();
            used += disk.total_space() - disk.available_space();
        }
        (total, used)
    };

    let load_avg = System::load_average();

    let status = ServerStatus {
        hostname,
        os,
        uptime,
        cpu_usage,
        memory_total,
        memory_used,
        disk_total,
        disk_used,
        load_average: [load_avg.one, load_avg.five, load_avg.fifteen],
    };

    HttpResponse::Ok().json(ApiResponse::ok(status))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(get_status);
}
