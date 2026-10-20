use super::*;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShareAuthRequest {
    pub password: String,
    #[serde(default)]
    pub query: String,
}
#[derive(Serialize)]
pub(super) struct TicketResponse {
    ticket: String,
    expires_in_seconds: u64,
    /// Present when the link grants files: the key for the file panel.
    #[serde(skip_serializing_if = "Option::is_none")]
    files: Option<FilesGrant>,
}
#[derive(Serialize)]
pub(super) struct FilesGrant {
    key: String,
    mode: ShareFiles,
}
pub(super) fn admission_error(status: actix_web::http::StatusCode, message: &str) -> HttpResponse {
    no_store(HttpResponse::build(status).json(ApiResponse::<()>::error(message)))
}
pub(super) fn limited() -> HttpResponse {
    no_store(
        HttpResponse::TooManyRequests()
            .insert_header(("Retry-After", "60"))
            .json(ApiResponse::<()>::error(
                "Share admission temporarily limited",
            )),
    )
}
pub(super) async fn share_auth(
    path: web::Path<String>,
    body: web::Json<ShareAuthRequest>,
    req: HttpRequest,
    tickets: web::Data<share_ticket::ShareTickets>,
    shares: web::Data<ShareStore>,
    users: web::Data<UserStore>,
) -> HttpResponse {
    use actix_web::http::StatusCode;
    if !share_ticket::valid_query(&body.query) || body.password.len() > 72 {
        return admission_error(
            StatusCode::BAD_REQUEST,
            "Invalid share admission parameters",
        );
    }
    let origin = match crate::terminal_tickets::origin(&req) {
        Ok(origin) => origin,
        Err(()) => return admission_error(StatusCode::BAD_REQUEST, "Invalid share origin"),
    };
    let token = path.into_inner();
    // Do not trust client-supplied Forwarded/X-Forwarded-For as a rate-limit identity.
    let peer = req
        .peer_addr()
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|| "unknown".into());
    let allowed = match tickets.attempts.lock() {
        Ok(mut attempts) => attempts.allow(&token, &peer, std::time::Instant::now()),
        Err(_) => return storage_error(),
    };
    if !allowed {
        return limited();
    }
    if !shares.lock().is_ok_and(|r| r.ready()) {
        return storage_error();
    }
    let Some(link) = live_link(&shares, &users, &token) else {
        return admission_error(StatusCode::FORBIDDEN, "Share authentication denied");
    };
    if !tickets.has_room(&token) {
        return limited();
    }
    if let Some(hash) = link.password_hash.clone() {
        let Ok(permit) = tickets.verification.clone().try_acquire_owned() else {
            return limited();
        };
        let password = body.password.clone();
        let verified = web::block(move || {
            // The blocking task owns the permit, even if the HTTP future is cancelled.
            let _permit = permit;
            bcrypt::verify(&password, &hash).unwrap_or(false)
        })
        .await;
        match verified {
            Ok(true) => {}
            Ok(false) => {
                return admission_error(StatusCode::FORBIDDEN, "Share authentication denied")
            }
            Err(_) => return storage_error(),
        }
    }
    let Some(current) = live_link(&shares, &users, &token).filter(|l| l.same_authorization(&link))
    else {
        return admission_error(StatusCode::FORBIDDEN, "Share authentication denied");
    };
    let lifetime = share_ticket::TTL.min(std::time::Duration::from_secs(
        current.expires_unix.saturating_sub(now_unix()),
    ));
    if lifetime.is_zero() {
        return admission_error(StatusCode::FORBIDDEN, "Share authentication denied");
    }
    let issued = match tickets.tickets.lock() {
        Ok(mut store) => store.issue(
            current,
            body.query.clone(),
            origin,
            std::time::Instant::now(),
            lifetime,
        ),
        Err(_) => return storage_error(),
    };
    match issued {
        Ok(ticket) => {
            // The same admission that opens the terminal opens its files.
            let files = super::super::sftp::share_keys::issue(&link)
                .map(|key| FilesGrant { key, mode: link.files });
            no_store(HttpResponse::Ok().json(ApiResponse::ok(TicketResponse {
                ticket,
                expires_in_seconds: lifetime.as_secs(),
                files,
            })))
        }
        Err(share_ticket::IssueError::Capacity) => limited(),
        Err(share_ticket::IssueError::Entropy) => storage_error(),
    }
}

/// The visitor's terminal socket. Everything a link needs to be safe is checked
/// here: it exists, it has not expired, the password ticket holds up, and the
/// session behind it is still running.
pub(super) async fn share_ws(
    path: web::Path<String>,
    req: HttpRequest,
    stream: web::Payload,
    tickets: web::Data<share_ticket::ShareTickets>,
    store: web::Data<PtyStore>,
    shares: web::Data<ShareStore>,
    users: web::Data<UserStore>,
) -> Result<HttpResponse, actix_web::Error> {
    use actix_web::http::StatusCode;
    let token = path.into_inner();
    let Some(snapshot) = share_ticket::redeem(&req, &token, &tickets) else {
        return Ok(admission_error(
            StatusCode::UNAUTHORIZED,
            "Share admission required",
        ));
    };
    // Redemption is consumed even if this is a plain HTTP request, never a shell.
    ws::handshake(&req)?;
    if req.query_string().starts_with("purpose=") {
        return Ok(admission_error(StatusCode::BAD_REQUEST, "A dictation ticket does not open the terminal"));
    }
    if !shares.lock().is_ok_and(|r| r.ready()) {
        return Ok(storage_error());
    }
    let Some(link) = live_link(&shares, &users, &token).filter(|l| l.same_authorization(&snapshot))
    else {
        return Ok(admission_error(
            StatusCode::FORBIDDEN,
            "Share authentication denied",
        ));
    };
    let query = req.query_string();

    let tmux_name = {
        let map = store.lock().unwrap();
        map.get(&link.session_id)
            .filter(|s| s.owned_by(&link.owner_id))
            .map(|s| s.tmux_name.clone())
    };
    let tmux_name = match tmux_name.filter(|n| tmux_session_exists(n)) {
        Some(n) => n,
        None => {
            return Ok(ws::start(
                super::super::pty::NoticeSocket::new(
                    "공유된 세션이 종료되었습니다 / the shared session has ended",
                ),
                &req,
                stream,
            )?)
        }
    };

    let share_grant = ShareGrant {
        link: link.clone(),
        tmux_name: tmux_name.clone(),
        shares: shares.get_ref().clone(),
        users: users.get_ref().clone(),
        sessions: store.get_ref().clone(),
    };
    if !share_grant.valid() {
        return Ok(HttpResponse::Forbidden().finish());
    }
    let Some(lease) = share_ticket::reserve(&tickets, &token) else {
        return Ok(limited());
    };

    // Attach at the pane's own size: opening the pseudo-terminal at the
    // visitor's size would reshape the session for the person working in it.
    let size = client_size(&tmux_name).unwrap_or_else(|| parse_size(query));
    let (fd, pid) = match attach_tmux_shared(&tmux_name, size, link.read_only) {
        Ok(r) => r,
        Err(e) => return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()>::error(&e))),
    };

    {
        // Activity is informational and does not rewrite the security registry on every attach.
        if let Ok(mut registry) = shares.lock() {
            registry.touch(&token);
        }
        let mut map = store.lock().unwrap();
        if let Some(s) = map.get_mut(&link.session_id) {
            s.ws_count += 1;
            s.last_active_unix = now_unix();
        }
    }

    let file = unsafe { std::fs::File::from_raw_fd(fd) };
    ws::start(
        WsBridge {
            session_id: link.session_id,
            tmux_name,
            store: store.get_ref().clone(),
            master_fd: fd,
            master_file: Some(file),
            child_pid: pid,
            ended: false,
            read_only: link.read_only,
            grant: None,
            share_grant: Some(share_grant),
            share_lease: Some(lease),
            mirror_size: true,
            last_size: Some(size),
            history: None,
            client_tty: super::super::pty::slave_tty(fd),
        },
        &req,
        stream,
    )
}
