//! Bounded nonblocking pipes; the password/command content is never reflected in errors.
use std::{
    io::{Read, Write},
    os::{fd::AsRawFd, unix::process::CommandExt},
    path::Path,
    process::{Command, ExitStatus, Stdio},
    time::{Duration, Instant},
};
pub(crate) struct Output {
    pub status: ExitStatus,
    pub out: String,
    pub err: String,
}
pub(crate) fn run(
    program: &Path,
    args: &[&str],
    input: Option<&str>,
    timeout: Duration,
) -> Result<Output, String> {
    let mut child = Command::new(program)
        .args(args)
        .env_clear()
        .env("PATH", "/usr/sbin:/usr/bin:/sbin:/bin")
        .env("LC_ALL", "C")
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()
        .map_err(|_| "Command tool could not start; no change dispatched")?;
    let result = (|| {
        let mut stdin = child.stdin.take();
        let mut stdout = child.stdout.take().ok_or("Missing tool output")?;
        let mut stderr = child.stderr.take().ok_or("Missing tool diagnostics")?;
        let mut fds = vec![stdout.as_raw_fd(), stderr.as_raw_fd()];
        if let Some(s) = &stdin {
            fds.push(s.as_raw_fd());
        }
        for fd in fds {
            unsafe {
                let flags = libc::fcntl(fd, libc::F_GETFL);
                if flags < 0 || libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) < 0 {
                    return Err("Cannot bound command pipes".into());
                }
            }
        }
        let started = Instant::now();
        let mut written = 0;
        let mut out = vec![];
        let mut err = vec![];
        let mut eof = [false, false];
        let mut status = None;
        loop {
            if started.elapsed() > timeout {
                return Err(
                    "Command deadline exceeded; outcome unknown. Reload before retrying".into(),
                );
            }
            if let Some(s) = &mut stdin {
                let bytes = input.unwrap_or("").as_bytes();
                match s.write(&bytes[written..]) {
                    Ok(n) => written += n,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => (),
                    Err(_) => {
                        return Err(
                            "Command input failed; outcome unknown. Reload before retrying".into(),
                        )
                    }
                }
                if written == bytes.len() {
                    stdin.take();
                }
            }
            for (i, reader, dest) in [
                (0, &mut stdout as &mut dyn Read, &mut out),
                (1, &mut stderr as &mut dyn Read, &mut err),
            ] {
                let mut buf = [0u8; 8192];
                match reader.read(&mut buf) {
                    Ok(0) => eof[i] = true,
                    Ok(n) => {
                        dest.extend_from_slice(&buf[..n]);
                        if dest.len() > 1024 * 1024 {
                            return Err("Command output limit exceeded; outcome unknown".into());
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => (),
                    Err(_) => return Err("Command output failed; outcome unknown".into()),
                }
            }
            if status.is_none() {
                status = child
                    .try_wait()
                    .map_err(|_| "Command wait failed; outcome unknown")?;
            }
            if status.is_some() && eof.iter().all(|x| *x) {
                return Ok(Output {
                    status: status.unwrap(),
                    out: String::from_utf8(out).map_err(|_| "Command output is not UTF-8")?,
                    err: String::from_utf8(err).map_err(|_| "Command diagnostics are not UTF-8")?,
                });
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    })();
    if result.is_err() {
        unsafe {
            libc::kill(-(child.id() as i32), libc::SIGKILL);
        }
        let _ = child.wait();
    }
    result
}
