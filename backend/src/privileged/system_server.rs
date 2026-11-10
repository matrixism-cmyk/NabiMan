//! The helper's side of a system operation: the same admission as service
//! control (kernel peer uid, pinned API process, version, request id, actor,
//! a deadline at most 3 s away), then the system policy and the argument
//! checks, an audit line before anything runs, and one typed reply.
use super::{
    audit::{Audit, Entry},
    protocol::{self, Operation, Request, Status, VERSION},
    system_ops::{Outcome, SystemPolicy, SystemReply},
};
use std::{
    io,
    os::unix::net::UnixStream,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

fn lock(audit: &Mutex<Audit>) -> MutexGuard<'_, Audit> {
    audit.lock().unwrap_or_else(PoisonError::into_inner)
}

pub(crate) fn valid_actor(actor: &str) -> bool {
    !actor.is_empty() && actor.len() <= 128 && actor.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
}

/// An audit entry for a request that was read.
pub(crate) fn asked(uid: u32, pid: i32, request: &Request, decision: &'static str) -> Entry {
    let mut entry = Entry::new(uid, pid, decision);
    entry.request_id = Some(request.request_id.chars().take(64).collect());
    entry.actor_id = Some(request.actor_id.chars().take(128).collect());
    entry.operation = serde_json::to_value(&request.operation).ok();
    entry
}

/// Best effort (service control keeps its durable ledger either way).
pub(crate) fn note(audit: &Arc<Mutex<Audit>>, entry: Entry) {
    let _ = lock(audit).record(entry);
}

fn answer(socket: &mut UnixStream, reply: &SystemReply) {
    let _ = protocol::write_frame_within(socket, reply, protocol::MAX_REPLY);
}

/// A permitted system operation, handed to whoever runs it; `finish` answers it.
pub(crate) struct SystemJob {
    pub operation: Operation,
    request_id: String,
    socket: UnixStream,
    audit: Arc<Mutex<Audit>>,
    entry: Entry,
}
impl SystemJob {
    fn reply(&self, status: Status, outcome: Option<Outcome>, reason: Option<&str>) -> SystemReply {
        SystemReply { version: VERSION, request_id: self.request_id.clone(), status, outcome, reason: reason.map(String::from) }
    }
    pub(crate) fn finish(mut self, outcome: Outcome) {
        let mut entry = self.entry.clone();
        entry.decision = "completed";
        entry.state = Some(outcome.state.as_str());
        entry.exit_code = outcome.exit_code;
        entry.at = chrono::Utc::now().to_rfc3339();
        note(&self.audit, entry);
        let reply = self.reply(Status::Completed, Some(outcome), None);
        answer(&mut self.socket, &reply);
    }
    /// Not run (another one is running): answered without a command.
    pub(crate) fn refuse(mut self, status: Status, reason: &str) {
        let mut entry = self.entry.clone();
        entry.decision = "busy";
        note(&self.audit, entry);
        let reply = self.reply(status, None, Some(reason));
        answer(&mut self.socket, &reply);
    }
}

pub(crate) fn system_request<S>(
    mut socket: UnixStream,
    request: Request,
    uid: u32,
    pid: i32,
    policy: &SystemPolicy,
    audit: &Arc<Mutex<Audit>>,
    system: S,
) -> io::Result<()>
where
    S: FnOnce(SystemJob),
{
    let entry = |decision| asked(uid, pid, &request, decision);
    let refuse = |socket: &mut UnixStream, status, decision, reason: &str| {
        note(audit, entry(decision));
        let reply = SystemReply { version: VERSION, request_id: request.request_id.clone(), status, outcome: None, reason: Some(reason.into()) };
        answer(socket, &reply);
        Ok(())
    };
    if request.version != VERSION || !protocol::is_id(&request.request_id) || !valid_actor(&request.actor_id) {
        return refuse(&mut socket, Status::Rejected, "invalid", "Invalid request");
    }
    let remaining = request.expires_at.checked_sub(chrono::Utc::now().timestamp());
    if !remaining.is_some_and(|value| (1..=3).contains(&value)) {
        return refuse(&mut socket, Status::Expired, "expired", "Request expired before helper dispatch");
    }
    if !policy.permits(uid, &request.operation) {
        return refuse(&mut socket, Status::Rejected, "denied",
            "Not approved by the helper's system policy (privileged-system.json), or invalid arguments");
    }
    let dispatched = entry("dispatched");
    // No audit line, no command.
    if lock(audit).record(dispatched.clone()).is_err() {
        return refuse(&mut socket, Status::Rejected, "denied", "Helper audit unavailable; nothing was run");
    }
    system(SystemJob { operation: request.operation, request_id: request.request_id, socket, audit: audit.clone(), entry: dispatched });
    Ok(())
}
