//! A session's own settings, over the account's: what is not set here
//! follows the terminal settings (and dictation's) as they are now.
//!
//! What a session can set for itself is how it is shown and dictated into —
//! font, size, history kept in the browser, replaying history, the pause
//! that ends a sentence, whether it presses Enter, "내 목소리만", and whether
//! answers are read aloud. Whether it is kept
//! alive and when it closes were always the session's own (fixed when it
//! was made, changed by their own routes); the SSH keep-alive interval is
//! used only as the connection starts, so it stays with the account.
//!
//! Kept in a file of their own, beside the session registry: a registry
//! format change could stop a restart, and a release rolled back never
//! reads this file. A file that cannot be read starts empty.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use actix_web::{web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};

use super::session::{PtyStore, MAX_SCROLLBACK};
use super::settings::FONTS;
use crate::models::ApiResponse;

/// Settings one session may have; `None` follows the account's.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Overrides {
    #[serde(default)]
    pub font_family: Option<String>,
    #[serde(default)]
    pub font_size: Option<u32>,
    #[serde(default)]
    pub scrollback_lines: Option<u32>,
    #[serde(default)]
    pub restore_scrollback: Option<bool>,
    #[serde(default)]
    pub dictation_pause_ms: Option<u32>,
    #[serde(default)]
    pub dictation_auto_enter: Option<bool>,
    #[serde(default)]
    pub dictation_voice_lock: Option<bool>,
    /// Each dictated line's answer read aloud ("음성으로 대답하기").
    #[serde(default)]
    pub dictation_voice_reply: Option<bool>,
}

impl Overrides {
    fn is_empty(&self) -> bool {
        *self == Overrides::default()
    }

    pub(crate) fn check(&self) -> Result<(), String> {
        if self.font_family.as_deref().is_some_and(|f| !FONTS.contains(&f)) {
            return Err("Unknown font".into());
        }
        if self.font_size.is_some_and(|s| !(8..=28).contains(&s)) {
            return Err("The text size is 8 to 28".into());
        }
        if self.scrollback_lines.is_some_and(|n| !(100..=MAX_SCROLLBACK).contains(&n)) {
            return Err(format!("History is 100 to {MAX_SCROLLBACK} lines"));
        }
        if self.dictation_pause_ms.is_some_and(|ms| !super::dictation::valid_pause(u64::from(ms))) {
            return Err("A pause is 300 to 10000 ms, or 0 for off".into());
        }
        Ok(())
    }
}

type Map = HashMap<String, Overrides>;

fn file() -> PathBuf {
    nabiman_server::data_dir().join("terminal_session_settings.json")
}

fn load(file: &Path) -> Map {
    let Ok(bytes) = std::fs::read(file) else { return Map::new() };
    serde_json::from_slice(&bytes).unwrap_or_else(|e| {
        eprintln!("[terminal] {} is unreadable ({e}); sessions follow the account settings", file.display());
        Map::new()
    })
}

fn store() -> &'static Mutex<Map> {
    static STORE: OnceLock<Mutex<Map>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(load(&file())))
}

/// Keeps the sessions still registered; the rest are gone for good.
fn prune(map: &mut Map, live: &dyn Fn(&str) -> bool) {
    map.retain(|id, o| live(id) && !o.is_empty());
}

fn save(map: &Map, file: &Path) -> Result<(), String> {
    let json = serde_json::to_vec(map).map_err(|e| e.to_string())?;
    crate::private_storage::write(file, &json).map_err(|e| format!("Cannot save the session's settings: {e}"))
}

/// The session's settings, if the caller owns it; what the session itself
/// holds (kept alive, closing time) comes along to show beside them.
fn owned(req: &HttpRequest, sessions: &PtyStore, id: &str) -> Result<serde_json::Value, HttpResponse> {
    let user = crate::auth::verified_user_id(req).unwrap_or_default();
    let map = sessions.lock().unwrap_or_else(|e| e.into_inner());
    match map.get(id) {
        Some(s) if s.owned_by(&user) => Ok(serde_json::json!({ "keepalive": s.keepalive, "timeout_secs": s.timeout_secs })),
        Some(_) => Err(HttpResponse::Forbidden().json(ApiResponse::<()>::error("Not owner"))),
        None => Err(HttpResponse::NotFound().json(ApiResponse::<()>::error("Session not found"))),
    }
}

async fn get(req: HttpRequest, path: web::Path<String>, sessions: web::Data<PtyStore>) -> HttpResponse {
    let id = path.into_inner();
    let session = match owned(&req, &sessions, &id) {
        Ok(s) => s,
        Err(r) => return r,
    };
    let overrides = store().lock().unwrap_or_else(|e| e.into_inner()).get(&id).cloned().unwrap_or_default();
    HttpResponse::Ok().insert_header(("Cache-Control", "no-store"))
        .json(ApiResponse::ok(serde_json::json!({ "overrides": overrides, "session": session })))
}

async fn put(req: HttpRequest, path: web::Path<String>, body: web::Json<Overrides>, sessions: web::Data<PtyStore>) -> HttpResponse {
    let id = path.into_inner();
    let session = match owned(&req, &sessions, &id) {
        Ok(s) => s,
        Err(r) => return r,
    };
    let overrides = body.into_inner();
    if let Err(e) = overrides.check() {
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error(&e));
    }
    let live: Vec<String> = sessions.lock().unwrap_or_else(|e| e.into_inner()).keys().cloned().collect();
    let mut map = store().lock().unwrap_or_else(|e| e.into_inner());
    let mut next = map.clone();
    next.insert(id, overrides.clone());
    prune(&mut next, &|sid| live.iter().any(|l| l == sid));
    if let Err(e) = save(&next, &file()) {
        return HttpResponse::ServiceUnavailable().json(ApiResponse::<()>::error(&e));
    }
    *map = next;
    HttpResponse::Ok().json(ApiResponse::ok(serde_json::json!({ "overrides": overrides, "session": session })))
}

pub(super) fn config(cfg: &mut web::ServiceConfig) {
    cfg.route("/api/terminal/sessions/{id}/settings", web::get().to(get))
        .route("/api/terminal/sessions/{id}/settings", web::put().to(put));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_are_checked_and_unset_ones_follow_the_account() {
        assert!(Overrides::default().check().is_ok());
        let ok = Overrides { font_family: Some("jetbrains".into()), font_size: Some(12), dictation_pause_ms: Some(5000), ..Overrides::default() };
        assert!(ok.check().is_ok());
        for bad in [
            Overrides { font_family: Some("comic".into()), ..Overrides::default() },
            Overrides { font_size: Some(40), ..Overrides::default() },
            Overrides { scrollback_lines: Some(10), ..Overrides::default() },
            Overrides { dictation_pause_ms: Some(50), ..Overrides::default() },
        ] {
            assert!(bad.check().is_err(), "{bad:?}");
        }
        let parsed: Overrides = serde_json::from_str(r#"{"font_size":13,"font_family":null}"#).unwrap();
        assert_eq!((parsed.font_size, parsed.font_family), (Some(13), None));
        assert!(serde_json::from_str::<Overrides>(r#"{"keepalive_interval_secs":5}"#).is_err());
    }

    #[test]
    fn ended_and_empty_sessions_are_dropped_and_a_bad_file_starts_empty() {
        let mut map = Map::new();
        map.insert("live".into(), Overrides { font_size: Some(12), ..Overrides::default() });
        map.insert("gone".into(), Overrides { font_size: Some(12), ..Overrides::default() });
        map.insert("plain".into(), Overrides::default());
        prune(&mut map, &|id| id != "gone");
        assert_eq!(map.keys().collect::<Vec<_>>(), vec!["live"]);

        let file = std::env::temp_dir().join(format!("nabiman-session-settings-{}.json", uuid::Uuid::new_v4()));
        save(&map, &file).unwrap();
        assert_eq!(load(&file), map);
        std::fs::write(&file, b"{broken").unwrap();
        assert!(load(&file).is_empty());
        std::fs::remove_file(&file).unwrap();
    }
}
