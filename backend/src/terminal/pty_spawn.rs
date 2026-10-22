use super::*;

pub(super) fn spawn_pty(
    cmd: ShellCommand,
    size: (u16, u16),
) -> Result<(i32, nix::unistd::Pid), String> {
    use std::os::fd::IntoRawFd;
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};

    let winsize = nix::pty::Winsize {
        ws_row: size.1,
        ws_col: size.0,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    let pty = nix::pty::openpty(Some(&winsize), None).map_err(|e| format!("openpty: {}", e))?;
    // No long-lived fork of the server: it would retain unrelated workers'
    // pipe writers and prevent Command::output from ever observing EOF.
    // Command prepares argv/environment before fork and execs the child directly.
    for fd in [pty.master.as_raw_fd(), pty.slave.as_raw_fd()] {
        nix::fcntl::fcntl(
            fd,
            nix::fcntl::FcntlArg::F_SETFD(nix::fcntl::FdFlag::FD_CLOEXEC),
        )
        .map_err(|e| format!("fcntl cloexec: {}", e))?;
    }
    let flags = nix::fcntl::fcntl(pty.master.as_raw_fd(), nix::fcntl::FcntlArg::F_GETFL)
        .map_err(|e| format!("fcntl: {}", e))?;
    nix::fcntl::fcntl(
        pty.master.as_raw_fd(),
        nix::fcntl::FcntlArg::F_SETFL(
            nix::fcntl::OFlag::from_bits_truncate(flags) | nix::fcntl::OFlag::O_NONBLOCK,
        ),
    )
    .map_err(|e| format!("fcntl nonblock: {}", e))?;

    let slave = std::fs::File::from(pty.slave);
    let stdin = slave.try_clone().map_err(|e| format!("clone PTY: {}", e))?;
    let stdout = slave.try_clone().map_err(|e| format!("clone PTY: {}", e))?;
    let mut command = Command::new(&cmd.program);
    command
        .args(&cmd.args)
        .env_remove("TMUX")
        .env_remove("TMUX_PANE")
        .env("TERM", "xterm-256color")
        .env("COLORTERM", "truecolor")
        .env("LANG", "en_US.UTF-8")
        .stdin(Stdio::from(stdin))
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(slave));
    // SAFETY: stdio has already been installed by Command. Only system calls
    // and errno conversion are allowed here: no allocation, env access, locks,
    // or nested Command::spawn in the post-fork child.
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(std::io::Error::last_os_error());
            }
            if libc::ioctl(0, libc::TIOCSCTTY, 0) == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let child = command.spawn().map_err(|e| format!("spawn PTY: {}", e))?;
    Ok((
        pty.master.into_raw_fd(),
        nix::unistd::Pid::from_raw(child.id() as i32),
    ))
}

/// How long a closed attachment's `tmux attach` gets to exit on its own.
const REAP_GRACE: std::time::Duration = std::time::Duration::from_secs(5);

/// Collects the `tmux attach` of an attachment that just closed. Its PTY is
/// gone, so it exits within moments; until it is waited for it stays a
/// zombie, and one was left behind by every disconnect. Waits on a thread of
/// its own, and ends a client that lingers — only the client: the tmux
/// session and the shell in it go on.
pub(crate) fn reap_later(pid: nix::unistd::Pid) {
    reap_within(pid, REAP_GRACE);
}

pub(super) fn reap_within(pid: nix::unistd::Pid, grace: std::time::Duration) {
    use nix::sys::wait::{waitpid, WaitPidFlag, WaitStatus};
    let reaper = move || {
        let deadline = std::time::Instant::now() + grace;
        while std::time::Instant::now() < deadline {
            match waitpid(pid, Some(WaitPidFlag::WNOHANG)) {
                Ok(WaitStatus::StillAlive) => std::thread::sleep(std::time::Duration::from_millis(50)),
                // Exited (and now collected), or not ours to wait for.
                _ => return,
            }
        }
        let _ = nix::sys::signal::kill(pid, nix::sys::signal::Signal::SIGKILL);
        let _ = waitpid(pid, None);
    };
    if std::thread::Builder::new().name("pty-reaper".into()).spawn(reaper).is_err() {
        // No thread to spare: at least collect it if it has already exited.
        let _ = waitpid(pid, Some(WaitPidFlag::WNOHANG));
    }
}
