//! Every API route NabiMan serves, in one place: `main` mounts it, and the
//! route-access test walks the very same routes (see `route_access`).

use actix_web::web;

pub(crate) fn configure(cfg: &mut web::ServiceConfig) {
    crate::observability::config(cfg);
    crate::auth::config(cfg);
    crate::branding::config(cfg);
    crate::external_identities::config(cfg);
    crate::users::config(cfg);
    crate::scope_settings::config(cfg);
    crate::jwt_sessions::config(cfg);
    crate::totp::config(cfg);
    crate::server_status::config(cfg);
    crate::network::config(cfg);
    crate::accounts::config(cfg);
    crate::config_manager::config(cfg);
    crate::traffic::config(cfg);
    crate::packages::config(cfg);
    crate::terminal::config(cfg);
    crate::terminal_tickets::config(cfg);
    crate::containers::config(cfg);
    crate::services::config(cfg);
    crate::firewall::config(cfg);
    crate::logs::config(cfg);
    crate::cron::config(cfg);
    crate::processes::config(cfg);
    crate::disks::config(cfg);
    crate::remote_servers::config(cfg);
    crate::updates::config(cfg);
    crate::diagnostics::config(cfg);
    crate::ssl::config(cfg);
    crate::charts::config(cfg);
    crate::file_manager::config(cfg);
    crate::backup::config(cfg);
    crate::audit_forwarding::config(cfg);
    crate::audit::config(cfg);
    crate::database::config(cfg);
    crate::mail::config(cfg);
    crate::swap::config(cfg);
    crate::sessions::config(cfg);
    crate::dns::config(cfg);
    crate::vhost::config(cfg);
    crate::license::config(cfg);
    crate::usage::config(cfg);
    crate::notifications::config(cfg);
    crate::alert_rules::config(cfg);
    crate::ip_block::config(cfg);
    crate::api_keys::config(cfg);
    crate::ldap_auth::config(cfg);
    crate::oauth::config(cfg);
    crate::openapi::config(cfg);
    crate::mec::config(cfg);
}
