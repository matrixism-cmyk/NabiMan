//! `/healthz` (liveness) and `/readyz` (readiness), reached without signing
//! in. Neither says more than a short machine-readable reason: no paths, no
//! versions, no errors' text, so an anonymous caller learns nothing useful.
//!
//! Readiness looks at what a request needs: a data directory it can write,
//! an audit journal that still records, and the server's own tmux (through
//! the bounded tmux call the terminal uses; the probe never starts a tmux
//! server). Results are kept for a few seconds so a busy prober cannot make
//! NabiMan spawn a tmux client per request.

use crate::lock_safe::LockExt;
use actix_web::{web, HttpResponse};
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// How long one readiness answer is reused.
const CACHE_FOR: Duration = Duration::from_secs(5);

/// What the server's tmux said to a probe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tmux {
    /// A tmux server answered.
    Reachable,
    /// No tmux server runs: normal while no terminal session is open.
    NoServer,
    /// tmux did not answer in time, could not start, or answered oddly.
    Unreachable,
}

/// Why the server is not ready, as stable machine-readable words.
pub(crate) fn reasons(data_dir_writable: bool, audit_ok: bool, tmux: Tmux, registered_sessions: usize) -> Vec<&'static str> {
    let mut out = Vec::new();
    if !data_dir_writable {
        out.push("data_dir_not_writable");
    }
    if !audit_ok {
        out.push("audit_journal_unavailable");
    }
    let tmux_ok = match tmux {
        Tmux::Reachable => true,
        // Sessions on record but no tmux server: they are gone or tmux is broken.
        Tmux::NoServer => registered_sessions == 0,
        Tmux::Unreachable => false,
    };
    if !tmux_ok {
        out.push("tmux_unreachable");
    }
    out
}

/// Whether a file can be created, written, synced and removed in `dir`.
pub(crate) fn writable(dir: &Path) -> bool {
    use std::io::Write;
    let probe = dir.join(format!(".readyz-{}", std::process::id()));
    let written = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&probe)
        .and_then(|mut f| f.write_all(b"ok").and_then(|_| f.sync_data()))
        .is_ok();
    let removed = std::fs::remove_file(&probe).is_ok();
    written && removed
}

/// What a tmux client's answer to `has-session` for a name nobody uses means.
pub(crate) fn classify(output: std::io::Result<std::process::Output>) -> Tmux {
    let Ok(out) = output else { return Tmux::Unreachable };
    let err = String::from_utf8_lossy(&out.stderr);
    if out.status.success() || err.contains("can't find session") {
        Tmux::Reachable
    } else if err.contains("no server running") || err.contains("error connecting to") {
        Tmux::NoServer
    } else {
        Tmux::Unreachable
    }
}

#[cfg(not(test))]
fn probe_tmux() -> Tmux {
    use crate::bounded_command::{deadline, OutputWithin};
    classify(
        crate::terminal::session::tmux()
            .env("LC_ALL", "C")
            .args(["has-session", "-t", "=nabiman-readiness-probe"])
            .output_within(deadline::TMUX),
    )
}

/// Tests never reach a real tmux (the production socket may be this user's).
#[cfg(test)]
fn probe_tmux() -> Tmux {
    *tests::FAKE_TMUX.lock_safe()
}

/// The last tmux probe, shared with the self-checks (`None`: never probed).
static LAST_TMUX: Mutex<Option<(Instant, Tmux)>> = Mutex::new(None);

/// The tmux state, probed at most once per `CACHE_FOR`.
pub(crate) fn tmux_state() -> Tmux {
    if let Some((at, state)) = *LAST_TMUX.lock_safe() {
        if at.elapsed() < CACHE_FOR {
            return state;
        }
    }
    let state = probe_tmux();
    *LAST_TMUX.lock_safe() = Some((Instant::now(), state));
    state
}

static LAST_ANSWER: Mutex<Option<(Instant, Vec<&'static str>)>> = Mutex::new(None);

pub(crate) async fn healthz() -> HttpResponse {
    HttpResponse::Ok().content_type("text/plain; charset=utf-8").insert_header(("Cache-Control", "no-store")).body("ok")
}

pub(crate) async fn readyz(
    audit: web::Data<crate::audit::AuditLog>,
    sessions: web::Data<crate::terminal::PtyStore>,
) -> HttpResponse {
    let cached = LAST_ANSWER.lock_safe().as_ref().filter(|(at, _)| at.elapsed() < CACHE_FOR).map(|(_, r)| r.clone());
    let found = match cached {
        Some(found) => found,
        None => {
            let audit_ok = audit.lock_safe().error.is_none();
            let registered = sessions.lock_safe().len();
            let found = web::block(move || {
                reasons(writable(&nabiman_server::data_dir()), audit_ok, tmux_state(), registered)
            })
            .await
            .unwrap_or_else(|_| vec!["probe_failed"]);
            *LAST_ANSWER.lock_safe() = Some((Instant::now(), found.clone()));
            found
        }
    };
    let mut answer = if found.is_empty() { HttpResponse::Ok() } else { HttpResponse::ServiceUnavailable() };
    let status = if found.is_empty() { "ready" } else { "not_ready" };
    answer
        .insert_header(("Cache-Control", "no-store"))
        .json(serde_json::json!({ "status": status, "reasons": found }))
}

#[cfg(test)]
#[path = "health_tests.rs"]
pub(crate) mod tests;
