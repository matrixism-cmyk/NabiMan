//! The console registry's HTTP handlers (list, register, edit, delete, and
//! per-console access), kept apart from the proxy actors in vnc_console.

use actix_web::{web, HttpRequest, HttpResponse};

use crate::models::ApiResponse;
use super::vnc_console::{is_admin, may_manage_console, port_for, CONSOLES};

/// `GET /api/remote/consoles` — the VM consoles the caller may open: the
/// built-in internal bridges (read-only) and the admin-registered ones.
#[actix_web::get("/api/remote/consoles")]
async fn list_consoles(req: HttpRequest) -> HttpResponse {
    let admin = is_admin(&req);
    let uid = crate::auth::verified_user_id(&req).unwrap_or_default();
    let visible = |key: &str| admin || super::vnc_access::allowed(&uid, key);
    let mut list: Vec<serde_json::Value> = CONSOLES
        .iter()
        .filter(|(name, _, _)| visible(name))
        .map(|(name, label, _)| serde_json::json!({ "name": name, "label": label, "protocol": "vnc", "builtin": true }))
        .collect();
    for c in super::vnc_registry::load() {
        if visible(&c.id) {
            list.push(serde_json::json!({
                "name": c.id, "label": c.label, "host": c.host, "port": c.port,
                "protocol": c.protocol, "user": c.user, "has_password": !c.password_enc.is_empty(), "builtin": false
            }));
        }
    }
    HttpResponse::Ok().json(ApiResponse::ok(list))
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ConsoleBody {
    label: String,
    host: String,
    port: u16,
    #[serde(default = "vnc_default")]
    protocol: String,
    #[serde(default)]
    user: String,
    #[serde(default)]
    password: Option<String>,
}
fn vnc_default() -> String {
    "vnc".into()
}
fn as_json(c: super::vnc_registry::Console) -> serde_json::Value {
    serde_json::json!({ "name": c.id, "label": c.label, "host": c.host, "port": c.port,
        "protocol": c.protocol, "user": c.user, "has_password": !c.password_enc.is_empty(), "builtin": false })
}
/// `POST /api/remote/consoles` — register an external VNC console (admin).
#[actix_web::post("/api/remote/consoles")]
async fn create_console(req: HttpRequest, body: web::Json<ConsoleBody>) -> HttpResponse {
    if !may_manage_console(&req) {
        return HttpResponse::Forbidden().finish();
    }
    match super::vnc_registry::create(&body.label, &body.host, body.port, &body.protocol, &body.user, body.password.as_deref().unwrap_or("")) {
        Ok(c) => HttpResponse::Ok().json(ApiResponse::ok(as_json(c))),
        Err(e) => HttpResponse::BadRequest().json(ApiResponse::<()>::error(&e)),
    }
}
/// `PUT /api/remote/consoles/{id}` — edit a registered console (admin).
#[actix_web::put("/api/remote/consoles/{id}")]
async fn update_console(req: HttpRequest, id: web::Path<String>, body: web::Json<ConsoleBody>) -> HttpResponse {
    if !may_manage_console(&req) {
        return HttpResponse::Forbidden().finish();
    }
    match super::vnc_registry::update(&id, &body.label, &body.host, body.port, &body.protocol, &body.user, body.password.as_deref()) {
        Ok(c) => HttpResponse::Ok().json(ApiResponse::ok(as_json(c))),
        Err(e) => HttpResponse::BadRequest().json(ApiResponse::<()>::error(&e)),
    }
}
/// `DELETE /api/remote/consoles/{id}` — remove a registered console (admin).
#[actix_web::delete("/api/remote/consoles/{id}")]
async fn delete_console(req: HttpRequest, id: web::Path<String>) -> HttpResponse {
    if !may_manage_console(&req) {
        return HttpResponse::Forbidden().finish();
    }
    match super::vnc_registry::delete(&id) {
        Ok(()) => {
            super::vnc_access::forget(&id);
            super::console_share::forget_console(&id);
            HttpResponse::Ok().json(ApiResponse::ok(()))
        }
        Err(e) => HttpResponse::BadRequest().json(ApiResponse::<()>::error(&e)),
    }
}
/// `GET /api/remote/consoles/{key}/access` — who may open this console (admin).
#[actix_web::get("/api/remote/consoles/{key}/access")]
async fn get_access(req: HttpRequest, key: web::Path<String>) -> HttpResponse {
    if !is_admin(&req) {
        return HttpResponse::Forbidden().finish();
    }
    HttpResponse::Ok().json(ApiResponse::ok(super::vnc_access::allowed_users(&key)))
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct AccessBody {
    users: Vec<String>,
}
/// `PUT /api/remote/consoles/{key}/access` — set who may open this console (admin).
#[actix_web::put("/api/remote/consoles/{key}/access")]
async fn set_access(req: HttpRequest, key: web::Path<String>, body: web::Json<AccessBody>) -> HttpResponse {
    if !is_admin(&req) {
        return HttpResponse::Forbidden().finish();
    }
    // Only a real console key may carry a grant.
    if port_for(&key).is_none() && super::vnc_registry::find(&key).is_none() {
        return HttpResponse::NotFound().finish();
    }
    match super::vnc_access::set(&key, body.users.clone()) {
        Ok(()) => HttpResponse::Ok().json(ApiResponse::ok(())),
        Err(e) => HttpResponse::BadRequest().json(ApiResponse::<()>::error(&e)),
    }
}
pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(list_consoles)
        .service(create_console)
        .service(update_console)
        .service(delete_console)
        .service(get_access)
        .service(set_access);
}
