//! Counters for `/metrics` (Prometheus text format). Each is an atomic
//! bumped where the event happens; the per-route counter is one small map
//! keyed by route pattern (a bounded set) and status.
//!
//! `/metrics` is for super administrators, like the rest of the admin API;
//! a scraper can instead send `Authorization: Bearer <NABIMAN_METRICS_TOKEN>`
//! when that setting is given.

use crate::lock_safe::LockExt;
use std::collections::BTreeMap;
use std::fmt::Write;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering::Relaxed};
use std::sync::Mutex;
use std::time::Duration;

/// Lives with the bounded runner, which is in the library the helper shares.
pub(crate) use nabiman_server::bounded_command::TIMEOUTS as CHILD_TIMEOUTS;
pub(crate) static AUDIT_WRITE_FAILURES: AtomicU64 = AtomicU64::new(0);
pub(crate) static LOGIN_FAILURES: AtomicU64 = AtomicU64::new(0);
pub(crate) static RATE_LIMITED: AtomicU64 = AtomicU64::new(0);
pub(crate) static WS_OPEN: AtomicI64 = AtomicI64::new(0);
pub(crate) static WS_OPENED: AtomicU64 = AtomicU64::new(0);

/// Upper bounds of the request-duration buckets, in seconds.
const BUCKETS: [f64; 12] = [0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0];
static BUCKET_COUNTS: [AtomicU64; 12] = [const { AtomicU64::new(0) }; 12];
static DURATION_COUNT: AtomicU64 = AtomicU64::new(0);
static DURATION_SUM_MICROS: AtomicU64 = AtomicU64::new(0);
static REQUESTS: Mutex<BTreeMap<(String, u16), u64>> = Mutex::new(BTreeMap::new());

/// The sign-in routes whose refusals count as login failures.
const LOGIN_ROUTES: [&str; 3] = ["/api/auth/login", "/api/auth/ldap/login", "/api/auth/2fa/verify"];

/// One request answered: by route pattern, status and how long it took.
pub(crate) fn record_request(route: &str, status: u16, took: Duration) {
    *REQUESTS.lock_safe().entry((route.to_string(), status)).or_insert(0) += 1;
    let secs = took.as_secs_f64();
    for (i, bound) in BUCKETS.iter().enumerate() {
        if secs <= *bound {
            BUCKET_COUNTS[i].fetch_add(1, Relaxed);
        }
    }
    DURATION_COUNT.fetch_add(1, Relaxed);
    DURATION_SUM_MICROS.fetch_add(took.as_micros().min(u64::MAX as u128) as u64, Relaxed);
    if status == 429 {
        RATE_LIMITED.fetch_add(1, Relaxed);
    }
    if LOGIN_ROUTES.contains(&route) && matches!(status, 401 | 403) {
        LOGIN_FAILURES.fetch_add(1, Relaxed);
    }
}

/// Held for as long as a WebSocket connection is open.
pub(crate) struct WsOpen(());

impl WsOpen {
    pub(crate) fn begin() -> Self {
        WS_OPEN.fetch_add(1, Relaxed);
        WS_OPENED.fetch_add(1, Relaxed);
        WsOpen(())
    }
}

impl Drop for WsOpen {
    fn drop(&mut self) {
        WS_OPEN.fetch_sub(1, Relaxed);
    }
}

/// Values read at scrape time rather than counted.
#[derive(Default)]
pub(crate) struct Gauges {
    pub terminal_sessions: usize,
    /// Unix time of the newest successful scheduled backup.
    pub backup_last_success: Option<i64>,
}

fn label(v: &str) -> String {
    v.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n")
}

fn metric(out: &mut String, name: &str, kind: &str, help: &str) {
    let _ = writeln!(out, "# HELP {name} {help}\n# TYPE {name} {kind}");
}

/// Everything, in the Prometheus text exposition format (0.0.4).
pub(crate) fn render(g: &Gauges) -> String {
    let mut out = String::new();
    metric(&mut out, "nabiman_http_requests_total", "counter", "HTTP requests answered, by route pattern and status.");
    for ((route, status), n) in REQUESTS.lock_safe().iter() {
        let _ = writeln!(out, "nabiman_http_requests_total{{route=\"{}\",status=\"{status}\"}} {n}", label(route));
    }
    metric(&mut out, "nabiman_http_request_duration_seconds", "histogram", "Time to answer an HTTP request (to the response head).");
    for (i, bound) in BUCKETS.iter().enumerate() {
        let _ = writeln!(out, "nabiman_http_request_duration_seconds_bucket{{le=\"{bound}\"}} {}", BUCKET_COUNTS[i].load(Relaxed));
    }
    let count = DURATION_COUNT.load(Relaxed);
    let _ = writeln!(out, "nabiman_http_request_duration_seconds_bucket{{le=\"+Inf\"}} {count}");
    let _ = writeln!(out, "nabiman_http_request_duration_seconds_sum {}", DURATION_SUM_MICROS.load(Relaxed) as f64 / 1e6);
    let _ = writeln!(out, "nabiman_http_request_duration_seconds_count {count}");
    let simple: [(&str, &str, &str, String); 8] = [
        ("nabiman_terminal_sessions", "gauge", "Terminal sessions on record (open tmux sessions).", g.terminal_sessions.to_string()),
        ("nabiman_websocket_connections", "gauge", "WebSocket connections open now.", WS_OPEN.load(Relaxed).max(0).to_string()),
        ("nabiman_websocket_connections_total", "counter", "WebSocket connections opened.", WS_OPENED.load(Relaxed).to_string()),
        ("nabiman_child_process_timeouts_total", "counter", "Child processes stopped at their deadline by the bounded runner.", CHILD_TIMEOUTS.load(Relaxed).to_string()),
        ("nabiman_audit_write_failures_total", "counter", "Audit records that could not be written to the local journal.", AUDIT_WRITE_FAILURES.load(Relaxed).to_string()),
        ("nabiman_login_failures_total", "counter", "Refused sign-ins (password, LDAP, second factor).", LOGIN_FAILURES.load(Relaxed).to_string()),
        ("nabiman_rate_limited_total", "counter", "Requests refused with 429 by a rate limit.", RATE_LIMITED.load(Relaxed).to_string()),
        (
            "nabiman_backup_last_success_timestamp_seconds",
            "gauge",
            "Unix time of the newest successful scheduled backup (0: none yet).",
            g.backup_last_success.unwrap_or(0).to_string(),
        ),
    ];
    for (name, kind, help, value) in simple {
        metric(&mut out, name, kind, help);
        let _ = writeln!(out, "{name} {value}");
    }
    out
}

/// `GET /metrics`. The gate in front (`auth::check_auth`) lets through
/// super administrators and the scrape token only.
pub(crate) async fn serve(req: actix_web::HttpRequest) -> actix_web::HttpResponse {
    let terminal_sessions = req.app_data::<actix_web::web::Data<crate::terminal::PtyStore>>().map_or(0, |s| s.lock_safe().len());
    let backup_last_success = req
        .app_data::<actix_web::web::Data<crate::backup::BackupScheduleStore>>()
        .and_then(|s| crate::backup::last_success(s));
    actix_web::HttpResponse::Ok()
        .content_type("text/plain; version=0.0.4; charset=utf-8")
        .insert_header(("Cache-Control", "no-store"))
        .body(render(&Gauges { terminal_sessions, backup_last_success }))
}

/// Whether the request carries the scrape token (`NABIMAN_METRICS_TOKEN`).
pub(crate) fn scrape_token_matches(headers: &actix_web::http::header::HeaderMap) -> bool {
    scrape_token_in(headers, &crate::install_config::values::env)
}

pub(crate) fn scrape_token_in(headers: &actix_web::http::header::HeaderMap, get: crate::install_config::values::Lookup) -> bool {
    use sha2::{Digest, Sha256};
    let Some(expected) = crate::install_config::values::text_in(get, "NABIMAN_METRICS_TOKEN") else { return false };
    let mut given = headers.get_all("Authorization");
    let (Some(one), None) = (given.next(), given.next()) else { return false };
    let Some(token) = one.to_str().ok().and_then(|v| v.strip_prefix("Bearer ")) else { return false };
    // Equal-length digests compared in full: the time says nothing about the token.
    let (a, b) = (Sha256::digest(token.trim().as_bytes()), Sha256::digest(expected.as_bytes()));
    a.iter().zip(b.iter()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
#[path = "metrics_tests.rs"]
mod tests;
