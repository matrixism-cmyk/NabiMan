//! Optional session recording: for review and audit, each terminal session's
//! output can be saved as a timestamped stream and replayed in the browser.
//!
//! Off by default — an administrator turns it on, per the principle that
//! tighter security stays a choice. Only the pane's *output* is piped (tmux
//! `pipe-pane`), once per tmux session however many browsers attach, so a
//! password typed at a no-echo prompt is never written. Capturing keystrokes
//! too is a separate switch, also off by default.
//!
//! One file pair per session: `<sid>.cast` (a header line then
//! `[elapsed_ms, base64(bytes)]` per chunk, written by the `session-record`
//! subcommand) and `<sid>.json` (who/where/when, written here).

use std::io::{Read, Write};
use std::path::PathBuf;

use actix_web::{web, HttpRequest, HttpResponse};
use crate::models::ApiResponse;

use serde::{Deserialize, Serialize};

pub const SUBCOMMAND: &str = "session-record";

/// What an administrator may choose. Off unless turned on.
#[derive(Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub enabled: bool,
    /// Also record keystrokes (input). Off: only what the screen showed.
    #[serde(default)]
    pub keystrokes: bool,
    /// Recordings older than this are swept away; 0 keeps them.
    #[serde(default = "default_retention")]
    pub retention_days: u32,
    /// A single recording stops growing past this; keeps one session bounded.
    #[serde(default = "default_max_mb")]
    pub max_mb: u32,
}
fn default_retention() -> u32 {
    30
}
fn default_max_mb() -> u32 {
    50
}
impl Default for Settings {
    fn default() -> Self {
        Self { enabled: false, keystrokes: false, retention_days: default_retention(), max_mb: default_max_mb() }
    }
}

fn settings_file() -> PathBuf {
    nabiman_server::data_dir().join("recording.json")
}

pub fn settings() -> Settings {
    std::fs::read(settings_file())
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

pub fn save_settings(s: &Settings) -> Result<(), String> {
    if s.max_mb > 2048 || s.retention_days > 3650 {
        return Err("Recording limits out of range".into());
    }
    let json = serde_json::to_vec_pretty(s).map_err(|e| e.to_string())?;
    crate::private_storage::write(&settings_file(), &json).map_err(|e| format!("Cannot save recording settings: {e}"))
}

pub(crate) fn dir() -> PathBuf {
    nabiman_server::data_dir().join("session-recordings")
}

/// `<sid>` is one of our own session ids: letters, digits and `_` only.
fn safe_sid(sid: &str) -> bool {
    !sid.is_empty() && sid.len() <= 128 && sid.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

pub(crate) fn cast_path(sid: &str) -> Option<PathBuf> {
    safe_sid(sid).then(|| dir().join(format!("{sid}.cast")))
}
pub(crate) fn meta_path(sid: &str) -> Option<PathBuf> {
    safe_sid(sid).then(|| dir().join(format!("{sid}.json")))
}

#[derive(Serialize, Deserialize)]
pub(crate) struct Meta {
    pub sid: String,
    pub owner: String,
    pub owner_id: String,
    pub label: String,
    pub started_ms: u128,
    pub cols: u16,
    pub rows: u16,
}

fn now_ms() -> u128 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0)
}

/// Starts recording `name`'s output when recording is on. Best effort: a
/// failure here never stops the session from opening.
pub(crate) fn start(name: &str, sid: &str, owner: &str, owner_id: &str, label: &str, size: (u16, u16)) {
    let s = settings();
    if !s.enabled {
        return;
    }
    let (Some(cast), Some(meta_p)) = (cast_path(sid), meta_path(sid)) else { return };
    if std::fs::create_dir_all(dir()).is_err() {
        return;
    }
    let meta = Meta {
        sid: sid.into(), owner: owner.into(), owner_id: owner_id.into(), label: label.into(),
        started_ms: now_ms(), cols: size.0, rows: size.1,
    };
    if let Ok(json) = serde_json::to_vec(&meta) {
        let _ = crate::private_storage::write(&meta_p, &json);
    }
    // The recorder is this very binary, reading the pane's output on stdin.
    let exe = std::env::current_exe().map(|p| p.to_string_lossy().trim_end_matches(" (deleted)").to_string());
    let Ok(exe) = exe else { return };
    let max = s.max_mb.max(1);
    let command = format!(
        "{} {SUBCOMMAND} {} {} {} {}",
        quote(&exe), quote(&cast.to_string_lossy()), size.0, size.1, max
    );
    super::session::pipe_pane_record(name, &command);
}

fn quote(arg: &str) -> String {
    format!("'{}'", arg.replace('\'', "'\\''"))
}

/// Runs the recorder and exits when the binary was started as one. The pane's
/// output arrives on stdin; each chunk is written as `[elapsed_ms, base64]`.
pub fn run_if_asked() {
    if std::env::args().nth(1).as_deref() != Some(SUBCOMMAND) {
        return;
    }
    let mut a = std::env::args().skip(2);
    let path = a.next().unwrap_or_default();
    let cols: u16 = a.next().and_then(|v| v.parse().ok()).unwrap_or(80);
    let rows: u16 = a.next().and_then(|v| v.parse().ok()).unwrap_or(24);
    let max_bytes: u64 = a.next().and_then(|v| v.parse::<u64>().ok()).unwrap_or(50) * 1024 * 1024;

    let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(&path) else {
        std::process::exit(0);
    };
    let started = std::time::Instant::now();
    // A header line: version, start time, and the pane size for the player.
    let _ = writeln!(file, "{{\"v\":1,\"t\":{},\"cols\":{cols},\"rows\":{rows}}}", now_ms());
    let mut written: u64 = 0;
    let mut buf = [0u8; 16 * 1024];
    let mut stdin = std::io::stdin();
    loop {
        match stdin.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                if written >= max_bytes {
                    continue; // keep draining so tmux's pipe never blocks
                }
                let ms = started.elapsed().as_millis();
                let data = b64(&buf[..n]);
                if writeln!(file, "[{ms},\"{data}\"]").is_err() {
                    break;
                }
                written += n as u64;
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }
    std::process::exit(0);
}

/// Standard base64, no line breaks (the recorder has no outside crate here).
fn b64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((bytes.len() + 2) / 3 * 4);
    for c in bytes.chunks(3) {
        let b = [c[0], *c.get(1).unwrap_or(&0), *c.get(2).unwrap_or(&0)];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(T[(n >> 18 & 63) as usize] as char);
        out.push(T[(n >> 12 & 63) as usize] as char);
        out.push(if c.len() > 1 { T[(n >> 6 & 63) as usize] as char } else { '=' });
        out.push(if c.len() > 2 { T[(n & 63) as usize] as char } else { '=' });
    }
    out
}


// --- Administrator API: settings, listing, playback, deletion -------------

fn require_super(req: &HttpRequest, store: &crate::users::UserStore) -> Result<(), HttpResponse> {
    crate::membership::require_super(req, store)
}

async fn get_settings(req: HttpRequest, store: web::Data<crate::users::UserStore>) -> HttpResponse {
    if let Err(r) = require_super(&req, &store) { return r; }
    HttpResponse::Ok().insert_header(("Cache-Control", "no-store")).json(ApiResponse::ok(settings()))
}

async fn put_settings(req: HttpRequest, store: web::Data<crate::users::UserStore>, body: web::Json<Settings>) -> HttpResponse {
    if let Err(r) = require_super(&req, &store) { return r; }
    match save_settings(&body) {
        Ok(()) => HttpResponse::Ok().json(ApiResponse::ok("saved")),
        Err(e) => HttpResponse::BadRequest().json(ApiResponse::<()>::error(&e)),
    }
}

#[derive(serde::Serialize)]
struct Row {
    sid: String,
    owner: String,
    label: String,
    started_ms: u128,
    bytes: u64,
    live: bool,
}

/// Recordings, newest first. Sweeps away ones past the retention window first.
async fn list(req: HttpRequest, store: web::Data<crate::users::UserStore>, sessions: web::Data<super::session::PtyStore>) -> HttpResponse {
    if let Err(r) = require_super(&req, &store) { return r; }
    sweep();
    let live: std::collections::HashSet<String> = sessions.lock().map(|m| m.keys().cloned().collect()).unwrap_or_default();
    let mut rows: Vec<Row> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir()) {
        for e in entries.flatten() {
            let path = e.path();
            if path.extension().and_then(|x| x.to_str()) != Some("json") { continue; }
            let Ok(bytes) = std::fs::read(&path) else { continue };
            let Ok(meta) = serde_json::from_slice::<Meta>(&bytes) else { continue };
            let size = cast_path(&meta.sid).and_then(|p| std::fs::metadata(p).ok()).map(|m| m.len()).unwrap_or(0);
            rows.push(Row { live: live.contains(&meta.sid), sid: meta.sid, owner: meta.owner, label: meta.label, started_ms: meta.started_ms, bytes: size });
        }
    }
    rows.sort_by_key(|r| std::cmp::Reverse(r.started_ms));
    HttpResponse::Ok().insert_header(("Cache-Control", "no-store")).json(ApiResponse::ok(rows))
}

/// The recording stream itself, for the player to replay.
async fn get_one(req: HttpRequest, store: web::Data<crate::users::UserStore>, path: web::Path<String>) -> HttpResponse {
    if let Err(r) = require_super(&req, &store) { return r; }
    let Some(file) = cast_path(&path) else { return HttpResponse::BadRequest().finish() };
    match std::fs::read(file) {
        Ok(bytes) => HttpResponse::Ok().insert_header(("Cache-Control", "no-store")).content_type("application/x-ndjson").body(bytes),
        Err(_) => HttpResponse::NotFound().json(ApiResponse::<()>::error("Recording not found")),
    }
}

async fn delete_one(req: HttpRequest, store: web::Data<crate::users::UserStore>, path: web::Path<String>) -> HttpResponse {
    if let Err(r) = require_super(&req, &store) { return r; }
    let sid = path.into_inner();
    if let (Some(c), Some(m)) = (cast_path(&sid), meta_path(&sid)) {
        let _ = std::fs::remove_file(c);
        let _ = std::fs::remove_file(m);
        return HttpResponse::Ok().json(ApiResponse::ok("deleted"));
    }
    HttpResponse::BadRequest().finish()
}

/// Removes recordings older than the retention window (0 keeps them all).
pub fn sweep() {
    let days = settings().retention_days;
    if days == 0 { return; }
    let cutoff = std::time::SystemTime::now().checked_sub(std::time::Duration::from_secs(u64::from(days) * 86400));
    let Some(cutoff) = cutoff else { return };
    if let Ok(entries) = std::fs::read_dir(dir()) {
        for e in entries.flatten() {
            if let Ok(meta) = e.metadata() {
                if meta.modified().map(|m| m < cutoff).unwrap_or(false) {
                    let _ = std::fs::remove_file(e.path());
                }
            }
        }
    }
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.route("/api/terminal/recording/settings", web::get().to(get_settings))
        .route("/api/terminal/recording/settings", web::put().to(put_settings))
        .route("/api/terminal/recordings", web::get().to(list))
        .route("/api/terminal/recordings/{sid}", web::get().to(get_one))
        .route("/api/terminal/recordings/{sid}", web::delete().to(delete_one));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_known_vectors() {
        assert_eq!(b64(b""), "");
        assert_eq!(b64(b"f"), "Zg==");
        assert_eq!(b64(b"fo"), "Zm8=");
        assert_eq!(b64(b"foo"), "Zm9v");
        assert_eq!(b64(b"foobar"), "Zm9vYmFy");
        assert_eq!(b64(&[0, 255, 16]), "AP8Q");
    }

    #[test]
    fn only_safe_session_ids_get_a_path() {
        assert!(cast_path("pty_6aa9").is_some());
        assert!(cast_path("../etc/passwd").is_none() && cast_path("a b").is_none() && cast_path("").is_none());
    }

    #[test]
    fn settings_default_off_and_bounded() {
        let d = Settings::default();
        assert!(!d.enabled && !d.keystrokes && d.retention_days == 30 && d.max_mb == 50);
        assert!(save_settings(&Settings { max_mb: 5000, ..Default::default() }).is_err());
    }
}
