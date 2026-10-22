use super::*;

#[derive(Deserialize)]
pub struct CreateShareRequest {
    /// Lifetime, at most 90 days; ignored for a permanent link.
    #[serde(default)]
    pub expires_in_secs: u64,
    /// Lasts until revoked.
    #[serde(default)]
    pub permanent: bool,
    /// Empty or absent means anyone with the link gets straight in.
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default = "default_read_only")]
    pub read_only: bool,
    /// Off unless asked for.
    #[serde(default)]
    pub files: ShareFiles,
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
    if let Some(refused) = not_granted(&owner, true, body.files != ShareFiles::None) {
        return refused;
    }
    let (label, via_server) = {
        let map = store.lock().unwrap();
        match map.get(&session_id) {
            Some(s)
                if s.owned_by(&owner.id)
                    && super::super::remote_access::allowed(s)
                    && super::super::remote_access::member_may_use(&owner, s) =>
            {
                let via = s.remote_binding.as_ref().map(|b| b.server_id().to_string()).unwrap_or_default();
                (s.label.clone(), via)
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
    let target = LinkTarget { session_id, label, via_server, ..LinkTarget::default() };
    publish(&req, &body, owner, target, &users, &sessions, &shares)
}

/// Why a member may not make this link, in their words; `None` when they may.
pub(super) fn not_granted(owner: &NabimanUser, session: bool, files: bool) -> Option<HttpResponse> {
    if crate::member_servers::may_share(owner, session, files) {
        return None;
    }
    let what = if session && !owner.remote_permissions.share_sessions { "세션 공유" } else { "파일 공유" };
    Some(no_store(HttpResponse::Forbidden().json(ApiResponse::<()>::error(&format!(
        "{what} 권한이 없습니다. 관리자에게 권한을 요청하세요."
    )))))
}

/// What a new link opens: a terminal session, or a saved server's files.
#[derive(Default)]
pub(super) struct LinkTarget {
    pub session_id: String,
    pub server_id: String,
    pub server_binding: String,
    pub via_server: String,
    pub label: String,
}

/// Makes the link: checks the request, hashes the password, and publishes it
/// while the owner's account generation still holds.
pub(super) fn publish(
    req: &HttpRequest,
    body: &CreateShareRequest,
    owner: NabimanUser,
    target: LinkTarget,
    users: &web::Data<UserStore>,
    sessions: &web::Data<crate::jwt_sessions::SessionStore>,
    shares: &web::Data<ShareStore>,
) -> HttpResponse {
    if !body.permanent && (body.expires_in_secs == 0 || body.expires_in_secs > MAX_EXPIRY_SECS) {
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error(
            "Share expiry must be between 1 second and 90 days, or permanent",
        ));
    }
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

    let days = (!body.permanent).then(|| body.expires_in_secs as f64 / 86400.0);
    if let Err(why) = crate::member_servers::share_terms(
        &owner,
        days,
        password_hash.is_some(),
        body.read_only,
        body.files == ShareFiles::Write,
    ) {
        return no_store(HttpResponse::BadRequest().json(ApiResponse::<()>::error(&why)));
    }
    let token = match generate_token() {
        Ok(t) => t,
        Err(_) => return storage_error(),
    };
    let link = ShareLink {
        token: token.clone(),
        session_id: target.session_id,
        owner: owner.username.clone(),
        owner_id: owner.id.clone(),
        owner_authorization_version: owner.share_authorization_version.clone(),
        label: target.label,
        created_unix: now_unix(),
        expires_unix: if body.permanent { PERMANENT_UNIX } else { now_unix() + body.expires_in_secs },
        password_hash,
        read_only: body.read_only,
        last_used_unix: 0,
        needs_password: false,
        alive: true,
        usable: true,
        files: body.files,
        server_id: target.server_id,
        server_binding: target.server_binding,
        via_server: target.via_server,
    };

    let Some(grant) = crate::stream_access::ConnectionGrant::from_request(
        req,
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
            && super::owner_may_share(u, &link)
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
            let alive = if l.files_only() {
                super::files_link_alive(&l)
            } else {
                sessions
                    .get(&l.session_id)
                    .map(|s| {
                        s.owned_by(&l.owner_id)
                            && super::super::remote_access::allowed(s)
                            && tmux_session_exists(&s.tmux_name)
                    })
                    .unwrap_or(false)
            };
            ShareLink {
                alive,
                usable: alive
                    && super::owner_may_share(&owner, &l)
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
