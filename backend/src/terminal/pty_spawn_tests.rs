use super::*;
use std::os::fd::{FromRawFd, OwnedFd};

struct TestChild {
    master: OwnedFd,
    pid: nix::unistd::Pid,
}
impl Drop for TestChild {
    fn drop(&mut self) {
        let _ = nix::sys::signal::kill(self.pid, nix::sys::signal::Signal::SIGKILL);
        let _ = nix::sys::wait::waitpid(self.pid, None);
    }
}
fn spawn(program: &str, args: &[&str]) -> TestChild {
    let (fd, pid) = spawn_pty(
        ShellCommand {
            program: program.into(),
            args: args.iter().map(|s| s.to_string()).collect(),
        },
        (91, 27),
    )
    .unwrap();
    TestChild {
        master: unsafe { OwnedFd::from_raw_fd(fd) },
        pid,
    }
}

#[test]
fn terminal_does_not_keep_unrelated_command_pipe_open() {
    // Command::output briefly owns such a pipe writer in the parent.
    // A concurrent terminal fork must not keep it alive until disconnect.
    let (reader, writer) = nix::unistd::pipe2(nix::fcntl::OFlag::O_CLOEXEC).unwrap();
    let _child = spawn("/bin/sleep", &["30"]);
    drop(writer);
    let mut event = libc::pollfd {
        fd: reader.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    assert_eq!(
        unsafe { libc::poll(&mut event, 1, 1000) },
        1,
        "terminal retained another command's pipe writer"
    );
    let mut file = std::fs::File::from(reader);
    assert_eq!(file.read(&mut [0u8; 1]).unwrap(), 0);
}

#[test]
fn terminal_has_controlling_tty_environment_and_requested_size() {
    let child = spawn("/bin/sh", &["-c",
            "test -t 0 && test -t 1 && test -t 2 && test -r /dev/tty && printf 'PTY_OK:%s:%s:%s\\n' \"$TERM\" \"$COLORTERM\" \"$LANG\"; stty size; sleep 30"]);
    let mut file = std::fs::File::from(child.master.try_clone().unwrap());
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    let mut output = Vec::new();
    while std::time::Instant::now() < deadline {
        let mut buf = [0u8; 1024];
        match file.read(&mut buf) {
            Ok(n) => output.extend_from_slice(&buf[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(e) => panic!("PTY read failed: {e}"),
        }
        let text = String::from_utf8_lossy(&output);
        if text.contains("PTY_OK:xterm-256color:truecolor:en_US.UTF-8") && text.contains("27 91") {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("missing PTY setup: {}", String::from_utf8_lossy(&output));
}

#[test]
fn missing_terminal_program_returns_spawn_error() {
    let result = spawn_pty(
        ShellCommand {
            program: "/nonexistent/nabiman-test-command".into(),
            args: vec![],
        },
        (80, 24),
    );
    assert!(result.is_err(), "exec failure must reach the caller");
}

/// A process's state letter in /proc (Z: a zombie), or None once it is gone.
fn state(pid: nix::unistd::Pid) -> Option<char> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    stat.rsplit(')').next()?.trim().chars().next()
}

#[test]
fn a_closed_attachment_leaves_no_zombie() {
    let child = |script: &str| {
        // Waited for by the reaper under test, not here.
        #[allow(clippy::zombie_processes)]
        let c = std::process::Command::new("sh").args(["-c", script]).spawn().unwrap();
        nix::unistd::Pid::from_raw(c.id() as i32)
    };
    // Exits on its own: collected once it does.
    let quick = child("sleep 0.2");
    spawn::reap_within(quick, std::time::Duration::from_secs(5));
    // Ignores hangups and lingers: ended after the grace period, then collected.
    let stubborn = child("trap '' HUP TERM; sleep 30");
    spawn::reap_within(stubborn, std::time::Duration::from_millis(300));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while (state(quick).is_some() || state(stubborn).is_some()) && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert_eq!((state(quick), state(stubborn)), (None, None), "both are gone, not zombies");
}
