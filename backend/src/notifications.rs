use actix_web::{web, HttpResponse};
use crate::models::ApiResponse;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Serialize, Deserialize, Clone)]
pub struct NotificationChannel {
    pub id: String,
    pub name: String,
    pub channel_type: String, // "email", "webhook", "slack"
    pub target: String,       // email address, webhook URL, slack webhook URL
    pub enabled: bool,
}

pub type ChannelStore = Arc<Mutex<Vec<NotificationChannel>>>;

pub fn new_channel_store() -> ChannelStore {
    let path = channels_file();
    let channels = std::fs::read_to_string(&path).ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    Arc::new(Mutex::new(channels))
}

fn channels_file() -> std::path::PathBuf {
    let dir = std::env::var("NABIMAN_DATA_DIR").unwrap_or_else(|_| "/var/lib/nabiman".into());
    std::path::PathBuf::from(dir).join("notifications.json")
}

fn save_channels(channels: &[NotificationChannel]) -> Result<(), String> {
    let json = serde_json::to_string_pretty(channels).map_err(|e| e.to_string())?;
    std::fs::write(channels_file(), json).map_err(|e| e.to_string())
}

fn gen_id() -> String {
    use rand::Rng;
    format!("{:016x}", rand::thread_rng().gen::<u64>())
}

// --- Notification History ---

#[derive(Serialize, Deserialize, Clone)]
pub struct NotificationEvent {
    pub timestamp: String,
    pub subject: String,
    pub body: String,
    pub channel_name: String,
    pub channel_type: String,
    pub success: bool,
}

fn history_file() -> std::path::PathBuf {
    let dir = std::env::var("NABIMAN_DATA_DIR").unwrap_or_else(|_| "/var/lib/nabiman".into());
    std::path::PathBuf::from(dir).join("notification_history.json")
}

fn append_history(event: &NotificationEvent) {
    let mut history: Vec<NotificationEvent> = std::fs::read_to_string(history_file()).ok()
        .and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    history.push(event.clone());
    // Keep last 500 events
    if history.len() > 500 { history.drain(0..history.len() - 500); }
    let _ = std::fs::write(history_file(), serde_json::to_string(&history).unwrap_or_default());
}

fn load_history() -> Vec<NotificationEvent> {
    std::fs::read_to_string(history_file()).ok()
        .and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

pub fn send_notification(channels: &[NotificationChannel], subject: &str, body: &str) {
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    for ch in channels.iter().filter(|c| c.enabled) {
        let success = match ch.channel_type.as_str() {
            "email" => send_email(&ch.target, subject, body),
            "webhook" | "slack" => send_webhook(&ch.target, subject, body),
            _ => false,
        };
        append_history(&NotificationEvent {
            timestamp: now.clone(), subject: subject.to_string(), body: body.to_string(),
            channel_name: ch.name.clone(), channel_type: ch.channel_type.clone(), success,
        });
    }
}

fn send_email(to: &str, subject: &str, body: &str) -> bool {
    let msg = format!("Subject: {}\nTo: {}\n\n{}", subject, to, body);
    std::process::Command::new("sh")
        .args(["-c", &format!("echo '{}' | sendmail '{}'", msg.replace('\'', ""), to)])
        .output().map(|o| o.status.success()).unwrap_or(false)
}

fn send_webhook(url: &str, subject: &str, body: &str) -> bool {
    let payload = serde_json::json!({ "text": format!("*{}*\n{}", subject, body) });
    std::process::Command::new("curl")
        .args(["-s", "-X", "POST", "-H", "Content-Type: application/json",
               "-d", &payload.to_string(), "--max-time", "10", url])
        .output().map(|o| o.status.success()).unwrap_or(false)
}

async fn list_channels(store: web::Data<ChannelStore>) -> HttpResponse {
    HttpResponse::Ok().json(ApiResponse::ok(store.lock().unwrap().clone()))
}

#[derive(Deserialize)]
pub struct AddChannelRequest {
    pub name: String, pub channel_type: String, pub target: String,
}

async fn add_channel(body: web::Json<AddChannelRequest>, store: web::Data<ChannelStore>) -> HttpResponse {
    let ch = NotificationChannel {
        id: gen_id(), name: body.name.clone(), channel_type: body.channel_type.clone(),
        target: body.target.clone(), enabled: true,
    };
    let mut channels = store.lock().unwrap();
    channels.push(ch);
    let _ = save_channels(&channels);
    HttpResponse::Ok().json(ApiResponse::ok("Channel added"))
}

async fn delete_channel(path: web::Path<String>, store: web::Data<ChannelStore>) -> HttpResponse {
    let id = path.into_inner();
    let mut channels = store.lock().unwrap();
    channels.retain(|c| c.id != id);
    let _ = save_channels(&channels);
    HttpResponse::Ok().json(ApiResponse::ok("Channel deleted"))
}

#[derive(Deserialize)]
pub struct TestRequest { pub message: Option<String> }

async fn test_channel(store: web::Data<ChannelStore>, body: web::Json<TestRequest>) -> HttpResponse {
    let channels = store.lock().unwrap().clone();
    let msg = body.message.as_deref().unwrap_or("NabiMan test notification");
    send_notification(&channels, "NabiMan Alert Test", msg);
    HttpResponse::Ok().json(ApiResponse::ok("Test sent"))
}

// --- Custom Templates ---

#[derive(Serialize, Deserialize, Clone)]
pub struct NotificationTemplate {
    pub id: String,
    pub name: String,
    pub subject_template: String,
    pub body_template: String,
}

fn templates_file() -> std::path::PathBuf {
    let dir = std::env::var("NABIMAN_DATA_DIR").unwrap_or_else(|_| "/var/lib/nabiman".into());
    std::path::PathBuf::from(dir).join("notification_templates.json")
}

fn load_templates() -> Vec<NotificationTemplate> {
    std::fs::read_to_string(templates_file()).ok()
        .and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

fn save_templates(t: &[NotificationTemplate]) {
    let _ = std::fs::write(templates_file(), serde_json::to_string_pretty(t).unwrap_or_default());
}

/// Render template with variable substitution: {{hostname}}, {{metric}}, {{value}}, {{threshold}}, {{timestamp}}
pub fn render_template(template: &str, vars: &std::collections::HashMap<String, String>) -> String {
    let mut result = template.to_string();
    for (k, v) in vars {
        result = result.replace(&format!("{{{{{}}}}}", k), v);
    }
    result
}

/// Send notification using a named template, falling back to raw subject/body
pub fn send_notification_with_template(
    channels: &[NotificationChannel], template_name: &str,
    vars: &std::collections::HashMap<String, String>,
    fallback_subject: &str, fallback_body: &str,
) {
    let templates = load_templates();
    let (subject, body) = if let Some(t) = templates.iter().find(|t| t.name == template_name) {
        (render_template(&t.subject_template, vars), render_template(&t.body_template, vars))
    } else {
        (fallback_subject.to_string(), fallback_body.to_string())
    };
    send_notification(channels, &subject, &body);
}

async fn list_templates() -> HttpResponse {
    HttpResponse::Ok().json(ApiResponse::ok(load_templates()))
}

async fn add_template(body: web::Json<NotificationTemplate>) -> HttpResponse {
    let mut t = body.into_inner();
    t.id = gen_id();
    let mut templates = load_templates();
    templates.push(t);
    save_templates(&templates);
    HttpResponse::Ok().json(ApiResponse::ok("Template created"))
}

async fn update_template(path: web::Path<String>, body: web::Json<NotificationTemplate>) -> HttpResponse {
    let id = path.into_inner();
    let mut templates = load_templates();
    if let Some(t) = templates.iter_mut().find(|t| t.id == id) {
        t.name = body.name.clone();
        t.subject_template = body.subject_template.clone();
        t.body_template = body.body_template.clone();
        save_templates(&templates);
        HttpResponse::Ok().json(ApiResponse::ok("Template updated"))
    } else {
        HttpResponse::Ok().json(ApiResponse::<()>::error("Template not found"))
    }
}

async fn delete_template(path: web::Path<String>) -> HttpResponse {
    let id = path.into_inner();
    let mut templates = load_templates();
    templates.retain(|t| t.id != id);
    save_templates(&templates);
    HttpResponse::Ok().json(ApiResponse::ok("Template deleted"))
}

async fn get_history() -> HttpResponse {
    let history = load_history();
    HttpResponse::Ok().json(ApiResponse::ok(history))
}

async fn clear_history() -> HttpResponse {
    let _ = std::fs::write(history_file(), "[]");
    HttpResponse::Ok().json(ApiResponse::ok("History cleared"))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/notifications")
            .route("/channels", web::get().to(list_channels))
            .route("/channels", web::post().to(add_channel))
            .route("/channels/{id}", web::delete().to(delete_channel))
            .route("/test", web::post().to(test_channel))
            .route("/history", web::get().to(get_history))
            .route("/history", web::delete().to(clear_history))
            .route("/templates", web::get().to(list_templates))
            .route("/templates", web::post().to(add_template))
            .route("/templates/{id}", web::put().to(update_template))
            .route("/templates/{id}", web::delete().to(delete_template))
    );
}
