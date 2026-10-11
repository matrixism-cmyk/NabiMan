use super::*;

#[derive(Deserialize)]
pub struct CreateShareRequest {
    /// Finite lifetime, at most 30 days.
    pub expires_in_secs: u64,
    /// Empty or absent means anyone with the link gets straight in.
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default = "default_read_only")]
    pub read_only: bool,
}
pub(super) fn default_read_only() -> bool {
    true
}

pub(super) fn generate_token() -> io::Result<String> {
    let mut bytes = [0u8; 32];
    OsRng.try_fill_bytes(&mut bytes).map_err(io::Error::other)?;
    Ok(hex::encode(bytes))
}

pub(super) async fn create_share(
    path: web::Path<String>,
    body: web::Json<CreateShareRequest>,
    store: web::Data<PtyStore>,
    shares: web::Data<ShareStore>,
    users: web::Data<UserStore>,
    sessions: web::Data<crate::jwt_sessions::SessionStore>,
    req: HttpRequest,
) -> HttpResponse {
    let session_id = path.into_inner();
    let Some(owner) = current_owner(&req, &users) else {
        return HttpResponse::Forbidden().finish();
    };
    let username = owner.username.clone();
    if body.expires_in_secs == 0 || body.expires_in_secs > MAX_EXPIRY_SECS {
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error(
            "Share expiry must be between 1 second and 30 days",
        ));
    }

    let label = {
        let map = store.lock().unwrap();
        match map.get(&session_id) {
            Some(s) if s.owned_by(&owner.id) && super::super::remote_access::allowed(s) => {
                s.label.clone()
            }
            Some(_) => {
                return HttpResponse::Ok()
                    .insert_header((actix_web::http::header::CACHE_CONTROL, "no-store"))
                    .json(ApiResponse::<()>::error("Not owner"))
            }
            None => {
                return HttpResponse::Ok()
                    .insert_header((actix_web::http::header::CACHE_CONTROL, "no-store"))
                    .json(ApiResponse::<()>::error("Session not found"))
            }
        }
    };

    if body
        .password
        .as_deref()
        .is_some_and(|p| p.trim().len() > 72)
    {
        return no_store(HttpResponse::BadRequest().json(ApiResponse::<()>::error(
            "Share password must be at most 72 UTF-8 bytes",
        )));
    }
    let password_hash = match body
        .password
        .as_deref()
        .map(str::trim)
        .filter(|p| !p.is_empty())
    {
        Some(pw) => match bcrypt::hash(pw, 10) {
            Ok(h) => Some(h),
            Err(_) => {
                return HttpResponse::Ok()
                    .insert_header((actix_web::http::header::CACHE_CONTROL, "no-store"))
                    .json(ApiResponse::<()>::error("Cannot hash password"))
            }
        },
        None => None,
    };

    let token = match generate_token() {
        Ok(t) => t,
        Err(_) => return storage_error(),
    };
    let link = ShareLink {
        token: token.clone(),
        session_id,
        owner: username,
        owner_id: owner.id.clone(),
        owner_authorization_version: owner.share_authorization_version.clone(),
        label,
        created_unix: now_unix(),
        expires_unix: now_unix() + body.expires_in_secs,
        password_hash,
        read_only: body.read_only,
        last_used_unix: 0,
        needs_password: false,
        alive: true,
        usable: true,
    };

    let Some(grant) = crate::terminal_tickets::ConnectionGrant::from_request(
        &req,
        sessions.get_ref().clone(),
        users.get_ref().clone(),
    ) else {
        return HttpResponse::Unauthorized().finish();
    };
    if !grant.valid_for_method(req.method().as_str()) {
        return HttpResponse::Forbidden().finish();
    }
    let Ok(mut registry) = shares.lock() else {
        return storage_error();
    };
    // Hold the account generation through publication to prevent a stale create
    // from becoming usable after a concurrent security change.
    let Ok(current_users) = users.lock() else {
        return storage_error();
    };
    if !current_users.iter().any(|u| {
        u.id == owner.id
            && u.super_admin()
            && !u.share_authorization_version.is_empty()
            && u.share_authorization_version == owner.share_authorization_version
    }) {
        return HttpResponse::Forbidden().finish();
    }
    if registry
        .update(|map| {
            map.retain(|_, l| !expired(l));
            if map.contains_key(&token) {
                return Err(io::Error::other("Share token collision"));
            }
            map.insert(token, link.clone());
            Ok(())
        })
        .is_err()
    {
        return storage_error();
    }
    no_store(
        HttpResponse::Ok()
            .insert_header((actix_web::http::header::CACHE_CONTROL, "no-store"))
            .json(ApiResponse::ok(public_view(link))),
    )
}

pub(super) async fn list_shares(
    store: web::Data<PtyStore>,
    shares: web::Data<ShareStore>,
    users: web::Data<UserStore>,
    req: HttpRequest,
) -> HttpResponse {
    let Some(owner) = current_owner(&req, &users) else {
        return HttpResponse::Forbidden().finish();
    };
    let records = match shares.lock().ok().and_then(|r| r.all().ok()) {
        Some(r) => r,
        None => return storage_error(),
    };
    let sessions = store.lock().unwrap();
    let mut list: Vec<ShareLink> = records
        .into_iter()
        .filter(|l| l.owner_id == owner.id || (l.owner_id.is_empty() && l.owner == owner.username))
        .map(|l| {
            let alive = sessions
                .get(&l.session_id)
                .map(|s| {
                    s.owned_by(&l.owner_id)
                        && super::super::remote_access::allowed(s)
                        && tmux_session_exists(&s.tmux_name)
                })
                .unwrap_or(false);
            ShareLink {
                alive,
                usable: alive
                    && l.owner == owner.username
                    && !expired(&l)
                    && !l.owner_id.is_empty()
                    && !l.owner_authorization_version.is_empty()
                    && l.owner_authorization_version == owner.share_authorization_version,
                ..public_view(l)
            }
        })
        .collect();
    list.sort_by_key(|l| std::cmp::Reverse(l.created_unix));
    no_store(
        HttpResponse::Ok()
            .insert_header((actix_web::http::header::CACHE_CONTROL, "no-store"))
            .json(ApiResponse::ok(list)),
    )
}

pub(super) async fn revoke_share(
    path: web::Path<String>,
    shares: web::Data<ShareStore>,
    users: web::Data<UserStore>,
    req: HttpRequest,
) -> HttpResponse {
    let token = path.into_inner();
    let Some(owner) = current_owner(&req, &users) else {
        return HttpResponse::Forbidden().finish();
    };
    let Ok(mut registry) = shares.lock() else {
        return storage_error();
    };
    if !registry.ready() {
        return storage_error();
    }
    match registry.get(&token) {
        Some(link)
            if link.owner_id == owner.id
                || (link.owner_id.is_empty() && link.owner == owner.username) =>
        {
            if registry
                .update(|map| {
                    map.remove(&token);
                    Ok(())
                })
                .is_err()
            {
                return storage_error();
            }
            no_store(
                HttpResponse::Ok()
                    .insert_header((actix_web::http::header::CACHE_CONTROL, "no-store"))
                    .json(ApiResponse::ok("Link revoked")),
            )
        }
        _ => HttpResponse::Ok()
            .insert_header((actix_web::http::header::CACHE_CONTROL, "no-store"))
            .json(ApiResponse::<()>::error("Link not found")),
    }
}

// --- Visitor-side API (no account) ---------------------------------------
