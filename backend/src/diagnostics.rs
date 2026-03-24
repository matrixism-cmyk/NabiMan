use actix_web::{web, HttpResponse};
use crate::models::ApiResponse;
use serde::{Deserialize, Serialize};
use std::net::TcpStream;
use std::time::Duration;

#[derive(Deserialize)]
pub struct HostRequest {
    pub host: String,
}

#[derive(Deserialize)]
pub struct PortCheckRequest {
    pub host: String,
    pub port: u16,
}

#[derive(Serialize)]
pub struct PortCheckResult {
    pub host: String,
    pub port: u16,
    pub open: bool,
}

fn validate_host(host: &str) -> bool {
    !host.is_empty() && host.len() <= 253
        && host.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '-' || c == ':')
}

async fn ping(body: web::Json<HostRequest>) -> HttpResponse {
    if !validate_host(&body.host) {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Invalid host"));
    }
    match std::process::Command::new("ping")
        .args(["-c", "4", "-W", "3", &body.host]).output() {
        Ok(o) => {
            let out = String::from_utf8_lossy(&o.stdout).to_string();
            let err = String::from_utf8_lossy(&o.stderr).to_string();
            HttpResponse::Ok().json(ApiResponse::ok(if out.is_empty() { err } else { out }))
        }
        Err(e) => HttpResponse::Ok().json(ApiResponse::<String>::error(&e.to_string())),
    }
}

async fn traceroute(body: web::Json<HostRequest>) -> HttpResponse {
    if !validate_host(&body.host) {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Invalid host"));
    }
    let cmd = if std::path::Path::new("/usr/bin/traceroute").exists()
        || std::path::Path::new("/usr/sbin/traceroute").exists() {
        "traceroute"
    } else { "tracepath" };

    match std::process::Command::new(cmd)
        .args(["-m", "20", &body.host]).output() {
        Ok(o) => {
            let out = String::from_utf8_lossy(&o.stdout).to_string();
            let err = String::from_utf8_lossy(&o.stderr).to_string();
            HttpResponse::Ok().json(ApiResponse::ok(if out.is_empty() { err } else { out }))
        }
        Err(e) => HttpResponse::Ok().json(ApiResponse::<String>::error(
            &format!("{} not found: {}", cmd, e)
        )),
    }
}

async fn nslookup(body: web::Json<HostRequest>) -> HttpResponse {
    if !validate_host(&body.host) {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Invalid host"));
    }
    let cmd = if std::path::Path::new("/usr/bin/dig").exists() { "dig" } else { "nslookup" };
    let args: Vec<&str> = if cmd == "dig" {
        vec![&body.host, "+noall", "+answer", "+authority"]
    } else {
        vec![&body.host]
    };

    match std::process::Command::new(cmd).args(&args).output() {
        Ok(o) => {
            let out = String::from_utf8_lossy(&o.stdout).to_string();
            HttpResponse::Ok().json(ApiResponse::ok(out))
        }
        Err(e) => HttpResponse::Ok().json(ApiResponse::<String>::error(&e.to_string())),
    }
}

async fn port_check(body: web::Json<PortCheckRequest>) -> HttpResponse {
    if !validate_host(&body.host) {
        return HttpResponse::Ok().json(ApiResponse::<PortCheckResult>::error("Invalid host"));
    }
    let addr = format!("{}:{}", body.host, body.port);
    let open = TcpStream::connect_timeout(
        &addr.parse().unwrap_or_else(|_| {
            use std::net::ToSocketAddrs;
            addr.to_socket_addrs()
                .ok().and_then(|mut a| a.next())
                .unwrap_or_else(|| "0.0.0.0:0".parse().unwrap())
        }),
        Duration::from_secs(3),
    ).is_ok();

    HttpResponse::Ok().json(ApiResponse::ok(PortCheckResult {
        host: body.host.clone(),
        port: body.port,
        open,
    }))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/diagnostics")
            .route("/ping", web::post().to(ping))
            .route("/traceroute", web::post().to(traceroute))
            .route("/nslookup", web::post().to(nslookup))
            .route("/port-check", web::post().to(port_check))
    );
}
