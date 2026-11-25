//! `GET …/sessions/{id}/cwd`: the folder the shell of a session on this
//! machine is in now, so a file name printed there can be found.
//!
//! tmux knows it for the program in the foreground of the pane. When that
//! program runs a terminal of its own (`su --pty`, as a "＋ Claude 세션" tile
//! starts, or `sudo`, `script`), what the person works in is in the foreground
//! of that inner terminal, and it is followed there.
//!
//! A session to another machine has no folder to report: its pane runs the
//! ssh or telnet client, whose folder is this server's, not the remote shell's.

use crate::bounded_command::{deadline, OutputWithin};
use crate::lock_safe::LockExt;
use actix_web::{web, HttpRequest, HttpResponse};

use super::super::session::{exact_session, tmux, PtyStore};

/// Longest path reported (Linux's own limit).
const MAX_PATH: usize = 4096;
/// Terminals inside terminals followed at most.
const MAX_NESTED: usize = 4;

#[derive(serde::Serialize)]
struct CwdInfo {
    cwd: Option<String>,
}

/// What a session's folder is asked of.
#[derive(Debug, PartialEq)]
pub(super) enum Target {
    /// A shell on this machine, by its tmux session.
    Local(String),
    /// An SSH or Telnet session: no folder is reported.
    Remote,
}

/// The session `id` as `user_id` may ask about it: exactly the sessions whose
/// Claude header (`…/claude`) the account may read.
pub(super) fn locate(store: &PtyStore, id: &str, user_id: &str, users: &crate::users::UserStore) -> Option<Target> {
    let map = store.lock_safe();
    let session = map.get(id)?;
    if !super::super::session::visible_to(session, user_id, users) || !super::super::remote_access::allowed(session) {
        return None;
    }
    Some(match session.remote_binding {
        Some(_) => Target::Remote,
        None => Target::Local(session.tmux_name.clone()),
    })
}

/// An absolute path fit to hand out, or nothing.
fn clean(path: &str) -> Option<String> {
    let path = path.trim_end_matches(['\r', '\n']);
    // The kernel marks a folder that was removed under the process.
    let gone = path.ends_with(" (deleted)") && !std::path::Path::new(path).is_dir();
    (path.starts_with('/') && path.len() <= MAX_PATH && !path.chars().any(char::is_control) && !gone).then(|| path.to_string())
}

/// One process, as `/proc/<pid>/stat` has it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Proc {
    pub pid: i64,
    pub ppid: i64,
    pub pgrp: i64,
    pub sid: i64,
    pub tty: i64,
    /// The foreground process group of its terminal.
    pub tpgid: i64,
}

pub(super) fn parse_stat(pid: i64, stat: &str) -> Option<Proc> {
    // The command name (field 2) may hold spaces and parentheses.
    let mut rest = stat[stat.rfind(')')? + 1..].split_whitespace().skip(1).map(|f| f.parse::<i64>().ok());
    Some(Proc { pid, ppid: rest.next()??, pgrp: rest.next()??, sid: rest.next()??, tty: rest.next()??, tpgid: rest.next()?? })
}

fn processes() -> Vec<Proc> {
    std::fs::read_dir("/proc")
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let pid = entry.file_name().to_str()?.parse::<i64>().ok()?;
            parse_stat(pid, &std::fs::read_to_string(entry.path().join("stat")).ok()?)
        })
        .collect()
}

/// The process group in the foreground of the innermost terminal under the
/// pane whose first process is `pane_pid`, and how many terminals deep it is.
pub(super) fn foreground(pane_pid: i64, procs: &[Proc]) -> Option<(i64, usize)> {
    let pane = procs.iter().find(|p| p.pid == pane_pid)?;
    let (mut group, mut tty, mut depth) = (pane.tpgid, pane.tty, 0);
    while depth < MAX_NESTED {
        // A session leader on another terminal, started by the foreground program.
        let mut inner = procs.iter().filter(|c| {
            c.sid == c.pid && c.tty != 0 && c.tty != tty && procs.iter().any(|p| p.pid == c.ppid && p.pgrp == group && p.tty == tty)
        });
        match (inner.next(), inner.next()) {
            (Some(leader), None) if leader.tpgid > 0 => {
                group = leader.tpgid;
                tty = leader.tty;
                depth += 1;
            }
            _ => break,
        }
    }
    (group > 0).then_some((group, depth))
}

/// The folder the foreground of the active pane of tmux session `name` is in.
pub(super) fn pane_cwd(name: &str) -> Option<String> {
    let out = tmux()
        .args(["display-message", "-p", "-t", &format!("{}:", exact_session(name)), "#{pane_pid} #{pane_current_path}"])
        .output_within(deadline::TMUX)
        .ok()
        .filter(|o| o.status.success())?;
    let text = String::from_utf8_lossy(&out.stdout);
    let (pid, path) = text.split_once(' ')?;
    let procs = processes();
    match pid.parse().ok().and_then(|pid| foreground(pid, &procs)) {
        // A terminal inside the pane: tmux's answer is the folder of the
        // program that opened it, not of the shell in it.
        Some((group, depth)) if depth > 0 => {
            let leader = std::fs::read_link(format!("/proc/{group}/cwd")).ok()?;
            clean(leader.to_str()?)
        }
        _ => clean(path),
    }
}

/// `GET …/sessions/{id}/cwd`. Reading only; nothing is changed or recorded.
pub(super) async fn session_cwd(
    path: web::Path<String>,
    store: web::Data<PtyStore>,
    req: HttpRequest,
    users: web::Data<crate::users::UserStore>,
) -> HttpResponse {
    let user_id = crate::auth::verified_user_id(&req).unwrap_or_default();
    answer(locate(store.get_ref(), &path.into_inner(), &user_id, &users)).await
}

/// A session that is not there and one the account may not see answer alike.
pub(super) async fn answer(target: Option<Target>) -> HttpResponse {
    let cwd = match target {
        None => return HttpResponse::NotFound().json(crate::models::ApiResponse::<()>::error("Session not found")),
        Some(Target::Remote) => None,
        // tmux is asked off the worker, within its deadline.
        Some(Target::Local(name)) => web::block(move || pane_cwd(&name)).await.ok().flatten(),
    };
    HttpResponse::Ok().json(crate::models::ApiResponse::ok(CwdInfo { cwd }))
}

#[cfg(test)]
#[path = "session_cwd_tests.rs"]
mod tests;
