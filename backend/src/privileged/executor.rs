use super::protocol::{ServiceAction, State};
use std::{
    os::unix::process::CommandExt,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

pub(crate) fn run(unit: &str, action: ServiceAction) -> (State, Option<i32>) {
    execute(
        Command::new("/usr/bin/systemctl")
            .args([
                "--no-ask-password",
                "--no-pager",
                action.as_str(),
                "--",
                unit,
            ])
            .env_clear()
            .env("PATH", "/usr/sbin:/usr/bin:/sbin:/bin")
            .env("LC_ALL", "C"),
        Duration::from_secs(20),
    )
}
pub(crate) fn execute(command: &mut Command, timeout: Duration) -> (State, Option<i32>) {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    unsafe {
        command.pre_exec(|| {
            if libc::setpgid(0, 0) == -1 {
                Err(std::io::Error::last_os_error())
            } else {
                Ok(())
            }
        });
    }
    let mut child = match command.spawn() {
        Ok(value) => value,
        Err(_) => return (State::NotDispatched, None),
    };
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return (
                    if status.success() {
                        State::CommandSucceeded
                    } else {
                        State::CommandFailed
                    },
                    status.code(),
                )
            }
            Ok(None) if start.elapsed() < timeout => std::thread::sleep(Duration::from_millis(10)),
            _ => {
                // Killing the client does not cancel a job already accepted by systemd.
                unsafe {
                    libc::kill(-(child.id() as i32), libc::SIGKILL);
                }
                let _ = child.kill();
                let _ = child.wait();
                return (State::UnknownResult, None);
            }
        }
    }
}
