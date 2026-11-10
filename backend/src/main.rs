mod lock_safe;
mod bounded_command;
mod access_rules;
mod accounts;
mod alert_rules;
mod app_routes;
mod api_keys;
mod audit;
mod audit_forwarding;
mod audit_middleware;
mod auth;
mod auth_gate;
mod backup;
mod charts;
mod cli_args;
mod config_manager;
mod containers;
mod cors;
mod cron;
mod data_dir;
mod database;
mod database_pipe;
mod diagnostics;
mod disks;
mod dns;
mod dns_db;
mod extension_points;
mod file_manager;
mod firewall;
mod install_config;
mod ip_block;
mod jwt_sessions;
mod ldap_auth;
mod license;
mod limits;
mod logs;
mod mail;
mod mec;
mod member_servers;
mod membership;
mod mfa_policy;
mod models;
mod network;
mod notifications;
mod oauth;
mod openapi;
mod packages;
mod password_check;
mod password_policy;
mod private_storage;
mod processes;
mod rate_limit;
mod rbac;
mod route_access;
mod remote;
mod remote_host_trust;
mod remote_probe;
mod attack_watch;
mod remote_servers;
mod server_facts;
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
mod sqlite_schema;
mod ssh_auth;
mod ssh_cert;
mod ssh_keys;
mod ssl;
mod stream_access;
mod support_report;
mod swap;
mod terminal;
mod terminal_tickets;
mod trusted_proxy;
mod totp;
mod traffic;
mod updates;
mod users;
mod vhost;
use actix_files as afs;
use lock_safe::LockExt;
use actix_web::body::EitherBody;
use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::{web, App, HttpServer};
#[cfg(test)]
use actix_web::HttpResponse;
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
        let denial = auth_gate::denial(auth::check_auth(&req, &self.secret, &self.sessions, &self.users));
        if let Some(response) = denial {
            return Box::pin(async move { Ok(req.into_response(response).map_into_right_body()) });
        }
        let svc = self.service.clone();
        Box::pin(async move { svc.call(req).await.map(|res| res.map_into_left_body()) })
    }
}

fn main() -> std::io::Result<()> {
    terminal::telnet_client::run_if_asked();
    terminal::recording::run_if_asked();
    // The settings file joins the environment while this is the only thread.
    install_config::apply_at_startup().map_err(|e| {
        eprintln!("{e}");
        std::io::Error::new(std::io::ErrorKind::InvalidInput, e)
    })?;
    actix_web::rt::System::new().block_on(serve())
}

async fn serve() -> std::io::Result<()> {
    if unsafe { libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    // One server per data directory: taken before any store opens it, held until exit.
    let storage = data_dir::prepare().inspect_err(|e| eprintln!("{e}"))?;
    let runtime_identity = web::Data::new(runtime_identity::RuntimeIdentity::capture()?);
    let mec_mode = mec::services::bootstrap::ServiceMode::from_env()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
    // Validate durable storage before starting background collectors or auth writes.
    let owned_mec_db = mec::db::open_owned(mec::db::default_path())?;
    let mec_db = owned_mec_db.handle.clone();
    storage.seed_defaults();
    let port: u16 = install_config::values::number("NABIMAN_PORT", 8080);
    let static_dir = std::env::var("NABIMAN_STATIC").unwrap_or_else(|_| "./static".to_string());

    let share_store = terminal::share_store::open(
        nabiman_server::data_dir()
        .join("terminal_shares.json"),
    )?;
    let terminal_registry = terminal::open_registry(
        nabiman_server::data_dir()
        .join("terminal_sessions.json"),
    )?;
    let terminal_preferences = terminal::open_settings_registry(
        nabiman_server::data_dir()
        .join("terminal_settings.json"),
    )?;
    let helper_client = web::Data::new(nabiman_server::privileged::client::Client::from_env());
    let password_store = auth::new_password_store();
    let jwt_secret = auth::new_jwt_secret();
    let pw_hash = password_store.lock_safe().clone();
    let user_store = users::new_user_store(&pw_hash);
    let session_store = jwt_sessions::open_session_store();
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
    // Local programs (Claude Code in a session) may open Claude sessions here.
    terminal::start_claude_socket(terminal::ClaudeSocketCtx {
        store: pty_store.clone(),
        users: user_store.clone(),
        audit: audit_log.clone(),
        forwarding: Some(audit_forwarding.clone()),
    });

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
            .wrap(cors::build_cors())
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
            .app_data(web::Data::new(remote::extension_points()))
            .app_data(helper_client.clone())
            .app_data(runtime_identity.clone())
            .app_data(mec_state.clone())
            .configure(app_routes::configure)
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

#[cfg(test)]
#[path = "path_canon_tests.rs"]
mod path_canon_tests;
