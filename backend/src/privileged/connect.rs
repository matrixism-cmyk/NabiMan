//! A full helper accept queue must not leave an API worker blocked in connect().
use std::{
    io,
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::net::UnixStream,
    },
    path::Path,
    time::{Duration, Instant},
};
pub(crate) fn connect(path: &Path) -> io::Result<UnixStream> {
    let bytes = path.as_os_str().as_encoded_bytes();
    let mut address: libc::sockaddr_un = unsafe { std::mem::zeroed() };
    if bytes.is_empty() || bytes.len() >= address.sun_path.len() || bytes.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Invalid helper socket path",
        ));
    }
    address.sun_family = libc::AF_UNIX as libc::sa_family_t;
    for (target, source) in address.sun_path.iter_mut().zip(bytes) {
        *target = *source as libc::c_char;
    }
    let fd = unsafe {
        libc::socket(
            libc::AF_UNIX,
            libc::SOCK_STREAM | libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC,
            0,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    let socket = unsafe { UnixStream::from_raw_fd(fd) };
    let result = unsafe {
        libc::connect(
            fd,
            (&address as *const libc::sockaddr_un).cast(),
            std::mem::size_of_val(&address) as libc::socklen_t,
        )
    };
    if result != 0 {
        let error = io::Error::last_os_error();
        // AF_UNIX EAGAIN means a full accept queue. Reject without a connect retry.
        if error.raw_os_error() != Some(libc::EINPROGRESS) {
            return Err(error);
        }
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "Helper connect timeout",
                ));
            }
            let mut poll = libc::pollfd {
                fd: socket.as_raw_fd(),
                events: libc::POLLOUT,
                revents: 0,
            };
            let ready = unsafe { libc::poll(&mut poll, 1, remaining.as_millis().min(2000) as i32) };
            if ready < 0 && io::Error::last_os_error().kind() == io::ErrorKind::Interrupted {
                continue;
            }
            if ready <= 0 {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "Helper connect unavailable",
                ));
            }
            let mut code: libc::c_int = 0;
            let mut len = std::mem::size_of_val(&code) as libc::socklen_t;
            if unsafe {
                libc::getsockopt(
                    fd,
                    libc::SOL_SOCKET,
                    libc::SO_ERROR,
                    (&mut code as *mut libc::c_int).cast(),
                    &mut len,
                )
            } != 0
            {
                return Err(io::Error::last_os_error());
            }
            if code != 0 {
                return Err(io::Error::from_raw_os_error(code));
            }
            break;
        }
    }
    socket.set_nonblocking(false)?;
    Ok(socket)
}
