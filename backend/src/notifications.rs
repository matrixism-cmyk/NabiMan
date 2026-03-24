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

pub fn send_notification(channels: &[NotificationChannel], subject: &str, body: &str) {
    for ch in channels.iter().filter(|c| c.enabled) {
        match ch.channel_type.as_str() {
            "email" => send_email(&ch.target, subject, body),
            "webhook" | "slack" => send_webhook(&ch.target, subject, body),
            _ => {}
        }
    }
}

fn send_email(to: &str, subject: &str, body: &str) {
    let msg = format!("Subject: {}\nTo: {}\n\n{}", subject, to, body);
    let _ = std::process::Command::new("sh")
        .args(["-c", &format!("echo '{}' | sendmail '{}'", msg.replace('\'', ""), to)])
        .output();
}

fn send_webhook(url: &str, subject: &str, body: &str) {
    let payload = serde_json::json!({ "text": format!("*{}*\n{}", subject, body) });
    let _ = std::process::Command::new("curl")
        .args(["-s", "-X", "POST", "-H", "Content-Type: application/json",
               "-d", &payload.to_string(), url])
        .output();
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

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/notifications")
            .route("/channels", web::get().to(list_channels))
            .route("/channels", web::post().to(add_channel))
            .route("/channels/{id}", web::delete().to(delete_channel))
            .route("/test", web::post().to(test_channel))
    );
}
