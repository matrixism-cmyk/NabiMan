//! Adding, changing and removing saved servers. A remote-only member's own
//! servers are kept to a password and a public address (see `member_servers`).
use super::*;

/// The signed-in account, which a server may belong to.
fn actor(
    req: &actix_web::HttpRequest,
    store: &web::Data<crate::users::UserStore>,
) -> Result<crate::models::NabimanUser, HttpResponse> {
    crate::membership::actor(req, store)
}

/// A member's server: theirs, logged into with a password, and at an address
/// they may reach (see `member_servers`).
fn member_check(server: &mut RemoteServer) -> Result<(), String> {
    if !server.member_owned() {
        return Ok(());
    }
    server.auth_method = "password".into();
    crate::member_servers::dial_host(server, server.port).map(drop)
}

pub(super) async fn add_server(
    req: actix_web::HttpRequest,
    store: web::Data<crate::users::UserStore>,
    body: web::Json<AddRemoteServerRequest>,
) -> HttpResponse {
    let actor = match actor(&req, &store) {
        Ok(u) => u,
        Err(r) => return r,
    };
    if !validate_host(&body.host) {
        return HttpResponse::Ok().json(ApiResponse::<RemoteServer>::error("Invalid hostname"));
    }
    let user = body.user.as_deref().unwrap_or("root");
    let terminal = body.terminal.as_deref().unwrap_or("ssh");
    let port = body.port.unwrap_or_else(|| protocols::default_port(terminal));
    if port == 0 {
        return HttpResponse::Ok().json(ApiResponse::<RemoteServer>::error("Invalid port"));
    }

    // A pasted password can carry a stray newline; it is never part of the
    // secret and would be rejected by the server.
    let password = body
        .password
        .as_deref()
        .unwrap_or("")
        .trim_matches(['\r', '\n']);
    if password.len() > 512 {
        return HttpResponse::Ok().json(ApiResponse::<RemoteServer>::error("Password too long"));
    }
    let password_enc = if password.is_empty() {
        String::new()
    } else {
        match secret_store::encrypt(password) {
            Ok(enc) => enc,
            Err(e) => return HttpResponse::Ok().json(ApiResponse::<RemoteServer>::error(&e)),
        }
    };
    // A saved password implies password auth unless the caller says otherwise.
    let auth_method = body.auth_method.clone().unwrap_or_else(|| {
        if password.is_empty() {
            "key".into()
        } else {
            "password".into()
        }
    });

    let mut server = RemoteServer {
        id: generate_id(),
        name: body.name.clone(),
        host: body.host.clone(),
        port,
        user: user.to_string(),
        auth_method,
        tags: body.tags.clone().unwrap_or_default(),
        memo: body.memo.clone().unwrap_or_default(),
        created_at: now_iso(),
        last_checked: String::new(),
        status: "unknown".into(),
        password_enc,
        has_password: !password.is_empty(),
        host_trust_version: uuid::Uuid::new_v4().to_string(),
        host_trust: None,
        terminal: "ssh".into(),
        files: "sftp".into(),
        ftp_port: 21,
        tls_fingerprint: String::new(),
        ftps_implicit: false,
        owner_id: if crate::member_servers::is_member(&actor) { actor.id.clone() } else { String::new() },
    };
    protocols::apply(&mut server, &protocols::Change {
        terminal: body.terminal.as_deref(),
        files: body.files.as_deref(),
        ftp_port: body.ftp_port,
        tls_fingerprint: body.tls_fingerprint.as_deref(),
        ftps_implicit: body.ftps_implicit,
    });
    if let Err(e) = protocols::check(&server) {
        return HttpResponse::Ok().json(ApiResponse::<RemoteServer>::error(e));
    }
    // Keys are an SSH thing; Telnet and FTP log in with a password.
    if !server.uses_ssh() {
        server.auth_method = "password".into();
    }
    if let Err(e) = member_check(&mut server) {
        return HttpResponse::Ok().json(ApiResponse::<RemoteServer>::error(&e));
    }

    let _guard = SERVER_WRITES.lock().unwrap_or_else(|e| e.into_inner());
    let mut servers = match load_servers() {
        Ok(servers) => servers,
        Err(error) => return storage_error(error),
    };
    servers.push(server.clone());
    if let Err(e) = save_servers(&mut servers) {
        return storage_error(e);
    }
    HttpResponse::Ok().json(ApiResponse::ok(public(server)))
}

pub(super) async fn update_server(
    req: actix_web::HttpRequest,
    store: web::Data<crate::users::UserStore>,
    path: web::Path<String>,
    body: web::Json<UpdateRemoteServerRequest>,
) -> HttpResponse {
    let actor = match actor(&req, &store) {
        Ok(u) => u,
        Err(r) => return r,
    };
    let server_id = path.into_inner();
    let _guard = SERVER_WRITES.lock().unwrap_or_else(|e| e.into_inner());
    let mut servers = match load_servers() {
        Ok(servers) => servers,
        Err(error) => return storage_error(error),
    };
    let srv = match servers
        .iter_mut()
        .find(|s| s.id == server_id && crate::member_servers::manageable(&actor, s))
    {
        Some(s) => s,
        None => {
            return HttpResponse::Ok().json(ApiResponse::<RemoteServer>::error("Server not found"))
        }
    };

    if let Some(ref name) = body.name {
        srv.name = name.clone();
    }
    if let Some(ref host) = body.host {
        if !validate_host(host) {
            return HttpResponse::Ok().json(ApiResponse::<RemoteServer>::error("Invalid hostname"));
        }
        if srv.host != *host {
            srv.host_trust = None;
            srv.host_trust_version = uuid::Uuid::new_v4().to_string();
            srv.status = "unknown".into();
            srv.last_checked.clear();
        }
        srv.host = host.clone();
    }
    if let Some(port) = body.port {
        if port == 0 {
            return HttpResponse::Ok().json(ApiResponse::<RemoteServer>::error("Invalid port"));
        }
        if srv.port != port {
            srv.host_trust = None;
            srv.host_trust_version = uuid::Uuid::new_v4().to_string();
            srv.status = "unknown".into();
            srv.last_checked.clear();
        }
        srv.port = port;
    }
    if let Some(ref user) = body.user {
        srv.user = user.clone();
    }
    let reached_differently = protocols::apply(srv, &protocols::Change {
        terminal: body.terminal.as_deref(),
        files: body.files.as_deref(),
        ftp_port: body.ftp_port,
        tls_fingerprint: body.tls_fingerprint.as_deref(),
        ftps_implicit: body.ftps_implicit,
    });
    if reached_differently {
        srv.status = "unknown".into();
        srv.last_checked.clear();
    }
    if let Err(e) = protocols::check(srv) {
        return HttpResponse::Ok().json(ApiResponse::<RemoteServer>::error(e));
    }
    if let Some(ref method) = body.auth_method {
        srv.auth_method = method.clone();
    }
    if let Some(ref tags) = body.tags {
        srv.tags = tags.clone();
    }
    if let Some(ref memo) = body.memo {
        srv.memo = memo.clone();
    }
    if let Some(password) = body
        .password
        .as_deref()
        .map(|p| p.trim_matches(['\r', '\n']))
    {
        if password.is_empty() {
            srv.password_enc.clear();
        } else if password.len() > 512 {
            return HttpResponse::Ok()
                .json(ApiResponse::<RemoteServer>::error("Password too long"));
        } else {
            match secret_store::encrypt(password) {
                Ok(enc) => srv.password_enc = enc,
                Err(e) => return HttpResponse::Ok().json(ApiResponse::<RemoteServer>::error(&e)),
            }
        }
        srv.has_password = !srv.password_enc.is_empty();
    }
    if let Err(e) = member_check(srv) {
        return HttpResponse::Ok().json(ApiResponse::<RemoteServer>::error(&e));
    }

    let updated = public(srv.clone());
    if let Err(e) = save_servers(&mut servers) {
        return storage_error(e);
    }
    HttpResponse::Ok().json(ApiResponse::ok(updated))
}

pub(super) async fn delete_server(
    req: actix_web::HttpRequest,
    store: web::Data<crate::users::UserStore>,
    path: web::Path<String>,
) -> HttpResponse {
    let actor = match actor(&req, &store) {
        Ok(u) => u,
        Err(r) => return r,
    };
    let server_id = path.into_inner();
    let _guard = SERVER_WRITES.lock().unwrap_or_else(|e| e.into_inner());
    let mut servers = match load_servers() {
        Ok(servers) => servers,
        Err(error) => return storage_error(error),
    };
    let before = servers.len();
    servers.retain(|s| s.id != server_id || !crate::member_servers::manageable(&actor, s));
    if servers.len() == before {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Server not found"));
    }
    if let Err(e) = save_servers(&mut servers) {
        return storage_error(e);
    }
    HttpResponse::Ok().json(ApiResponse::ok("Server deleted".to_string()))
}
