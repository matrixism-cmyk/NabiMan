use super::{
    executor,
    policy::{validate_parent, Policy},
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
    if unsafe { libc::geteuid() } != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Helper requires root; API must use a separate unprivileged UID",
        ));
    }
    let policy = Policy::load(policy_path)?;
    let mut store = Store::open(ledger_path)?;
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
        handle(
            socket,
            &policy,
            expected.as_ref(),
            &mut store,
            |unit, action| {
                let current = super::identity::load(socket_path).ok();
                let unchanged = current.is_some()
                    && current == expected
                    && current.as_ref().is_some_and(|value| {
                        super::identity::capture(value.pid, policy.api_uid)
                            .ok()
                            .as_ref()
                            == Some(value)
                    });
                if unchanged && policy.still_approved(policy_path) {
                    executor::run(unit, action)
                } else {
                    (State::NotDispatched, None)
                }
            },
        )?;
    }
    Ok(())
}
pub(crate) fn handle<F>(
    mut socket: UnixStream,
    policy: &Policy,
    expected: Option<&super::identity::ProcessIdentity>,
    store: &mut Store,
    run: F,
) -> io::Result<()>
where
    F: FnOnce(&str, protocol::ServiceAction) -> (State, Option<i32>),
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
        Err(_) => return Ok(()),
    };
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
        let _ = protocol::write_frame(&mut socket, &reply);
        return Ok(());
    }
    let remaining = request
        .expires_at
        .checked_sub(chrono::Utc::now().timestamp());
    if !remaining.is_some_and(|value| (1..=3).contains(&value)) {
        reply.status = Status::Expired;
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
    // A disconnected caller cannot cause an already committed command to replay.
    let _ = protocol::write_frame(&mut socket, &reply);
    Ok(())
}
