//! Serial devices attached to terminal sessions (`/api/terminal/serial`): a
//! device's name is unique within its session only, its PTY lives under the
//! session owner's directory (`serial/<owner>/<sid>/<name>`), and whoever may
//! open the session may attach to it (see docs/serial-devices-design.md). The
//! bridge itself — socat, the relay, the reset/bootloader channel — is in
//! `serial_bridge`.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use actix_web::{web, HttpRequest, HttpResponse};
use actix_web_actors::ws;

use super::serial_bridge::{safe_name, serial_dir, SerialBridge};
use super::session::{PtyStore, TmuxSession};
use crate::stream_access::ConnectionGrant;

const MAX_PER_SESSION: usize = 8;
const MAX_PER_ACCOUNT: usize = 8;
const MAX_TOTAL: usize = 64;

/// A device attached to a session, as listed to the people who can see the session.
#[derive(Clone, serde::Serialize)]
pub(crate) struct Attached {
    pub name: String,
    pub path: String,
    #[serde(skip)]
    pub attacher_id: String,
    pub attached_by: String,
    pub since_unix: u64,
}

/// Live session attachments: session id → device name → device.
static HUB: LazyLock<Mutex<HashMap<String, HashMap<String, Attached>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Reserve `(sid, name)` in the hub, or say why not.
fn reserve(sid: &str, device: Attached) -> Result<(), &'static str> {
    let mut hub = HUB.lock().unwrap_or_else(|e| e.into_inner());
    let total: usize = hub.values().map(HashMap::len).sum();
    let mine = hub.values().flat_map(HashMap::values).filter(|d| d.attacher_id == device.attacher_id).count();
    let here = hub.get(sid);
    if here.is_some_and(|h| h.contains_key(&device.name)) {
        return Err("A device with this name is already attached to the session");
    }
    if here.map_or(0, HashMap::len) >= MAX_PER_SESSION || mine >= MAX_PER_ACCOUNT || total >= MAX_TOTAL {
        return Err("Too many serial devices attached");
    }
    hub.entry(sid.to_string()).or_default().insert(device.name.clone(), device);
    Ok(())
}

pub(super) fn release(sid: &str, name: &str) {
    let mut hub = HUB.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(here) = hub.get_mut(sid) {
        here.remove(name);
        if here.is_empty() {
            hub.remove(sid);
        }
    }
}

/// Phase 1 bridges devices into local shells only; a remote session needs the SSH forward of phase 3.
fn is_local(s: &TmuxSession) -> bool {
    s.ssh_cmd.is_none() && s.remote_binding.is_none()
}

/// Whether this account may still use this session's devices.
pub(super) fn session_admits(store: &PtyStore, sid: &str, grant: &ConnectionGrant) -> bool {
    store
        .lock()
        .ok()
        .and_then(|m| m.get(sid).cloned())
        .is_some_and(|s| grant.can_access_session(&s) && super::remote_access::allowed(&s) && is_local(&s))
}

/// What a session-attached bridge re-checks while it runs.
pub(super) struct SessionScope {
    pub(super) sid: String,
    pub(super) name: String,
    pub(super) grant: ConnectionGrant,
    pub(super) store: PtyStore,
}

/// `GET /api/terminal/serial?session_id=<sid>&serial=<name>&ticket=…` — attach a
/// device to one terminal session. Its PTY is `serial/<owner>/<sid>/<name>`.
#[actix_web::get("/api/terminal/serial")]
async fn ws_session_serial(
    req: HttpRequest,
    stream: web::Payload,
    store: web::Data<PtyStore>,
    sessions: web::Data<crate::jwt_sessions::SessionStore>,
    users: web::Data<crate::users::UserStore>,
) -> Result<HttpResponse, actix_web::Error> {
    ws::handshake(&req)?;
    let Some(grant) = ConnectionGrant::from_request(
        &req,
        sessions.get_ref().clone(),
        users.get_ref().clone(),
    )
    .filter(|g| g.valid()) else {
        return Ok(HttpResponse::Unauthorized().finish());
    };
    let query = req.query_string();
    let sid = super::parse_query_param(query, "session_id").unwrap_or_default();
    let name = super::parse_query_param(query, "serial").unwrap_or_default();
    let safe = safe_name(&name, 24);
    if safe.is_empty() || safe != name || !sid.starts_with("pty_") || safe_name(&sid, 40) != sid {
        return Ok(HttpResponse::BadRequest().finish());
    }
    let session = store.lock().ok().and_then(|m| m.get(&sid).cloned());
    let Some(session) = session.filter(|s| grant.can_access_session(s) && super::remote_access::allowed(s)) else {
        return Ok(HttpResponse::NotFound().body("Session not found"));
    };
    if !is_local(&session) {
        return Ok(HttpResponse::Conflict().body("Serial devices reach local sessions only for now; remote server sessions come later"));
    }
    let owner = safe_name(&session.owner_id, 64);
    if owner.is_empty() {
        return Ok(HttpResponse::NotFound().body("Session not found"));
    }
    let dir = serial_dir().join(&owner).join(&sid);
    if let Err(e) = private_dir(&dir) {
        eprintln!("[serial] cannot prepare {}: {e}", dir.display());
        return Ok(HttpResponse::InternalServerError().finish());
    }
    let link = dir.join(&safe).to_string_lossy().into_owned();
    let device = Attached {
        name: safe.clone(),
        path: link.clone(),
        attacher_id: grant.user_id.clone(),
        attached_by: grant.username.clone(),
        since_unix: super::session::now_unix(),
    };
    if let Err(why) = reserve(&sid, device) {
        return Ok(HttpResponse::Conflict().body(why));
    }
    let scope = SessionScope { sid: sid.clone(), name: safe.clone(), grant, store: store.get_ref().clone() };
    let bridge = SerialBridge::new(link, Some(scope));
    let started = ws::WsResponseBuilder::new(bridge, &req, stream).start();
    if started.is_err() {
        release(&sid, &safe);
    }
    started
}

/// `serial/`, `serial/<owner>`, `serial/<owner>/<sid>`: created as needed, owner and session levels 0700.
fn private_dir(dir: &std::path::Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(dir)?;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    if let Some(owner) = dir.parent() {
        std::fs::set_permissions(owner, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

/// `GET /api/terminal/sessions/{id}/serial` — the devices attached to a session,
/// for anyone who can see the session.
async fn list_session_devices(
    path: web::Path<String>,
    store: web::Data<PtyStore>,
    users: web::Data<crate::users::UserStore>,
    req: HttpRequest,
) -> HttpResponse {
    let sid = path.into_inner();
    let user_id = crate::auth::verified_user_id(&req).unwrap_or_default();
    let visible = store
        .lock()
        .ok()
        .and_then(|m| m.get(&sid).cloned())
        .is_some_and(|s| super::session::visible_to(&s, &user_id, &users) && super::remote_access::allowed(&s));
    if !visible {
        return HttpResponse::Ok().json(crate::models::ApiResponse::<()>::error("Session not found"));
    }
    let hub = HUB.lock().unwrap_or_else(|e| e.into_inner());
    let mut devices: Vec<serde_json::Value> = hub
        .get(&sid)
        .map(|here| {
            here.values()
                .map(|d| {
                    let mut v = serde_json::to_value(d).unwrap_or_default();
                    v["mine"] = serde_json::Value::Bool(d.attacher_id == user_id);
                    v
                })
                .collect()
        })
        .unwrap_or_default();
    devices.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    HttpResponse::Ok()
        .insert_header(("Cache-Control", "no-store"))
        .json(crate::models::ApiResponse::ok(devices))
}

pub(super) fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(ws_session_serial)
        .route("/api/terminal/sessions/{id}/serial", web::get().to(list_session_devices));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(name: &str, who: &str) -> Attached {
        Attached { name: name.into(), path: String::new(), attacher_id: who.into(), attached_by: who.into(), since_unix: 0 }
    }

    #[test]
    fn names_are_unique_per_session_only() {
        reserve("pty_t1a", device("esp32", "u1")).unwrap();
        assert!(reserve("pty_t1a", device("esp32", "u2")).is_err());
        reserve("pty_t1b", device("esp32", "u2")).unwrap();
        release("pty_t1a", "esp32");
        reserve("pty_t1a", device("esp32", "u2")).unwrap();
        release("pty_t1a", "esp32");
        release("pty_t1b", "esp32");
    }

    #[test]
    fn per_session_and_account_limits() {
        for i in 0..MAX_PER_SESSION {
            reserve("pty_t2", device(&format!("d{i}"), &format!("acct{i}"))).unwrap();
        }
        assert!(reserve("pty_t2", device("one-more", "someone")).is_err());
        for i in 0..MAX_PER_SESSION {
            release("pty_t2", &format!("d{i}"));
        }
        for i in 0..MAX_PER_ACCOUNT {
            reserve(&format!("pty_t3_{i}"), device("esp32", "busy")).unwrap();
        }
        assert!(reserve("pty_t3_x", device("esp32", "busy")).is_err());
        for i in 0..MAX_PER_ACCOUNT {
            release(&format!("pty_t3_{i}"), "esp32");
        }
        assert!(HUB.lock().unwrap().keys().all(|k| !k.starts_with("pty_t2") && !k.starts_with("pty_t3")));
    }

    #[test]
    fn names_are_filtered() {
        assert_eq!(safe_name("esp32/../x", 24), "esp32x");
        assert_eq!(safe_name("a b-c_d", 24), "ab-c_d");
        assert_eq!(safe_name(&"x".repeat(50), 24).len(), 24);
    }
}
