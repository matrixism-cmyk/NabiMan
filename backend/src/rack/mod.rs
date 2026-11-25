//! The server-room rack page: the racks as a file describes them
//! (`NABIMAN_RACK_LAYOUT_FILE`) and how each machine is doing, from
//! Prometheus (`NABIMAN_RACK_PROMETHEUS_URL`). Read-only: two GET routes,
//! no control of any device.
//!
//! - `GET /api/rack/layout`: the file, checked and cut down (see `layout`).
//!   Read again whenever the file (or what its link points to) changes.
//! - `GET /api/rack/status`: each node's and device's state (see `status`),
//!   from a fixed set of queries (see `prometheus`) whose answers are kept
//!   for ten seconds, so many open pages cost Prometheus the same as one.
//!
//! With neither setting the feature is off: both routes say so and the web
//! app hides the menu entry (`rack` in `GET /api/branding`).

mod layout;
mod ordered;
mod prometheus;
mod status;
#[cfg(test)]
mod layout_tests;
#[cfg(test)]
mod service_tests;
#[cfg(test)]
mod status_tests;

use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};

use actix_web::{web, HttpResponse};
use serde::Serialize;

use crate::install_config::values::{env, text_in, Lookup};
use crate::lock_safe::LockExt;
use crate::models::ApiResponse;
use layout::Layout;
use prometheus::Samples;
use status::{Source, Status};

/// How long Prometheus' answers serve every caller.
const STATUS_TTL: Duration = Duration::from_secs(10);

/// Whether the page exists on this install: either setting is given.
pub(crate) fn enabled_in(get: Lookup) -> bool {
    text_in(get, "NABIMAN_RACK_LAYOUT_FILE").is_some() || text_in(get, "NABIMAN_RACK_PROMETHEUS_URL").is_some()
}

/// Why there is no layout to show: a code for the page, and the detail.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub(crate) struct Fault {
    /// `no_file` (the setting is missing), `unreadable`, `invalid`.
    pub code: &'static str,
    pub detail: String,
}

struct Kept {
    /// What the file was when read: where its link led, when it changed, its size.
    stamp: (PathBuf, SystemTime, u64),
    result: Result<Arc<Layout>, Fault>,
}

struct Collected {
    at: Instant,
    stamp: String,
    result: Result<Samples, String>,
}

pub(crate) struct Service {
    layout_file: Option<PathBuf>,
    /// The setting, checked: none, a base to query, or why it cannot be used.
    prometheus: Option<Result<reqwest::Url, String>>,
    client: Result<reqwest::Client, String>,
    kept: Mutex<Option<Kept>>,
    /// Held while asking, so callers arriving together share one round.
    collected: tokio::sync::Mutex<Option<Arc<Collected>>>,
    ttl: Duration,
}

impl Service {
    pub(crate) fn from_settings(get: Lookup) -> Self {
        Self::new(text_in(get, "NABIMAN_RACK_LAYOUT_FILE").map(PathBuf::from), text_in(get, "NABIMAN_RACK_PROMETHEUS_URL"), STATUS_TTL)
    }

    pub(crate) fn new(layout_file: Option<PathBuf>, prometheus_url: Option<String>, ttl: Duration) -> Self {
        Self {
            layout_file,
            prometheus: prometheus_url.map(|u| prometheus::base_url(&u)),
            client: prometheus::client(),
            kept: Mutex::new(None),
            collected: tokio::sync::Mutex::new(None),
            ttl,
        }
    }

    pub(crate) fn enabled(&self) -> bool {
        self.layout_file.is_some() || self.prometheus.is_some()
    }

    /// The layout as the file is now. The file is read again only when it
    /// changed; a file that cannot be used is reported, and tried again
    /// once it changes.
    pub(crate) fn layout(&self) -> Result<Arc<Layout>, Fault> {
        let Some(path) = &self.layout_file else {
            return Err(Fault { code: "no_file", detail: "NABIMAN_RACK_LAYOUT_FILE (rack_layout_file) is not set".into() });
        };
        let unreadable = |e: std::io::Error| Fault { code: "unreadable", detail: format!("{}: {e}", path.display()) };
        let (real, meta) = match std::fs::canonicalize(path).and_then(|real| std::fs::metadata(&real).map(|m| (real, m))) {
            Ok(found) => found,
            Err(e) => {
                *self.kept.lock_safe() = None;
                return Err(unreadable(e));
            }
        };
        let stamp = (real, meta.modified().unwrap_or(SystemTime::UNIX_EPOCH), meta.len());
        let mut kept = self.kept.lock_safe();
        if let Some(k) = kept.as_ref().filter(|k| k.stamp == stamp) {
            return k.result.clone();
        }
        let result = if !meta.is_file() {
            Err(Fault { code: "unreadable", detail: format!("{}: not a file", path.display()) })
        } else if meta.len() > layout::MAX_FILE_BYTES {
            Err(Fault { code: "invalid", detail: format!("the file is larger than {} bytes", layout::MAX_FILE_BYTES) })
        } else {
            std::fs::read(&stamp.0).map_err(unreadable).and_then(|bytes| layout::parse(&bytes).map(Arc::new).map_err(|detail| Fault { code: "invalid", detail }))
        };
        match &result {
            Ok(l) => log_info!("rack", "rack layout read", path = stamp.0.display(), version = l.version, racks = l.racks.len(), nodes = l.nodes.len()),
            Err(f) => log_warn!("rack", "the rack layout file cannot be used", path = stamp.0.display(), code = f.code, detail = f.detail),
        }
        *kept = Some(Kept { stamp, result: result.clone() });
        result
    }

    /// Prometheus' answers, at most `ttl` old.
    async fn collected(&self, base: &reqwest::Url) -> Arc<Collected> {
        let mut slot = self.collected.lock().await;
        if let Some(have) = slot.as_ref().filter(|c| c.at.elapsed() < self.ttl) {
            return have.clone();
        }
        let result = match &self.client {
            Ok(client) => prometheus::collect(client, base).await,
            Err(e) => Err(e.clone()),
        };
        if let Err(e) = &result {
            log_warn!("rack", "Prometheus gave no answer for the rack page", reason = e);
        }
        let fresh = Arc::new(Collected { at: Instant::now(), stamp: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true), result });
        *slot = Some(fresh.clone());
        fresh
    }

    /// The status of everything in `layout`.
    pub(crate) async fn status(&self, layout: &Layout) -> Status {
        let now = || chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        match &self.prometheus {
            None => status::assemble(layout, Source::NotConfigured, now()),
            Some(Err(why)) => status::assemble(layout, Source::Unreachable(&format!("rack_prometheus_url: {why}")), now()),
            Some(Ok(base)) => {
                let collected = self.collected(base).await;
                let source = match &collected.result {
                    Ok(samples) => Source::Collected(samples),
                    Err(why) => Source::Unreachable(why),
                };
                status::assemble(layout, source, collected.stamp.clone())
            }
        }
    }
}

#[derive(Serialize)]
struct LayoutAnswer {
    /// False: the feature is off on this install (nothing else is given).
    configured: bool,
    #[serde(serialize_with = "shared")]
    layout: Option<Arc<Layout>>,
    error: Option<Fault>,
}

#[derive(Serialize)]
struct StatusAnswer {
    configured: bool,
    status: Option<Status>,
    /// Why there is no status: the layout it would describe cannot be read.
    error: Option<Fault>,
}

/// serde's `rc` feature is not on; the layout is written as itself.
fn shared<S: serde::Serializer>(layout: &Option<Arc<Layout>>, to: S) -> Result<S::Ok, S::Error> {
    layout.as_deref().serialize(to)
}

fn layout_answer(service: &Service) -> LayoutAnswer {
    if !service.enabled() {
        return LayoutAnswer { configured: false, layout: None, error: None };
    }
    match service.layout() {
        Ok(layout) => LayoutAnswer { configured: true, layout: Some(layout), error: None },
        Err(fault) => LayoutAnswer { configured: true, layout: None, error: Some(fault) },
    }
}

async fn status_answer(service: &Service) -> StatusAnswer {
    if !service.enabled() {
        return StatusAnswer { configured: false, status: None, error: None };
    }
    match service.layout() {
        Ok(layout) => StatusAnswer { configured: true, status: Some(service.status(&layout).await), error: None },
        Err(fault) => StatusAnswer { configured: true, status: None, error: Some(fault) },
    }
}

/// This process's service, from its settings (read once).
fn service() -> &'static Service {
    static SERVICE: OnceLock<Service> = OnceLock::new();
    SERVICE.get_or_init(|| Service::from_settings(&env))
}

fn fresh<T: Serialize>(answer: T) -> HttpResponse {
    HttpResponse::Ok().insert_header(("Cache-Control", "no-store")).json(ApiResponse::ok(answer))
}

async fn get_layout() -> HttpResponse {
    fresh(layout_answer(service()))
}

async fn get_status() -> HttpResponse {
    fresh(status_answer(service()).await)
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.route("/api/rack/layout", web::get().to(get_layout)).route("/api/rack/status", web::get().to(get_status));
}
