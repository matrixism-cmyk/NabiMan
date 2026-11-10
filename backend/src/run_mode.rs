//! How the API runs (`run_mode` / `NABIMAN_RUN_MODE`):
//!
//! - `root` (the default, for now): the API is root and does root work itself,
//!   exactly as before this setting existed.
//! - `service`: the API is an unprivileged user (`nabiman`). Root work goes
//!   through the typed helper where it has the operation (service control,
//!   firewall rules, package upgrades); everything else that needs root
//!   answers 503 with what is unavailable and why, instead of failing in the
//!   middle. `GET /api/diagnostics/runtime` lists each feature's state.
//!
//! The inventory behind the table: docs/security/privilege-inventory.md.

use std::sync::OnceLock;

use actix_web::body::MessageBody;
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::middleware::Next;
use actix_web::{http::Method, HttpResponse};
use serde::Serialize;

use crate::install_config::values::{env, text_in, Lookup};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RunMode {
    Root,
    Service,
}

pub(crate) fn mode_in(get: Lookup) -> Result<RunMode, String> {
    match text_in(get, "NABIMAN_RUN_MODE").as_deref().map(str::to_ascii_lowercase).as_deref() {
        None | Some("root") => Ok(RunMode::Root),
        Some("service") => Ok(RunMode::Service),
        Some(other) => Err(format!("NABIMAN_RUN_MODE is root or service, not {other:?}")),
    }
}

static MODE: OnceLock<RunMode> = OnceLock::new();

/// What startup decides: `service` while root is refused (start it as the
/// service user); `root` while not root carries on as before, with a note.
pub(crate) fn decide(mode: RunMode, euid: u32) -> Result<Option<String>, String> {
    match (mode, euid) {
        (RunMode::Service, 0) => Err("run_mode = service, but the server was started as root: start it as the service user (User=nabiman), or set run_mode = root".into()),
        (RunMode::Root, uid) if uid != 0 => Ok(Some(format!(
            "Running as uid {uid} with run_mode = root: features that need root will fail when used; set run_mode = service to have them reported as unavailable"
        ))),
        _ => Ok(None),
    }
}

pub(crate) fn at_startup() -> std::io::Result<RunMode> {
    let invalid = |e: String| std::io::Error::new(std::io::ErrorKind::InvalidInput, e);
    let mode = mode_in(&env).map_err(invalid)?;
    if let Some(note) = decide(mode, unsafe { libc::geteuid() }).map_err(invalid)? {
        eprintln!("{note}");
    }
    if mode == RunMode::Service {
        println!("Run mode: service (unprivileged; root work through the helper or reported unavailable)");
    }
    let _ = MODE.set(mode);
    Ok(mode)
}

/// The mode startup chose; `root` when startup did not run (tests).
pub(crate) fn current() -> RunMode {
    MODE.get().copied().unwrap_or(RunMode::Root)
}

pub(crate) fn is_service() -> bool {
    current() == RunMode::Service
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Status {
    /// Works as in root mode.
    Available,
    /// Done by the root helper, when its policy approves it.
    ViaHelper,
    /// Works with less reach (the service user's own files and processes).
    Limited,
    /// Refused with 503 in service mode.
    Unavailable,
}

/// One feature that needs root today, and what service mode does with it.
pub(crate) struct Feature {
    pub key: &'static str,
    pub title: &'static str,
    pub service: Status,
    pub reason: &'static str,
    /// `(writes_only, path)`: requests to `path` (exact, or below it with a
    /// trailing `/`); `writes_only` leaves GET/HEAD alone.
    pub routes: &'static [(bool, &'static str)],
}

pub(crate) const FEATURES: &[Feature] = &[
    Feature { key: "service_control", title: "Service control", service: Status::ViaHelper, reason: "systemctl start/stop/restart/enable/disable through the helper, for units its policy lists", routes: &[] },
    Feature { key: "firewall", title: "Firewall rules", service: Status::ViaHelper, reason: "ufw/firewalld/iptables need root (CAP_NET_ADMIN); the helper lists, adds and deletes rules when privileged-system.json allows firewall", routes: &[] },
    Feature { key: "package_upgrade", title: "Package updates", service: Status::ViaHelper, reason: "the package lists and upgrades need root; the helper refreshes and upgrades when privileged-system.json allows package_upgrade", routes: &[] },
    Feature { key: "package_install", title: "Installing and removing packages", service: Status::Unavailable, reason: "apt/dnf install and remove need root and are not a helper operation yet", routes: &[(true, "/api/packages/install"), (true, "/api/packages/remove")] },
    Feature { key: "vhost", title: "Web server sites", service: Status::Unavailable, reason: "writing /etc/nginx and /etc/apache2 and reloading the web server need root", routes: &[(true, "/api/vhost"), (true, "/api/vhost/")] },
    Feature { key: "config_files", title: "Service configuration files", service: Status::Unavailable, reason: "editing, rolling back and restarting with files under /etc need root", routes: &[(true, "/api/config/")] },
    Feature { key: "certificates", title: "Let's Encrypt certificates", service: Status::Unavailable, reason: "certbot writes /etc/letsencrypt and reloads the web server as root", routes: &[(true, "/api/ssl/")] },
    Feature { key: "os_accounts", title: "OS accounts", service: Status::Unavailable, reason: "useradd, userdel and chpasswd need root", routes: &[(true, "/api/accounts"), (true, "/api/accounts/")] },
    Feature { key: "crontabs", title: "Other users' crontabs", service: Status::Unavailable, reason: "crontab -u needs root", routes: &[(true, "/api/cron"), (true, "/api/cron/")] },
    Feature { key: "ip_block", title: "Blocking addresses", service: Status::Unavailable, reason: "fail2ban-client, iptables and nft need root", routes: &[(true, "/api/ipblock/ban"), (true, "/api/ipblock/unban")] },
    Feature { key: "login_sessions", title: "Ending login sessions", service: Status::Unavailable, reason: "signalling other users' processes needs root", routes: &[(true, "/api/sessions/kill")] },
    Feature { key: "databases", title: "Database queries, backups and restores", service: Status::Unavailable, reason: "they sign in as root's MySQL socket account or through sudo -u postgres", routes: &[(true, "/api/database/")] },
    Feature { key: "office_preview", title: "Office and Hangul previews", service: Status::Unavailable, reason: "the converters handle untrusted files and are run as nobody, which needs root; as the service user they could read NabiMan's data", routes: &[(false, "/api/preview/office")] },
    Feature { key: "terminal_as_user", title: "Starting sessions as another account", service: Status::Unavailable, reason: "su/runuser to another account needs root; sessions run as the service user", routes: &[] },
    Feature { key: "local_terminal", title: "Local terminal", service: Status::Limited, reason: "shells run as the service user, not root (no sudo: NoNewPrivileges)", routes: &[] },
    Feature { key: "processes", title: "Process signals and details", service: Status::Limited, reason: "only the service user's own processes", routes: &[] },
    Feature { key: "files", title: "File manager", service: Status::Limited, reason: "what the service user may read and write; /usr and /etc are read-only under ProtectSystem", routes: &[] },
    Feature { key: "claude_peers", title: "Claude sessions of other accounts", service: Status::Limited, reason: "other accounts' ~/.claude cannot be read", routes: &[] },
    Feature { key: "logs", title: "System logs and mail queue", service: Status::Limited, reason: "journalctl needs the systemd-journal (or adm) group; mailq the mail system's group", routes: &[] },
    Feature { key: "containers", title: "Containers", service: Status::Limited, reason: "the docker socket needs the docker group (which is root-equivalent; grant knowingly)", routes: &[] },
    Feature { key: "serial_bridge", title: "Browser serial devices", service: Status::Limited, reason: "the PTY's group is NABIMAN_SERIAL_GROUP only if the service user is in that group", routes: &[] },
];

fn matches(path: &str, route: &str) -> bool {
    match route.strip_suffix('/') {
        Some(dir) => path.strip_prefix(dir).is_some_and(|rest| rest.starts_with('/') && rest.len() > 1),
        None => path == route,
    }
}

/// The feature refused for this request in `mode`, if any.
pub(crate) fn refused(mode: RunMode, method: &Method, path: &str) -> Option<&'static Feature> {
    if mode != RunMode::Service {
        return None;
    }
    let write = ![Method::GET, Method::HEAD, Method::OPTIONS].contains(method);
    FEATURES.iter().filter(|f| f.service == Status::Unavailable).find(|f| {
        f.routes.iter().any(|(writes_only, route)| (write || !writes_only) && matches(path, route))
    })
}

/// The answer to a refused request: 503, what and why.
pub(crate) fn unavailable(feature: &Feature) -> HttpResponse {
    HttpResponse::ServiceUnavailable().insert_header(("Cache-Control", "no-store")).json(serde_json::json!({
        "success": false,
        "message": format!(
            "{} is not available while NabiMan runs as an unprivileged service (run_mode = service): {}. See privileges in GET /api/diagnostics/runtime.",
            feature.title, feature.reason
        ),
        "data": { "feature": feature.key, "run_mode": "service", "unavailable": true },
    }))
}

/// Middleware: in service mode, refuse what needs root and has no helper operation.
pub(crate) async fn gate(
    req: ServiceRequest,
    next: Next<impl MessageBody + 'static>,
) -> Result<ServiceResponse<actix_web::body::EitherBody<impl MessageBody>>, actix_web::Error> {
    // Tests give the mode as app data; the server uses what startup chose.
    let mode = req.app_data::<actix_web::web::Data<RunMode>>().map_or_else(current, |m| *m.get_ref());
    if let Some(feature) = refused(mode, req.method(), req.path()) {
        return Ok(req.into_response(unavailable(feature)).map_into_right_body());
    }
    next.call(req).await.map(ServiceResponse::map_into_left_body)
}

#[derive(Serialize)]
pub(crate) struct FeatureState {
    pub key: &'static str,
    pub status: Status,
    pub reason: &'static str,
}

#[derive(Serialize)]
pub(crate) struct Report {
    pub run_mode: RunMode,
    pub euid: u32,
    pub helper_socket: String,
    pub features: Vec<FeatureState>,
}

pub(crate) fn report_for(mode: RunMode) -> Report {
    let features = FEATURES.iter().map(|f| match (mode, f.key) {
        // Root mode has always refused service changes (crate::service_control).
        (RunMode::Root, "service_control") => FeatureState { key: f.key, status: Status::Unavailable, reason: "refused while the API runs as root; run_mode = service with the helper" },
        (RunMode::Root, _) => FeatureState { key: f.key, status: Status::Available, reason: "run directly by the root API" },
        _ => FeatureState { key: f.key, status: f.service, reason: f.reason },
    }).collect();
    Report {
        run_mode: mode,
        euid: unsafe { libc::geteuid() },
        helper_socket: text_in(&env, "NABIMAN_HELPER_SOCKET").unwrap_or_else(|| "/run/nabiman-helper/control.sock".into()),
        features,
    }
}

pub(crate) fn report() -> Report {
    report_for(current())
}

#[cfg(test)]
#[path = "run_mode_tests.rs"]
mod tests;
