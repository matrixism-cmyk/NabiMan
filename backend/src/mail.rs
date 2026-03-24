use actix_web::{web, HttpResponse};
use crate::models::ApiResponse;
use serde::Serialize;

#[derive(Serialize, Clone)]
pub struct MailStatus {
    pub running: bool,
    pub server: String,
    pub queue_count: u32,
    pub queue_entries: Vec<MailQueueEntry>,
    pub stats: MailStats,
}

#[derive(Serialize, Clone)]
pub struct MailQueueEntry {
    pub id: String,
    pub size: String,
    pub sender: String,
    pub recipient: String,
    pub status: String,
}

#[derive(Serialize, Clone, Default)]
pub struct MailStats {
    pub sent_today: u32,
    pub received_today: u32,
    pub bounced_today: u32,
    pub deferred: u32,
}

fn detect_mail_server() -> Option<&'static str> {
    let checks = [("postfix", "master"), ("sendmail", "sendmail"), ("exim", "exim4")];
    for (name, proc) in &checks {
        if std::process::Command::new("pgrep").arg("-x").arg(proc)
            .output().map(|o| o.status.success()).unwrap_or(false) {
            return Some(name);
        }
    }
    None
}

fn postfix_queue() -> (u32, Vec<MailQueueEntry>) {
    let output = std::process::Command::new("mailq").output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();

    if output.contains("Mail queue is empty") {
        return (0, Vec::new());
    }

    let mut entries = Vec::new();
    let mut current_id = String::new();
    let mut current_size = String::new();
    let mut current_sender = String::new();

    for line in output.lines() {
        if line.len() > 10 && line.chars().next().map(|c| c.is_alphanumeric()).unwrap_or(false) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                current_id = parts[0].trim_end_matches('*').trim_end_matches('!').to_string();
                current_size = parts[1].to_string();
                current_sender = parts.last().unwrap_or(&"").to_string();
            }
        } else if line.starts_with("    ") && !current_id.is_empty() {
            let recipient = line.trim().to_string();
            if !recipient.is_empty() && !recipient.starts_with('(') {
                entries.push(MailQueueEntry {
                    id: current_id.clone(), size: current_size.clone(),
                    sender: current_sender.clone(), recipient, status: "queued".into(),
                });
            }
        }
    }
    let count = entries.len() as u32;
    (count, entries)
}

fn mail_stats_from_log() -> MailStats {
    let output = std::process::Command::new("journalctl")
        .args(["-u", "postfix@-.service", "--since", "today", "--no-pager", "-q"])
        .output()
        .or_else(|_| std::process::Command::new("journalctl")
            .args(["-u", "postfix", "--since", "today", "--no-pager", "-q"]).output())
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();

    let mut stats = MailStats::default();
    for line in output.lines() {
        if line.contains("status=sent") { stats.sent_today += 1; }
        else if line.contains("status=bounced") { stats.bounced_today += 1; }
        else if line.contains("status=deferred") { stats.deferred += 1; }
        if line.contains("from=<") { stats.received_today += 1; }
    }
    stats
}

async fn status() -> HttpResponse {
    match detect_mail_server() {
        Some(server) => {
            let (queue_count, queue_entries) = postfix_queue();
            let stats = mail_stats_from_log();
            HttpResponse::Ok().json(ApiResponse::ok(MailStatus {
                running: true, server: server.to_string(),
                queue_count, queue_entries, stats,
            }))
        }
        None => HttpResponse::Ok().json(
            ApiResponse::<MailStatus>::error("No mail server running")
        ),
    }
}

async fn available() -> HttpResponse {
    HttpResponse::Ok().json(ApiResponse::ok(detect_mail_server().is_some()))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/mail")
            .route("/available", web::get().to(available))
            .route("/status", web::get().to(status))
    );
}
