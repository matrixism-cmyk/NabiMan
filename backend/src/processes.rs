mod identity;
mod signal;
#[cfg(test)]
mod tests;
use crate::models::{ApiResponse, KillProcessRequest, ProcessInfo};
use actix_web::{web, HttpRequest, HttpResponse};
fn admin(req: &HttpRequest, users: &crate::users::UserStore) -> bool {
    let id = crate::auth::verified_user_id(req);
    users.lock().ok().is_some_and(|u| {
        u.iter()
            .any(|u| Some(u.id.as_str()) == id.as_deref() && u.active() && u.super_admin())
    })
}
fn list() -> Result<Vec<ProcessInfo>, String> {
    let result = crate::bounded_command::run(
        std::path::Path::new("/usr/bin/ps"),
        &["-eo", "pid=,pcpu=,pmem=,vsz=,rss=", "--sort=-pcpu"],
        None,
        std::time::Duration::from_secs(5),
    )
    .map_err(|_| "Process collector unavailable")?;
    if !result.status.success() {
        return Err("Process collector failed".into());
    }
    let protected = identity::protected()?;
    let available = signal::available();
    let hz = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
    if hz <= 0 {
        return Err("Process clock unavailable".into());
    }
    let boot = sysinfo::System::boot_time();
    if boot == 0 {
        return Err("Boot time unavailable".into());
    }
    let mut rows = vec![];
    for line in result.out.lines().take(200) {
        let p: Vec<_> = line.split_whitespace().collect();
        if p.len() != 5 {
            return Err("Malformed process inventory".into());
        }
        let pid = p[0].parse::<u32>().map_err(|_| "Invalid process PID")?;
        let Ok(current) = identity::inspect(pid) else {
            continue;
        };
        let cpu = p[1].parse::<f32>().map_err(|_| "Invalid CPU sample")?;
        let memory = p[2].parse::<f32>().map_err(|_| "Invalid memory sample")?;
        if !cpu.is_finite() || !memory.is_finite() {
            return Err("Invalid process sample".into());
        }
        rows.push(ProcessInfo {
            pid,
            user: current.uid.to_string(),
            cpu,
            memory,
            vsz: p[3].parse().map_err(|_| "Invalid virtual size")?,
            rss: p[4].parse().map_err(|_| "Invalid resident size")?,
            command: current.command,
            started: chrono::DateTime::from_timestamp((boot + current.start / hz as u64) as i64, 0)
                .ok_or("Invalid process start time")?
                .to_rfc3339(),
            identity: current.value,
            can_signal: available && !protected.contains(&pid),
        });
    }
    Ok(rows)
}
async fn list_processes() -> HttpResponse {
    match web::block(list).await {
        Ok(Ok(rows)) => HttpResponse::Ok()
            .insert_header(("Cache-Control", "no-store"))
            .json(ApiResponse::ok(rows)),
        _ => HttpResponse::ServiceUnavailable().json(ApiResponse::<()>::error(
            "Process inventory unavailable; no empty-state fallback",
        )),
    }
}
async fn options(req: HttpRequest, users: web::Data<crate::users::UserStore>) -> HttpResponse {
    HttpResponse::Ok().insert_header(("Cache-Control","no-store")).json(ApiResponse::ok(serde_json::json!({"can_signal":admin(&req,&users)&&signal::available(),"identity":"pidfd plus boot/start/UID/command snapshot","limit":200})))
}
async fn inspect(path: web::Path<u32>) -> HttpResponse {
    let pid = path.into_inner();
    match web::block(move || identity::inspect(pid)).await {
        Ok(Ok(i)) => HttpResponse::Ok()
            .insert_header(("Cache-Control", "no-store"))
            .json(ApiResponse::ok(
                serde_json::json!({"pid":pid,"identity":i.value,"command":i.command,"uid":i.uid}),
            )),
        _ => HttpResponse::Conflict()
            .json(ApiResponse::<()>::error("Process is no longer available")),
    }
}
async fn kill_process(
    req: HttpRequest,
    users: web::Data<crate::users::UserStore>,
    body: web::Json<KillProcessRequest>,
) -> HttpResponse {
    if !admin(&req, &users) {
        return HttpResponse::Forbidden().json(ApiResponse::<()>::error(
            "Highest administrator required for local process signals",
        ));
    }
    let b = body.into_inner();
    if b.expected_identity.len() != 64
        || !b.expected_identity.bytes().all(|c| c.is_ascii_hexdigit())
    {
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error(
            "A reviewed process identity is required",
        ));
    }
    let Some(sessions) = req.app_data::<web::Data<crate::jwt_sessions::SessionStore>>() else {
        return HttpResponse::Forbidden().finish();
    };
    let Some(grant) = crate::terminal_tickets::ConnectionGrant::from_request(
        &req,
        sessions.get_ref().clone(),
        users.get_ref().clone(),
    ) else {
        return HttpResponse::Forbidden().finish();
    };
    let expires = crate::auth::verified_claims(&req)
        .map(|c| c.exp)
        .unwrap_or(0);
    let owner = grant.user_id.clone();
    let store = users.get_ref().clone();
    match web::block(move || {
        signal::send(
            b.pid,
            &b.expected_identity,
            b.signal.as_deref().unwrap_or("TERM"),
            move || {
                chrono::Utc::now().timestamp() < expires as i64
                    && grant.valid_for_method("POST")
                    && store.lock().ok().is_some_and(|u| {
                        u.iter()
                            .any(|u| u.id == owner && u.super_admin() && u.active())
                    })
            },
        )
    })
    .await
    {
        Ok(Ok(receipt)) => HttpResponse::Ok()
            .insert_header(("Cache-Control", "no-store"))
            .json(ApiResponse::ok(receipt)),
        Ok(Err(message)) => HttpResponse::Conflict().json(ApiResponse::<()>::error(&message)),
        Err(_) => HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
            "Signal result unavailable; outcome unknown. Inspect before retrying",
        )),
    }
}
pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/processes")
            .route("", web::get().to(list_processes))
            .route("/options", web::get().to(options))
            .route("/inspect/{pid}", web::get().to(inspect))
            .route("/kill", web::post().to(kill_process)),
    );
}
