//! How NabiMan reports on itself: structured logs, health endpoints for
//! load balancers and container runtimes (see docs/operations/monitoring.md).

#[macro_use]
mod macros;
pub(crate) mod damp;
pub(crate) mod health;
pub(crate) mod log;
pub(crate) mod metrics;
mod redact;
pub(crate) mod request_log;

use actix_web::web;

/// The one path outside `/api` that the sign-in gate judges (admin or the scrape token).
pub(crate) const METRICS_PATH: &str = "/metrics";

/// The routes outside `/api`: health for probes, metrics for scrapers.
pub(crate) fn config(cfg: &mut web::ServiceConfig) {
    cfg.route("/healthz", web::get().to(health::healthz))
        .route("/readyz", web::get().to(health::readyz))
        .route(METRICS_PATH, web::get().to(metrics::serve));
}
