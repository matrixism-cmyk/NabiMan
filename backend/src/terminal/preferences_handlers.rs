use super::*;

pub(super) async fn get_settings(
    req: HttpRequest,
    preferences: web::Data<SettingsStore>,
) -> HttpResponse {
    let username = extract_username_header(&req);
    match preferences.lock().unwrap().get(&username) {
        Ok(settings) => HttpResponse::Ok().json(crate::models::ApiResponse::ok(settings)),
        Err(_) => HttpResponse::ServiceUnavailable().json(crate::models::ApiResponse::<()>::error(
            "Terminal preferences unavailable; storage recovery required",
        )),
    }
}

pub(super) async fn put_settings(
    body: web::Json<TerminalSettings>,
    req: HttpRequest,
    preferences: web::Data<SettingsStore>,
) -> HttpResponse {
    let username = extract_username_header(&req);
    let mut settings = body.into_inner();
    if settings.scrollback_lines < 100 || settings.scrollback_lines > MAX_SCROLLBACK {
        return HttpResponse::Ok().json(crate::models::ApiResponse::<()>::error(
            "scrollback_lines must be between 100 and 200000",
        ));
    }
    if settings.idle_timeout_secs > MAX_TIMEOUT_SECS {
        return HttpResponse::Ok().json(crate::models::ApiResponse::<()>::error(
            "idle_timeout_secs must be 0 (permanent) or at most 30 days",
        ));
    }
    settings.keepalive_interval_secs = settings.keepalive_interval_secs.clamp(5, 3600);
    settings.font_size = settings.font_size.clamp(8, 28);
    if settings.keepalive {
        settings.idle_timeout_secs = 0;
    }
    match preferences
        .lock()
        .unwrap()
        .save(&username, settings.clone())
    {
        Ok(()) => HttpResponse::Ok().json(crate::models::ApiResponse::ok(settings)),
        Err(_) => HttpResponse::ServiceUnavailable().json(crate::models::ApiResponse::<()>::error(
            "Terminal preference save could not be confirmed; verify storage before retrying",
        )),
    }
}
