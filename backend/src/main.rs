mod bounded_command;
mod accounts;
mod alert_rules;
mod api_keys;
mod audit;
mod audit_forwarding;
mod audit_middleware;
mod auth;
mod backup;
mod charts;
mod config_manager;
mod containers;
mod cron;
mod database;
mod diagnostics;
mod disks;
mod dns;
mod file_manager;
mod firewall;
mod ip_block;
mod jwt_sessions;
mod ldap_auth;
mod license;
mod logs;
mod mail;
mod mec;
mod membership;
mod mfa_policy;
mod models;
mod network;
mod notifications;
mod oauth;
mod openapi;
mod packages;
mod private_storage;
mod processes;
mod rate_limit;
mod rbac;
mod remote_host_trust;
mod remote_probe;
mod remote_servers;
mod runtime_identity;
mod scope_access;
mod scope_settings;
mod secret_store;
mod security_headers;
mod server_status;
mod service_control;
mod service_registry;
mod services;
mod session_management;
mod sessions;
mod ssh_auth;
mod ssh_keys;
mod ssl;
mod stream_access;
mod support_report;
mod swap;
mod terminal;
mod terminal_tickets;
mod totp;
mod traffic;
mod updates;
mod users;
mod vhost;

use actix_cors::Cors;
use actix_files as afs;
use actix_web::body::EitherBody;
use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::{web, App, HttpResponse, HttpServer};
use std::future::{ready, Future, Ready};
use std::pin::Pin;
use std::rc::Rc;

// --- Auth Middleware (JWT-based) ---
pub struct AuthCheck {
    secret: auth::JwtSecret,
    sessions: jwt_sessions::SessionStore,
    users: users::UserStore,
}

impl AuthCheck {
    pub fn new(
        secret: auth::JwtSecret,
        sessions: jwt_sessions::SessionStore,
        users: users::UserStore,
    ) -> Self {
        Self {
            secret,
            sessions,
            users,
        }
    }
}

impl<S, B> Transform<S, ServiceRequest> for AuthCheck
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = actix_web::Error;
    type Transform = AuthCheckMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(AuthCheckMiddleware {
            service: Rc::new(service),
            secret: self.secret.clone(),
            sessions: self.sessions.clone(),
            users: self.users.clone(),
        }))
    }
}

pub struct AuthCheckMiddleware<S> {
    service: Rc<S>,
    secret: auth::JwtSecret,
    sessions: jwt_sessions::SessionStore,
    users: users::UserStore,
}

impl<S, B> Service<ServiceRequest> for AuthCheckMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = actix_web::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(
        &self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let denial = match auth::check_auth(&req, &self.secret, &self.sessions, &self.users) {
            auth::AuthDecision::Allow => None,
            auth::AuthDecision::Unauthorized => Some(
                HttpResponse::Unauthorized().json(models::ApiResponse::<()>::error("Unauthorized")),
            ),
            auth::AuthDecision::MfaRequired => Some(
                HttpResponse::Forbidden()
                    .insert_header(("X-Nabiman-MFA-Required", "1"))
                    .json(models::ApiResponse::<()>::error(
                        "소속 보안 정책에 따라 2단계 인증 등록 후 다시 로그인해야 합니다",
                    )),
            ),
            auth::AuthDecision::Forbidden => {
                Some(HttpResponse::Forbidden().json(models::ApiResponse::<()>::error("Forbidden")))
            }
        };
        if let Some(response) = denial {
            return Box::pin(async move { Ok(req.into_response(response).map_into_right_body()) });
        }
        let svc = self.service.clone();
        Box::pin(async move { svc.call(req).await.map(|res| res.map_into_left_body()) })
    }
}

fn build_cors() -> Cors {
    match std::env::var("NABIMAN_CORS_ORIGINS") {
        Ok(origins) if !origins.is_empty() => {
            let mut cors = Cors::default()
                .allow_any_method()
                .expose_headers(["X-Nabiman-MFA-Required"])
                .allowed_headers([
                    "Content-Type",
                    "Authorization",
                    "X-Auth-Token",
                    "X-API-Key",
                    "X-File-Path",
                ])
                .max_age(3600);
            for origin in origins.split(',') {
                cors = cors.allowed_origin(origin.trim());
            }
            cors
        }
        _ => Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .expose_headers(["X-Nabiman-MFA-Required"])
            .allowed_headers([
                "Content-Type",
                "Authorization",
                "X-Auth-Token",
                "X-API-Key",
                "X-File-Path",
            ])
            .max_age(3600),
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    if unsafe { libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    let runtime_identity = web::Data::new(runtime_identity::RuntimeIdentity::capture()?);
    let mec_mode = mec::services::bootstrap::ServiceMode::from_env()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
    // Validate durable storage before starting background collectors or auth writes.
    let owned_mec_db = mec::db::open_owned(mec::db::default_path())?;
    let mec_db = owned_mec_db.handle.clone();
    let port: u16 = std::env::var("NABIMAN_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);
    let static_dir = std::env::var("NABIMAN_STATIC").unwrap_or_else(|_| "./static".to_string());

    let share_store = terminal::share_store::open(
        std::path::PathBuf::from(
            std::env::var("NABIMAN_DATA_DIR").unwrap_or_else(|_| "/var/lib/nabiman".into()),
        )
        .join("terminal_shares.json"),
    )?;
    let terminal_registry = terminal::open_registry(
        std::path::PathBuf::from(
            std::env::var("NABIMAN_DATA_DIR").unwrap_or_else(|_| "/var/lib/nabiman".into()),
        )
        .join("terminal_sessions.json"),
    )?;
    let terminal_preferences = terminal::open_settings_registry(
        std::path::PathBuf::from(
            std::env::var("NABIMAN_DATA_DIR").unwrap_or_else(|_| "/var/lib/nabiman".into()),
        )
        .join("terminal_settings.json"),
    )?;
    let helper_client = web::Data::new(nabiman_server::privileged::client::Client::from_env());
    let password_store = auth::new_password_store();
    let jwt_secret = auth::new_jwt_secret();
    let pw_hash = password_store.lock().unwrap().clone();
    let user_store = users::new_user_store(&pw_hash);
    let session_store = jwt_sessions::new_session_store();
    let login_limiter = rate_limit::RateLimiter::new();
    let channel_store = notifications::new_channel_store();
    let rule_store = alert_rules::new_rule_store();
    alert_rules::start_alert_checker(rule_store.clone(), channel_store.clone());
    let history_store = charts::new_history_store();
    let audit_log = audit::new_audit_log();
    let audit_forwarding = audit_forwarding::Forwarder::start();
    charts::start_collector(history_store.clone());
    let api_key_store = api_keys::new_api_key_store();
    let schedule_store = backup::new_schedule_store();
    backup::start_backup_scheduler(schedule_store.clone());
    let terminal_tickets = terminal_tickets::new_store();
    let share_tickets = terminal::share_ticket::new_store();
    let pty_store = terminal_registry.store.clone();
    terminal::restore_sessions(&pty_store);
    terminal::start_pty_cleanup(pty_store.clone());

    let mec_services = mec::services::bootstrap::bootstrap(mec_mode).await;
    let mec_state = web::Data::new(
        mec::MecState::new(mec_db, mec_services).with_channels(channel_store.clone()),
    );
    // One shared poller feeds all dashboard SSE subscribers (single set of kube
    // calls regardless of how many walls are open).
    mec::handlers::dashboard_sse::start_poller(&mec_state);
    mec::managed_memberships::start(mec_state.clone(), user_store.clone());
    // Shared (cross-worker) throttle for recording MEC read access as audit views.
    let mec_view_throttle = mec::safety::new_view_throttle();

    println!("NabiMan Server starting on http://0.0.0.0:{}", port);
    println!("Static files: {}", static_dir);

    HttpServer::new(move || {
        let secret = jwt_secret.clone();
        let pw_store = password_store.clone();
        let usr_store = user_store.clone();
        let sess_store = session_store.clone();
        let chan_store = channel_store.clone();
        let rul_store = rule_store.clone();
        let hist_store = history_store.clone();
        let aud_log = audit_log.clone();
        let index_path = format!("{}/index.html", static_dir);

        App::new()
            .wrap(mec::safety::MecReadOnly)
            .wrap(mec::safety::MecRateLimit::new())
            .wrap(mec::safety::MecAccessAudit::new(
                mec_state.audit.clone(),
                mec_view_throttle.clone(),
            ))
            .wrap(security_headers::SecurityHeaders)
            .wrap(login_limiter.clone())
            .wrap(AuthCheck::new(
                secret.clone(),
                sess_store.clone(),
                usr_store.clone(),
            ))
            .wrap(build_cors())
            .wrap(audit_middleware::AuditMiddleware::new(aud_log.clone(), audit_forwarding.clone()))
            .app_data(web::Data::new(secret))
            .app_data(web::Data::new(pw_store))
            .app_data(web::Data::new(usr_store))
            .app_data(web::Data::new(sess_store))
            .app_data(web::Data::new(chan_store))
            .app_data(web::Data::new(rul_store))
            .app_data(web::Data::new(hist_store))
            .app_data(web::Data::new(aud_log))
            .app_data(web::Data::new(audit_forwarding.clone()))
            .app_data(web::Data::new(api_key_store.clone()))
            .app_data(web::Data::new(schedule_store.clone()))
            .app_data(web::Data::new(pty_store.clone()))
            .app_data(web::Data::new(terminal_preferences.clone()))
            .app_data(web::Data::new(share_store.clone()))
            .app_data(web::Data::new(terminal_tickets.clone()))
            .app_data(web::Data::new(share_tickets.clone()))
            .app_data(helper_client.clone())
            .app_data(runtime_identity.clone())
            .app_data(mec_state.clone())
            .configure(auth::config)
            .configure(external_identities::config)
            .configure(users::config)
            .configure(scope_settings::config)
            .configure(jwt_sessions::config)
            .configure(totp::config)
            .configure(server_status::config)
            .configure(network::config)
            .configure(accounts::config)
            .configure(config_manager::config)
            .configure(traffic::config)
            .configure(packages::config)
            .configure(terminal::config)
            .configure(terminal_tickets::config)
            .configure(containers::config)
            .configure(services::config)
            .configure(firewall::config)
            .configure(logs::config)
            .configure(cron::config)
            .configure(processes::config)
            .configure(disks::config)
            .configure(remote_servers::config)
            .configure(updates::config)
            .configure(diagnostics::config)
            .configure(ssl::config)
            .configure(charts::config)
            .configure(file_manager::config)
            .configure(backup::config)
            .configure(audit_forwarding::config)
            .configure(audit::config)
            .configure(database::config)
            .configure(mail::config)
            .configure(swap::config)
            .configure(sessions::config)
            .configure(dns::config)
            .configure(vhost::config)
            .configure(license::config)
            .configure(notifications::config)
            .configure(alert_rules::config)
            .configure(ip_block::config)
            .configure(api_keys::config)
            .configure(ldap_auth::config)
            .configure(oauth::config)
            .configure(openapi::config)
            .configure(mec::config)
            .service(
                afs::Files::new("/", &static_dir)
                    .index_file("index.html")
                    .default_handler(web::to(move || {
                        let path = index_path.clone();
                        async move { afs::NamedFile::open_async(path).await }
                    })),
            )
    })
    .bind(format!("0.0.0.0:{}", port))?
    .run()
    .await
}

#[cfg(test)]
#[path = "main_auth_boundary_tests.rs"]
mod auth_boundary_tests;

#[cfg(test)]
mod auth_principal_tests;

mod external_identities;

#[cfg(test)]
mod mec_member_boundary_tests;

#[cfg(test)]
mod mec_access_plan_tests;

#[cfg(test)]
mod mec_membership_intent_tests;

#[cfg(test)]
mod mec_managed_revocation_tests;

#[cfg(test)]
mod session_identity_tests;
