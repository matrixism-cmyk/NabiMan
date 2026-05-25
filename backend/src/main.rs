mod server_status;
mod network;
mod accounts;
mod config_manager;
mod service_registry;
mod traffic;
mod packages;
mod terminal;
mod containers;
mod services;
mod firewall;
mod logs;
mod cron;
mod processes;
mod disks;
mod remote_servers;
mod ssh_keys;
mod updates;
mod diagnostics;
mod ssl;
mod charts;
mod file_manager;
mod backup;
mod audit;
mod database;
mod mail;
mod swap;
mod sessions;
mod dns;
mod vhost;
mod license;
mod rate_limit;
mod security_headers;
mod audit_middleware;
mod notifications;
mod alert_rules;
mod auth;
mod users;
mod rbac;
mod jwt_sessions;
mod totp;
mod api_keys;
mod ldap_auth;
mod oauth;
mod ip_block;
mod openapi;
mod models;
mod mec;

use actix_cors::Cors;
use actix_files as afs;
use actix_web::{App, HttpServer, web, HttpResponse};
use actix_web::dev::{ServiceRequest, ServiceResponse, Transform, Service};
use actix_web::body::EitherBody;
use std::future::{Ready, ready, Future};
use std::pin::Pin;
use std::rc::Rc;

// --- Auth Middleware (JWT-based) ---
pub struct AuthCheck {
    secret: auth::JwtSecret,
}

impl AuthCheck {
    pub fn new(secret: auth::JwtSecret) -> Self { Self { secret } }
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
        }))
    }
}

pub struct AuthCheckMiddleware<S> {
    service: Rc<S>,
    secret: auth::JwtSecret,
}

impl<S, B> Service<ServiceRequest> for AuthCheckMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = actix_web::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(&self, cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        if !auth::check_auth(&req, &self.secret) {
            return Box::pin(async {
                Ok(req.into_response(
                    HttpResponse::Unauthorized()
                        .json(models::ApiResponse::<()>::error("Unauthorized"))
                ).map_into_right_body())
            });
        }
        let svc = self.service.clone();
        Box::pin(async move {
            svc.call(req).await.map(|res| res.map_into_left_body())
        })
    }
}

fn build_cors() -> Cors {
    match std::env::var("NABIMAN_CORS_ORIGINS") {
        Ok(origins) if !origins.is_empty() => {
            let mut cors = Cors::default()
                .allow_any_method()
                .allowed_headers(["Content-Type", "Authorization", "X-Auth-Token", "X-API-Key", "X-File-Path"])
                .max_age(3600);
            for origin in origins.split(',') {
                cors = cors.allowed_origin(origin.trim());
            }
            cors
        }
        _ => Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allowed_headers(["Content-Type", "Authorization", "X-Auth-Token", "X-API-Key", "X-File-Path"])
            .max_age(3600),
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let port: u16 = std::env::var("NABIMAN_PORT")
        .ok().and_then(|p| p.parse().ok()).unwrap_or(8080);
    let static_dir = std::env::var("NABIMAN_STATIC")
        .unwrap_or_else(|_| "./static".to_string());

    let password_store = auth::new_password_store();
    let jwt_secret = auth::new_jwt_secret();
    let pw_hash = password_store.lock().unwrap().clone();
    let user_store = users::new_user_store(&pw_hash);
    let session_store = jwt_sessions::new_session_store();
    let channel_store = notifications::new_channel_store();
    let rule_store = alert_rules::new_rule_store();
    alert_rules::start_alert_checker(rule_store.clone(), channel_store.clone());
    let history_store = charts::new_history_store();
    let audit_log = audit::new_audit_log();
    charts::start_collector(history_store.clone());
    let api_key_store = api_keys::new_api_key_store();
    let schedule_store = backup::new_schedule_store();
    backup::start_backup_scheduler(schedule_store.clone());
    let pty_store = terminal::new_pty_store();
    terminal::start_pty_cleanup(pty_store.clone());

    // MEC Management module state. Services pick real/mock based on
    // NABIMAN_MEC_MODE=real|mock (default: auto — real when env creds exist).
    let mec_db = mec::db::open(mec::db::default_path())
        .unwrap_or_else(|e| {
            eprintln!("Failed to open MEC DB, falling back to in-memory: {}", e);
            mec::db::open_in_memory().expect("in-memory MEC DB")
        });
    let mec_mode = mec::services::bootstrap::ServiceMode::from_env();
    let mec_services = mec::services::bootstrap::bootstrap(mec_mode).await;
    let mec_state = web::Data::new(
        mec::MecState::new(mec_db, mec_services).with_channels(channel_store.clone()),
    );
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
            .wrap(audit_middleware::AuditMiddleware::new(aud_log.clone()))
            .wrap(mec::safety::MecReadOnly)
            .wrap(mec::safety::MecRateLimit::new())
            .wrap(mec::safety::MecAccessAudit::new(
                mec_state.audit.clone(),
                mec_view_throttle.clone(),
            ))
            .wrap(security_headers::SecurityHeaders)
            .wrap(build_cors())
            .wrap(rate_limit::RateLimiter::new())
            .wrap(AuthCheck::new(secret.clone()))
            .app_data(web::Data::new(secret))
            .app_data(web::Data::new(pw_store))
            .app_data(web::Data::new(usr_store))
            .app_data(web::Data::new(sess_store))
            .app_data(web::Data::new(chan_store))
            .app_data(web::Data::new(rul_store))
            .app_data(web::Data::new(hist_store))
            .app_data(web::Data::new(aud_log))
            .app_data(web::Data::new(api_key_store.clone()))
            .app_data(web::Data::new(schedule_store.clone()))
            .app_data(web::Data::new(pty_store.clone()))
            .app_data(mec_state.clone())
            .configure(auth::config)
            .configure(users::config)
            .configure(jwt_sessions::config)
            .configure(totp::config)
            .configure(server_status::config)
            .configure(network::config)
            .configure(accounts::config)
            .configure(config_manager::config)
            .configure(traffic::config)
            .configure(packages::config)
            .configure(terminal::config)
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
                    .default_handler(
                        web::to(move || {
                            let path = index_path.clone();
                            async move { afs::NamedFile::open_async(path).await }
                        })
                    )
            )
    })
    .bind(format!("0.0.0.0:{}", port))?
    .run()
    .await
}
