//! How NabiMan reports on itself: health endpoints for load balancers and
//! container runtimes (see docs/operations/monitoring.md).

pub(crate) mod health;

use actix_web::web;

/// The routes outside `/api`: health for probes.
pub(crate) fn config(cfg: &mut web::ServiceConfig) {
    cfg.route("/healthz", web::get().to(health::healthz))
        .route("/readyz", web::get().to(health::readyz));
}
