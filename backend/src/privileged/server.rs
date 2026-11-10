use super::{
    audit::{Audit, Entry},
    executor,
    policy::{validate_parent, Policy},
    system_ops::{self, is_system, Outcome, SystemPolicy},
    system_server::{asked, note, system_request, SystemJob},
    protocol::{self, Operation, Reply, Request, State, Status, VERSION},
    store::Store,
};
use std::{
    fs, io,
    os::{
        fd::AsRawFd,
        unix::{
            fs::{FileTypeExt, MetadataExt, PermissionsExt},
            net::{UnixListener, UnixStream},
        },
    },
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

pub fn peer_uid(socket: &UnixStream) -> io::Result<u32> {
    Ok(peer_credentials(socket)?.uid)
}
fn peer_credentials(socket: &UnixStream) -> io::Result<libc::ucred> {
    let mut credentials: libc::ucred = unsafe { std::mem::zeroed() };
    let mut size = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    if unsafe {
        libc::getsockopt(
            socket.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&mut credentials as *mut libc::ucred).cast(),
            &mut size,
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    if size as usize != std::mem::size_of::<libc::ucred>() {
        return Err(io::Error::other("Missing peer credentials"));
    }
    Ok(credentials)
}
pub fn run(policy_path: &Path, socket_path: &Path, ledger_path: &Path) -> io::Result<()> {
    serve(policy_path, &policy_path.with_file_name("privileged-system.json"), socket_path, ledger_path)
}
/// The helper: service control from `policy_path`, the system operations
/// `system_path` approves (none when the file is missing), one audit line per call.
pub fn serve(policy_path: &Path, system_path: &Path, socket_path: &Path, ledger_path: &Path) -> io::Result<()> {
    if unsafe { libc::geteuid() } != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Helper requires root; API must use a separate unprivileged UID",
        ));
    }
    let policy = Policy::load(policy_path)?;
    let system_policy = SystemPolicy::load(system_path, policy.api_uid)?;
    let mut store = Store::open(ledger_path)?;
    let audit = Arc::new(Mutex::new(Audit::beside(ledger_path)?));
    // At most one system operation runs (on its own thread), so a long upgrade
    // does not hold up service control; another one meanwhile is refused.
    let busy = Arc::new(AtomicBool::new(false));
    validate_parent(socket_path)?;
    match fs::symlink_metadata(socket_path) {
        Ok(metadata) => {
            if !metadata.file_type().is_socket() || metadata.uid() != policy.api_uid {
                return Err(io::Error::other("Unsafe existing helper socket"));
            }
            match UnixStream::connect(socket_path) {
                Err(error) if error.kind() == io::ErrorKind::ConnectionRefused => {
                    fs::remove_file(socket_path)?
                }
                _ => {
                    return Err(io::Error::other(
                        "Existing helper socket is not confirmed stale",
                    ))
                }
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    // The parent directory is root-owned and not writable by the API.
    unsafe {
        libc::umask(0o077);
    }
    let listener = UnixListener::bind(socket_path)?;
    fs::set_permissions(socket_path, fs::Permissions::from_mode(0o600))?;
    let path = std::ffi::CString::new(socket_path.as_os_str().as_encoded_bytes())
        .map_err(io::Error::other)?;
    if unsafe { libc::chown(path.as_ptr(), policy.api_uid, !0 as libc::gid_t) } != 0 {
        return Err(io::Error::last_os_error());
    }
    for socket in listener.incoming() {
        let socket = socket?;
        // One bounded request/command at a time. Never spawn unlimited threads.
        let expected = super::identity::load(socket_path).ok();
        let still_pinned = || {
            let current = super::identity::load(socket_path).ok();
            current.is_some()
                && current == expected
                && current.as_ref().is_some_and(|value| {
                    super::identity::capture(value.pid, policy.api_uid)
                        .ok()
                        .as_ref()
                        == Some(value)
                })
        };
        let system = |job: SystemJob| {
            if busy.swap(true, Ordering::SeqCst) {
                return job.refuse(Status::Conflict, "Another system operation is running; try again when it ends");
            }
            // Rechecked right before the command, as for service control.
            let approved = still_pinned() && policy.still_approved(policy_path) && system_policy.still_approved(system_path);
            let flag = busy.clone();
            let worker = std::thread::Builder::new().name("system-op".into()).spawn(move || {
                let outcome = match system_ops::plan(&job.operation, &system_ops::Tools::detect()) {
                    Ok(plan) if approved => system_ops::execute(&plan),
                    Ok(plan) => Outcome { state: State::NotDispatched, exit_code: None, backend: Some(plan.backend.into()), output: String::new(), truncated: false },
                    Err(why) => Outcome { state: State::NotDispatched, exit_code: None, backend: None, output: why, truncated: false },
                };
                job.finish(outcome);
                flag.store(false, Ordering::SeqCst);
            });
            if worker.is_err() {
                busy.store(false, Ordering::SeqCst);
            }
        };
        handle_with(
            socket,
            &policy,
            &system_policy,
            expected.as_ref(),
            &mut store,
            &audit,
            |unit, action| {
                if still_pinned() && policy.still_approved(policy_path) {
                    executor::run(unit, action)
                } else {
                    (State::NotDispatched, None)
                }
            },
            system,
        )?;
    }
    Ok(())
}
/// Service control only (no system operations, no audit): what the ledger tests drive.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn handle<F>(
    socket: UnixStream,
    policy: &Policy,
    expected: Option<&super::identity::ProcessIdentity>,
    store: &mut Store,
    run: F,
) -> io::Result<()>
where
    F: FnOnce(&str, protocol::ServiceAction) -> (State, Option<i32>),
{
    let none = SystemPolicy::none(policy.api_uid);
    let audit = Arc::new(Mutex::new(Audit::Off));
    handle_with(socket, policy, &none, expected, store, &audit, run, |_| {})
}
/// `handle`, with the system operations (`system` gets each permitted one to
/// run, inline or on a worker, and answers it) and the audit trail.
#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_with<F, S>(
    mut socket: UnixStream,
    policy: &Policy,
    system_policy: &SystemPolicy,
    expected: Option<&super::identity::ProcessIdentity>,
    store: &mut Store,
    audit: &Arc<Mutex<Audit>>,
    run: F,
    system: S,
) -> io::Result<()>
where
    F: FnOnce(&str, protocol::ServiceAction) -> (State, Option<i32>),
    S: FnOnce(SystemJob),
{
    socket.set_read_timeout(Some(Duration::from_secs(2)))?;
    socket.set_write_timeout(Some(Duration::from_secs(2)))?;
    let peer = peer_credentials(&socket)?;
    let uid = peer.uid;
    let pinned = expected.is_some_and(|value| {
        peer.pid > 0
            && value.pid == peer.pid as u32
            && super::identity::capture(value.pid, uid).ok().as_ref() == Some(value)
    });
    if uid != policy.api_uid || !pinned {
        note(audit, Entry::new(uid, peer.pid, "peer_rejected"));
        let _ = protocol::write_frame(
            &mut socket,
            &Reply {
                version: VERSION,
                request_id: String::new(),
                status: Status::Rejected,
                receipt: None,
            },
        );
        return Ok(());
    }
    let request: Request = match protocol::read_frame(&mut socket) {
        Ok(value) => value,
        Err(_) => {
            note(audit, Entry::new(uid, peer.pid, "unreadable"));
            return Ok(());
        }
    };
    if is_system(&request.operation) {
        return system_request(socket, request, uid, peer.pid, system_policy, audit, system);
    }
    let entry = |decision| asked(uid, peer.pid, &request, decision);
    let mut reply = Reply {
        version: VERSION,
        request_id: request.request_id.clone(),
        status: Status::Rejected,
        receipt: None,
    };
    let valid_actor = !request.actor_id.is_empty()
        && request.actor_id.len() <= 128
        && request
            .actor_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b));
    if request.version != VERSION || !protocol::is_id(&request.request_id) || !valid_actor {
        note(audit, entry("invalid"));
        let _ = protocol::write_frame(&mut socket, &reply);
        return Ok(());
    }
    let remaining = request
        .expires_at
        .checked_sub(chrono::Utc::now().timestamp());
    if !remaining.is_some_and(|value| (1..=3).contains(&value)) {
        reply.status = Status::Expired;
        note(audit, entry("expired"));
        let _ = protocol::write_frame(&mut socket, &reply);
        return Ok(());
    }
    match &request.operation {
        Operation::Execute { unit, action } if policy.permits(uid, unit, *action) => {
            if let Some(receipt) = store.blocked(unit)? {
                if receipt.actor_id == request.actor_id {
                    reply.receipt = Some(receipt);
                    reply.status = Status::Conflict;
                }
            } else if store.begin(&request, unit, *action, policy)? {
                let (state, code) = if chrono::Utc::now().timestamp() >= request.expires_at {
                    (State::NotDispatched, None)
                } else {
                    run(unit, *action)
                };
                reply.receipt = Some(store.finish(&request.request_id, state, code)?);
                reply.status = Status::Completed;
            } else {
                if let Some(receipt) = store.get(&request.request_id)? {
                    if receipt.actor_id == request.actor_id {
                        reply.receipt = Some(receipt);
                        reply.status = Status::Conflict;
                    }
                }
            }
        }
        Operation::Inspect { operation_id } if protocol::is_id(operation_id) => {
            match store.get(operation_id)? {
                Some(receipt) if receipt.actor_id == request.actor_id => {
                    reply.status = Status::Completed;
                    reply.receipt = Some(receipt);
                }
                _ => reply.status = Status::NotFound,
            }
        }
        _ => {}
    }
    let mut done = entry(match reply.status {
        Status::Completed => "completed",
        Status::Conflict => "conflict",
        Status::NotFound => "not_found",
        Status::Expired => "expired",
        Status::Rejected => "denied",
    });
    done.state = reply.receipt.as_ref().map(|r| r.state.as_str());
    done.exit_code = reply.receipt.as_ref().and_then(|r| r.exit_code);
    note(audit, done);
    // A disconnected caller cannot cause an already committed command to replay.
    let _ = protocol::write_frame(&mut socket, &reply);
    Ok(())
}
