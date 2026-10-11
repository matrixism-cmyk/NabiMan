use super::*;

pub(super) async fn get_me(
    req: actix_web::HttpRequest,
    store: web::Data<UserStore>,
    sessions: web::Data<crate::jwt_sessions::SessionStore>,
) -> HttpResponse {
    let username = match crate::auth::extract_username(&req) {
        Some(username) => username,
        None => return HttpResponse::Unauthorized().json(ApiResponse::<()>::error("Unauthorized")),
    };
    let users = store.lock().unwrap();
    match users.iter().find(|u| {
        u.username == username
            && Some(u.id.as_str()) == crate::auth::verified_user_id(&req).as_deref()
    }) {
        Some(u) => {
            let verified = crate::auth::verified_claims(&req).is_some_and(|c| {
                sessions
                    .lock()
                    .unwrap()
                    .get(&c.sid)
                    .is_some_and(|s| s.user_id == u.id && s.mfa_verified)
            });
            let mut value = serde_json::to_value(u).unwrap();
            value["mfa_blocked"] =
                (!crate::mfa_policy::permitted(u, verified, "GET", "/api/remote-servers")).into();
            HttpResponse::Ok()
                .insert_header(("Cache-Control", "no-store"))
                .json(ApiResponse::ok(value))
        }
        None => HttpResponse::Ok().json(ApiResponse::<()>::error("User not found")),
    }
}
