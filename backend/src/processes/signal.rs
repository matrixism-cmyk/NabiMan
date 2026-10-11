use super::identity;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
pub(super) fn open(pid: u32) -> Result<OwnedFd, String> {
    if pid < 2 || pid > i32::MAX as u32 {
        return Err("Protected or invalid PID".into());
    }
    let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid as libc::pid_t, 0u32) };
    if fd < 0 {
        return Err("Stable process handle unavailable; no numeric-PID fallback".into());
    }
    Ok(unsafe { OwnedFd::from_raw_fd(fd as i32) })
}
pub(super) fn available() -> bool {
    let Ok(fd) = open(std::process::id()) else {
        return false;
    };
    unsafe {
        libc::syscall(
            libc::SYS_pidfd_send_signal,
            fd.as_raw_fd(),
            0,
            std::ptr::null::<libc::siginfo_t>(),
            0u32,
        ) == 0
    }
}
#[derive(serde::Serialize)]
pub(super) struct Receipt {
    pub pid: u32,
    pub signal: String,
    pub accepted: bool,
    pub observation: String,
}
pub(super) fn send(
    pid: u32,
    expected: &str,
    name: &str,
    authorize: impl FnOnce() -> bool,
) -> Result<Receipt, String> {
    let number = match name {
        "TERM" | "15" => libc::SIGTERM,
        "KILL" | "9" => libc::SIGKILL,
        "HUP" | "1" => libc::SIGHUP,
        "INT" | "2" => libc::SIGINT,
        "STOP" | "19" => libc::SIGSTOP,
        "CONT" | "18" => libc::SIGCONT,
        _ => return Err("Unsupported signal".into()),
    };
    if identity::protected()?.contains(&pid) {
        return Err("Management process, ancestor or system PID is protected".into());
    }
    let fd = open(pid)?;
    if identity::inspect(pid)?.value != expected {
        return Err("Process identity changed after review; refresh and select again".into());
    }
    if !authorize() {
        return Err("Authorization changed before dispatch; no signal sent".into());
    }
    let sent = unsafe {
        libc::syscall(
            libc::SYS_pidfd_send_signal,
            fd.as_raw_fd(),
            number,
            std::ptr::null::<libc::siginfo_t>(),
            0u32,
        )
    };
    if sent != 0 {
        return Err(
            "Kernel did not accept the signal; process may have exited or access was denied".into(),
        );
    }
    let mut poll = libc::pollfd {
        fd: fd.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    let ready = unsafe { libc::poll(&mut poll, 1, 100) };
    let observation = if ready < 0 || poll.revents & (libc::POLLNVAL | libc::POLLERR) != 0 {
        "unknown"
    } else if poll.revents & (libc::POLLIN | libc::POLLHUP) != 0 {
        "exited"
    } else {
        "still_present"
    };
    Ok(Receipt {
        pid,
        signal: name.into(),
        accepted: true,
        observation: observation.into(),
    })
}
