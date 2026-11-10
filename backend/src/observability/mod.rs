//! How NabiMan reports on itself: structured logs, health endpoints for
//! load balancers and container runtimes (see docs/operations/monitoring.md).

#[macro_use]
mod macros;
pub(crate) mod health;
pub(crate) mod log;
mod redact;
pub(crate) mod request_log;

use actix_web::web;

/// The routes outside `/api`: health for probes.
pub(crate) fn config(cfg: &mut web::ServiceConfig) {
    cfg.route("/healthz", web::get().to(health::healthz))
        .route("/readyz", web::get().to(health::readyz));
}
