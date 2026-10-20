//! Talking with Claude by voice: what is said is transcribed (the terminal's
//! dictation), asked of Claude Code in headless mode (`claude -p`), and the
//! answer read aloud by the speech service.
//!
//! Claude runs on this machine as the account it is signed in under, with
//! no tools by default: a spoken sentence should never change the server.
//! Only a super administrator may change how it runs.

mod ask;
mod chats;
mod launch;

use actix_web::{web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};

use super::dictation;
use crate::models::ApiResponse;

/// The voices the speech service has (id, name, what it sounds like).
pub(crate) const SPEAKERS: [(&str, &str, &str); 9] = [
    ("sohee", "Sohee", "따뜻하고 감정이 풍부한 한국어 여성"),
    ("vivian", "Vivian", "밝고 또렷한 젊은 여성 (중국어)"),
    ("serena", "Serena", "부드럽고 차분한 젊은 여성 (중국어)"),
    ("ono_anna", "Ono Anna", "경쾌한 젊은 여성 (일본어)"),
    ("aiden", "Aiden", "밝은 미국 남성 (영어)"),
    ("ryan", "Ryan", "리듬감 있는 남성 (영어)"),
    ("uncle_fu", "Uncle Fu", "낮고 묵직한 중년 남성 (중국어)"),
    ("dylan", "Dylan", "베이징 젊은 남성 (중국어)"),
    ("eric", "Eric", "청두 남성, 살짝 허스키 (중국어)"),
];
const LANGUAGES: [&str; 5] = ["korean", "english", "japanese", "chinese", "auto"];
/// What Claude may use: nothing, or tools that only read (files and the web).
const TOOL_MODES: [&str; 2] = ["none", "read"];

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct Settings {
    #[serde(default = "default_speaker")]
    pub speaker: String,
    #[serde(default = "default_language")]
    pub language: String,
    /// How to speak ("차분하게"), up to 100 characters; the service's `instruct`.
    #[serde(default)]
    pub instruct: String,
    /// Playback speed, 0.5 to 2.
    #[serde(default = "default_speed")]
    pub speed: f32,
    /// After an answer is read, listen for the next question on its own.
    #[serde(default = "default_true")]
    pub continuous: bool,
    #[serde(default = "default_tools")]
    pub tools: String,
    /// A model name for `--model`; empty for Claude Code's default.
    #[serde(default)]
    pub model: String,
    /// The account Claude runs as; empty for the owner of the `claude` program.
    #[serde(default)]
    pub run_as: String,
    /// Where `claude` is; empty to look in the usual places.
    #[serde(default)]
    pub claude_path: String,
    /// Added to the instructions Claude gets for spoken answers.
    #[serde(default)]
    pub extra_prompt: String,
}

fn default_speaker() -> String { "sohee".into() }
fn default_language() -> String { "korean".into() }
fn default_speed() -> f32 { 1.0 }
fn default_true() -> bool { true }
fn default_tools() -> String { "none".into() }

impl Default for Settings {
    fn default() -> Self {
        serde_json::from_str("{}").expect("defaults")
    }
}

pub(crate) fn data_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(std::env::var("NABIMAN_DATA_DIR").unwrap_or_else(|_| "/var/lib/nabiman".into()))
}

pub(crate) fn load() -> Settings {
    std::fs::read(data_dir().join("claude-voice.json")).ok()
        .and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn save(s: &Settings) -> Result<(), String> {
    let json = serde_json::to_vec_pretty(s).map_err(|e| e.to_string())?;
    crate::private_storage::write(&data_dir().join("claude-voice.json"), &json)
        .map_err(|e| format!("Cannot save the settings: {e}"))
}

/// Everything wrong with `s`, in one message, or nothing.
pub(crate) fn check(s: &Settings) -> Result<(), String> {
    if !SPEAKERS.iter().any(|(id, ..)| *id == s.speaker) { return Err("Unknown voice".into()); }
    if !LANGUAGES.contains(&s.language.as_str()) { return Err("Unknown language".into()); }
    if s.instruct.chars().count() > 100 { return Err("The speaking style is limited to 100 characters".into()); }
    if !(0.5..=2.0).contains(&s.speed) { return Err("The speed should be between 0.5 and 2".into()); }
    if !TOOL_MODES.contains(&s.tools.as_str()) { return Err("Unknown tool setting".into()); }
    if !s.model.chars().all(|c| c.is_ascii_alphanumeric() || "._-[]".contains(c)) || s.model.len() > 80 {
        return Err("A model name has letters, digits, dots and dashes".into());
    }
    if !s.run_as.is_empty() && !launch::valid_user(&s.run_as) { return Err("Not a valid account name".into()); }
    if !s.claude_path.is_empty() && !s.claude_path.starts_with('/') { return Err("The path to claude should be absolute".into()); }
    if s.extra_prompt.chars().count() > 2000 { return Err("The added instructions are limited to 2000 characters".into()); }
    Ok(())
}

fn admin_only(req: &HttpRequest) -> Result<(), HttpResponse> {
    dictation::super_admin(req)
}

async fn get_settings() -> HttpResponse {
    let s = load();
    let found = launch::Launch::resolve(&s);
    let status = serde_json::json!({
        "claude": found.as_ref().map(|l| l.program.clone()).ok(),
        "run_as": found.as_ref().map(|l| l.user.clone()).ok(),
        "problem": found.err(),
        "voice_ready": dictation::ready(),
        "speakers": SPEAKERS.iter().map(|(id, name, desc)| serde_json::json!({ "id": id, "name": name, "desc": desc })).collect::<Vec<_>>(),
    });
    HttpResponse::Ok().insert_header(("Cache-Control", "no-store"))
        .json(ApiResponse::ok(serde_json::json!({ "settings": s, "status": status })))
}

async fn put_settings(req: HttpRequest, body: web::Json<Settings>) -> HttpResponse {
    if let Err(r) = admin_only(&req) {
        return r;
    }
    let mut s = body.into_inner();
    s.instruct = s.instruct.trim().to_string();
    s.model = s.model.trim().to_string();
    s.run_as = s.run_as.trim().to_string();
    s.claude_path = s.claude_path.trim().to_string();
    if let Err(e) = check(&s) {
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error(&e));
    }
    match save(&s) {
        Ok(()) => get_settings().await,
        Err(e) => HttpResponse::InternalServerError().json(ApiResponse::<()>::error(&e)),
    }
}

/// Whether Claude starts here: its version, run the way a question would be.
async fn test(req: HttpRequest) -> HttpResponse {
    if let Err(r) = admin_only(&req) {
        return r;
    }
    match launch::Launch::resolve(&load()) {
        Ok(l) => match l.version().await {
            Ok(v) => HttpResponse::Ok().json(ApiResponse::ok(serde_json::json!({ "version": v, "run_as": l.user }))),
            Err(e) => HttpResponse::Ok().json(ApiResponse::<()>::error(&e)),
        },
        Err(e) => HttpResponse::Ok().json(ApiResponse::<()>::error(&e)),
    }
}

#[derive(Deserialize)]
struct Speak {
    text: String,
    /// A voice to try instead of the saved one (the settings' preview).
    speaker: Option<String>,
    instruct: Option<String>,
}

/// A sentence of an answer, read aloud as WAV.
async fn speak(body: web::Json<Speak>) -> HttpResponse {
    let s = load();
    let text = body.text.trim();
    if text.is_empty() || text.chars().count() > 2000 {
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error("Nothing to read, or too much at once"));
    }
    let speaker = body.speaker.clone().filter(|v| SPEAKERS.iter().any(|(id, ..)| id == v)).unwrap_or(s.speaker);
    let instruct = body.instruct.clone().map(|v| v.chars().take(100).collect()).unwrap_or(s.instruct);
    match dictation::speak(text, &speaker, &s.language, &instruct).await {
        Ok(wav) => HttpResponse::Ok().content_type("audio/wav").insert_header(("Cache-Control", "no-store")).body(wav),
        Err(e) => HttpResponse::BadGateway().json(ApiResponse::<()>::error(&e)),
    }
}

pub(crate) fn config(cfg: &mut web::ServiceConfig) {
    // Under /api/terminal: only administrators reach it (see rbac).
    cfg.route("/api/terminal/claude/settings", web::get().to(get_settings))
        .route("/api/terminal/claude/settings", web::put().to(put_settings))
        .route("/api/terminal/claude/test", web::post().to(test))
        .route("/api/terminal/claude/speak", web::post().to(speak))
        .route("/api/terminal/claude/chats", web::get().to(chats::list))
        .route("/api/terminal/claude/chats", web::post().to(chats::create))
        .route("/api/terminal/claude/chats/{id}", web::get().to(chats::get))
        .route("/api/terminal/claude/chats/{id}", web::delete().to(chats::delete))
        .route("/api/terminal/claude/chats/{id}/ask", web::post().to(ask::ask))
        .route("/api/terminal/claude/chats/{id}/stop", web::post().to(ask::stop));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_speak_korean_with_no_tools() {
        let s = Settings::default();
        assert_eq!((s.speaker.as_str(), s.language.as_str(), s.tools.as_str(), s.speed, s.continuous), ("sohee", "korean", "none", 1.0, true));
        assert!(check(&s).is_ok());
    }

    #[test]
    fn settings_are_checked() {
        let bad = |f: fn(&mut Settings)| { let mut s = Settings::default(); f(&mut s); check(&s).is_err() };
        assert!(bad(|s| s.speaker = "nobody".into()));
        assert!(bad(|s| s.speed = 3.0));
        assert!(bad(|s| s.tools = "all".into()));
        assert!(bad(|s| s.model = "opus; rm -rf /".into()));
        assert!(bad(|s| s.run_as = "root user".into()));
        assert!(bad(|s| s.claude_path = "claude".into()));
        let ok = Settings { model: "claude-opus-5-5".into(), run_as: "nabi".into(), ..Settings::default() };
        assert!(check(&ok).is_ok());
    }
}
