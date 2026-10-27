//! Dictation into a terminal: the browser's microphone, transcribed live by
//! the speech service (v.bebo.kr's Qwen3-ASR), typed at the prompt.
//!
//! The browser cannot send the service's API key on a WebSocket, and should
//! not hold it anyway, so NabiMan relays: the browser streams 16 kHz PCM to
//! `/api/terminal/dictation` (admitted like a terminal, by a one-use ticket),
//! and NabiMan opens the service's live transcription with the key kept here,
//! encrypted. Nothing is kept by the service (`save: false`): people dictate
//! commands, and sometimes what should never be written down.

mod bridge;
mod voice;

use actix_web::{web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};

use crate::models::ApiResponse;

pub(crate) const LANGUAGES: [&str; 5] = ["korean", "english", "japanese", "chinese", "auto"];
const DEFAULT_URL: &str = "https://v.bebo.kr";

/// Where the speech service is and how to sign in to it.
#[derive(Serialize, Deserialize, Clone, Default)]
pub(crate) struct Settings {
    /// The service's base URL; empty means v.bebo.kr.
    #[serde(default)]
    pub url: String,
    /// `host:port` to connect to instead of what the URL's name resolves to,
    /// for a service on this machine whose public address is not reachable
    /// from here. The URL's name is still what TLS checks.
    #[serde(default)]
    pub address: String,
    #[serde(default)]
    pub language: String,
    /// The API key (`vk_…`), encrypted with the server's secret key.
    #[serde(default)]
    pub key_enc: String,
    /// How long a pause ends what was said (ms), for the service's end-of-speech
    /// detection; 0 turns it off. None is the default.
    #[serde(default)]
    pub endpoint_ms: Option<u32>,
    /// In a terminal, press Enter at each pause too, not only when "엔터" is
    /// said. None is the default: on.
    #[serde(default)]
    pub auto_enter: Option<bool>,
    /// Take only the voice enrolled with the service's account ("내 목소리만"):
    /// the key's owner, since every dictation here goes out with one key.
    #[serde(default)]
    pub voice_lock: bool,
    /// Read each dictated line's answer aloud, as if "음성으로 대답해줘" had
    /// been said; a session may set its own. Off unless turned on.
    #[serde(default)]
    pub voice_reply: bool,
}

/// Where a pause of this long can end what was said; the service takes 300 to 10000 ms.
pub(crate) fn valid_pause(ms: u64) -> bool {
    ms == 0 || (300..=10_000).contains(&ms)
}

impl Settings {
    pub fn url(&self) -> &str {
        if self.url.is_empty() { DEFAULT_URL } else { &self.url }
    }
    pub fn endpoint_ms(&self) -> u32 {
        self.endpoint_ms.unwrap_or(1500)
    }
    pub fn auto_enter(&self) -> bool {
        self.auto_enter.unwrap_or(true)
    }
    pub fn language(&self) -> &str {
        if self.language.is_empty() { "korean" } else { &self.language }
    }
    pub fn key(&self) -> Option<String> {
        (!self.key_enc.is_empty()).then(|| crate::secret_store::decrypt(&self.key_enc).ok()).flatten()
    }
}

fn path() -> std::path::PathBuf {
    nabiman_server::data_dir().join("dictation.json")
}

pub(crate) fn load() -> Settings {
    std::fs::read(path()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn save(settings: &Settings) -> Result<(), String> {
    let json = serde_json::to_vec_pretty(settings).map_err(|e| e.to_string())?;
    crate::private_storage::write(&path(), &json).map_err(|e| format!("Cannot save dictation settings: {e}"))
}

/// A base URL: http or https, a host, nothing after it but a slash.
pub(crate) fn valid_url(text: &str) -> Option<String> {
    let url = reqwest::Url::parse(text.trim()).ok()?;
    let plain = matches!(url.scheme(), "http" | "https")
        && url.host_str().is_some_and(|h| !h.is_empty())
        && url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
        && url.path() == "/";
    plain.then(|| url.as_str().trim_end_matches('/').to_string())
}

/// `host:port`, or empty for none.
pub(crate) fn valid_address(text: &str) -> bool {
    let t = text.trim();
    t.is_empty()
        || t.rsplit_once(':').is_some_and(|(host, port)| {
            !host.is_empty() && port.parse::<u16>().is_ok_and(|p| p > 0)
                && host.chars().all(|c| c.is_ascii_alphanumeric() || ".-[]:".contains(c))
        })
}

pub(crate) fn valid_key(key: &str) -> bool {
    key.starts_with("vk_") && key.len() <= 200 && key.chars().all(|c| c.is_ascii_graphic())
}

pub(crate) fn super_admin(req: &HttpRequest) -> Result<(), HttpResponse> {
    let users = req.app_data::<web::Data<crate::users::UserStore>>()
        .ok_or_else(|| HttpResponse::ServiceUnavailable().finish())?;
    let actor = crate::membership::actor(req, users)?;
    if actor.super_admin() { Ok(()) } else {
        Err(HttpResponse::Forbidden().json(ApiResponse::<()>::error("Only a super administrator can change dictation")))
    }
}

/// What the terminal needs to know (whether to offer the microphone), and
/// for an administrator, how it is set up. The key itself never leaves.
async fn get_settings(req: HttpRequest) -> HttpResponse {
    let s = load();
    let admin = super_admin(&req).is_ok();
    let key = s.key();
    let mut out = serde_json::json!({
        "available": key.is_some(), "language": s.language(),
        "endpoint_ms": s.endpoint_ms(), "auto_enter": s.auto_enter(), "voice_lock": s.voice_lock, "voice_reply": s.voice_reply,
    });
    if admin {
        out["url"] = s.url().into();
        out["address"] = s.address.clone().into();
        out["key_set"] = key.is_some().into();
        out["key_hint"] = key.map(|k| format!("…{}", &k[k.len().saturating_sub(4)..])).into();
    }
    HttpResponse::Ok().insert_header(("Cache-Control", "no-store")).json(ApiResponse::ok(out))
}

#[derive(Deserialize)]
struct Change {
    url: Option<String>,
    address: Option<String>,
    language: Option<String>,
    /// Empty clears it.
    key: Option<String>,
    endpoint_ms: Option<u32>,
    auto_enter: Option<bool>,
    voice_lock: Option<bool>,
    voice_reply: Option<bool>,
}

async fn put_settings(req: HttpRequest, body: web::Json<Change>) -> HttpResponse {
    if let Err(r) = super_admin(&req) {
        return r;
    }
    let bad = |m: &str| HttpResponse::BadRequest().json(ApiResponse::<()>::error(m));
    let mut s = load();
    if let Some(url) = &body.url {
        match valid_url(url) {
            Some(u) => s.url = u,
            None => return bad("The address should be like https://v.bebo.kr"),
        }
    }
    if let Some(address) = &body.address {
        if !valid_address(address) {
            return bad("Connect to should be host:port, such as 127.0.0.1:443, or empty");
        }
        s.address = address.trim().to_string();
    }
    if let Some(ms) = body.endpoint_ms {
        if !valid_pause(u64::from(ms)) {
            return bad("A pause is 300 to 10000 ms, or 0 for off");
        }
        s.endpoint_ms = Some(ms);
    }
    if let Some(on) = body.auto_enter {
        s.auto_enter = Some(on);
    }
    if let Some(on) = body.voice_reply {
        s.voice_reply = on;
    }
    if let Some(on) = body.voice_lock {
        s.voice_lock = on;
    }
    if let Some(language) = &body.language {
        if !LANGUAGES.contains(&language.as_str()) {
            return bad("Unknown language");
        }
        s.language = language.clone();
    }
    if let Some(key) = body.key.as_deref().map(str::trim) {
        if key.is_empty() {
            s.key_enc.clear();
        } else if !valid_key(key) {
            return bad("An API key starts with vk_");
        } else {
            match crate::secret_store::encrypt(key) {
                Ok(enc) => s.key_enc = enc,
                Err(e) => return HttpResponse::InternalServerError().json(ApiResponse::<()>::error(&e)),
            }
        }
    }
    match save(&s) {
        Ok(()) => get_settings(req).await,
        Err(e) => HttpResponse::InternalServerError().json(ApiResponse::<()>::error(&e)),
    }
}

/// `text` read aloud by the speech service in `speaker`'s voice, as WAV, with
/// the key kept here; nothing is saved there.
pub(crate) async fn speak(text: &str, speaker: &str, language: &str, instruct: &str) -> Result<Vec<u8>, String> {
    let s = load();
    let key = s.key().ok_or("Dictation is not set up: no API key for the speech service")?;
    let body = serde_json::json!({ "text": text, "speaker": speaker, "language": language, "instruct": instruct, "save": false });
    voice::speak(&s, &key, &body).await
}

/// Whether the speech service has a key, so the microphone and voice can work.
pub(crate) fn ready() -> bool {
    load().key().is_some()
}

/// Signs in to the service and asks whether it is ready.
async fn test(req: HttpRequest) -> HttpResponse {
    if let Err(r) = super_admin(&req) {
        return r;
    }
    let s = load();
    let Some(key) = s.key() else {
        return HttpResponse::Ok().json(ApiResponse::<()>::error("No API key is set"));
    };
    match voice::health(&s, &key).await {
        Ok(health) => HttpResponse::Ok().json(ApiResponse::ok(health)),
        Err(e) => HttpResponse::Ok().json(ApiResponse::<()>::error(&e)),
    }
}

/// Whether the service's account (the key's owner) has a voice enrolled for
/// "내 목소리만", and where to enrol one: the service's own account page, which
/// asks for consent and checks the recording. Nothing of the voice comes here.
async fn voiceprint() -> HttpResponse {
    let s = load();
    let Some(key) = s.key() else {
        return HttpResponse::Ok().json(ApiResponse::<()>::error("No API key is set"));
    };
    let enrol_at = format!("{}/account#voice", s.url());
    match voice::voiceprint(&s, &key).await {
        Ok(v) => {
            let flag = |k: &str| v.get(k).and_then(|x| x.as_bool()).unwrap_or(false);
            let text = |k: &str| v.get(k).and_then(|x| x.as_str()).map(str::to_string);
            HttpResponse::Ok().insert_header(("Cache-Control", "no-store")).json(ApiResponse::ok(serde_json::json!({
                "enrolled": flag("enrolled"), "stale": flag("stale"),
                "redo": v.get("redo").is_some_and(|r| !r.is_null()),
                "created_at": text("created_at"), "last_used_at": text("last_used_at"),
                "use_count": v.get("use_count").and_then(|x| x.as_u64()), "enrol_at": enrol_at,
            })))
        }
        Err(e) => HttpResponse::Ok().json(ApiResponse::<()>::error(&e)),
    }
}

pub(crate) fn config(cfg: &mut web::ServiceConfig) {
    cfg.route("/api/terminal/dictation", web::get().to(bridge::start))
        // Public: checked against the share link in `start_shared`.
        .route("/api/share/{token}/dictation", web::get().to(bridge::start_shared))
        // Public: checked against the console share link in `start_console_shared`.
        .route("/api/cshare/{token}/dictation", web::get().to(bridge::start_console_shared))
        .route("/api/terminal/dictation/settings", web::get().to(get_settings))
        .route("/api/terminal/dictation/settings", web::put().to(put_settings))
        .route("/api/terminal/dictation/test", web::post().to(test))
        .route("/api/terminal/dictation/voiceprint", web::get().to(voiceprint));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enter_at_a_pause_is_on_unless_turned_off() {
        let unset: Settings = serde_json::from_str(r#"{"url":"https://v.bebo.kr"}"#).unwrap();
        assert!(unset.auto_enter());
        let off: Settings = serde_json::from_str(r#"{"auto_enter":false}"#).unwrap();
        assert!(!off.auto_enter());
    }

    #[test]
    fn addresses_and_keys_are_checked() {
        assert_eq!(valid_url("https://v.bebo.kr/").as_deref(), Some("https://v.bebo.kr"));
        assert_eq!(valid_url("http://127.0.0.1:19999").as_deref(), Some("http://127.0.0.1:19999"));
        for bad in ["ftp://x", "https://v.bebo.kr/api", "https://u:p@v.bebo.kr", "v.bebo.kr", "https://x/?a=1"] {
            assert_eq!(valid_url(bad), None, "{bad}");
        }
        assert!(valid_address("") && valid_address("127.0.0.1:443") && valid_address("[::1]:443"));
        assert!(!valid_address("127.0.0.1") && !valid_address("host:0") && !valid_address("a b:1"));
        assert!(valid_key("vk_abcDEF-123_x"));
        assert!(!valid_key("sk_abc") && !valid_key("vk_with space"));
    }

    #[test]
    fn defaults_point_at_v_bebo_kr_in_korean() {
        let s = Settings::default();
        assert_eq!((s.url(), s.language()), ("https://v.bebo.kr", "korean"));
        assert!(s.key().is_none());
    }
}
