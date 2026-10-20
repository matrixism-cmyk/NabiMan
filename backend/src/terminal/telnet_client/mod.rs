//! The Telnet client NabiMan's terminal runs inside tmux for a saved Telnet
//! server. It is the server binary itself, started as
//!
//!   nabiman-server telnet-client --host H --port P [--user U] [--password-file F]
//!
//! so every release that carries the server carries the client.
//!
//! It speaks the small part of Telnet a modern terminal needs: window size,
//! terminal type, server echo; everything else is declined. The saved user is
//! typed at the login prompt, and with a password file the password at the
//! password prompt, the way sshpass does for ssh. The file is read and
//! removed at once. Exits 0 when the server closes the connection and 1 when
//! it cannot be reached.

mod protocol;

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use protocol::{AutoLogin, Telnet};

/// The first argument that makes the server binary a Telnet client.
pub const SUBCOMMAND: &str = "telnet-client";
const CONNECT: Duration = Duration::from_secs(10);
/// The stored password is only typed this soon after connecting.
const LOGIN_WINDOW: Duration = Duration::from_secs(60);

static RESIZED: AtomicBool = AtomicBool::new(false);
static SAVED_TERMIOS: OnceLock<libc::termios> = OnceLock::new();

extern "C" fn on_winch(_: libc::c_int) {
    RESIZED.store(true, Ordering::Relaxed);
}

struct Args {
    host: String,
    port: u16,
    user: String,
    password_file: Option<String>,
}

fn args() -> Result<Args, String> {
    let mut out = Args { host: String::new(), port: 23, user: String::new(), password_file: None };
    let mut it = std::env::args().skip(2);
    while let Some(flag) = it.next() {
        let value = it.next().ok_or(format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--host" => out.host = value,
            "--port" => out.port = value.parse().map_err(|_| "Invalid port")?,
            "--user" => out.user = value,
            "--password-file" => out.password_file = Some(value),
            _ => return Err(format!("Unknown option {flag}")),
        }
    }
    if out.host.is_empty() {
        return Err("Usage: nabiman-server telnet-client --host H [--port P] [--user U] [--password-file F]".into());
    }
    Ok(out)
}

fn window() -> (u16, u16) {
    let mut ws: libc::winsize = unsafe { std::mem::zeroed() };
    // SAFETY: TIOCGWINSZ fills the struct it is given.
    if unsafe { libc::ioctl(1, libc::TIOCGWINSZ, &mut ws) } == 0 && ws.ws_col > 0 {
        (ws.ws_col, ws.ws_row)
    } else {
        (80, 24)
    }
}

/// Raw keyboard: every key goes to the server as it is pressed.
fn raw_mode() {
    // SAFETY: plain termios calls on stdin, with a zeroed struct to fill.
    unsafe {
        if libc::isatty(0) != 1 {
            return;
        }
        let mut t: libc::termios = std::mem::zeroed();
        if libc::tcgetattr(0, &mut t) != 0 {
            return;
        }
        let _ = SAVED_TERMIOS.set(t);
        libc::cfmakeraw(&mut t);
        libc::tcsetattr(0, libc::TCSANOW, &t);
    }
}

fn leave(code: i32, message: &str) -> ! {
    if let Some(t) = SAVED_TERMIOS.get() {
        // SAFETY: restores the settings read at start.
        unsafe { libc::tcsetattr(0, libc::TCSANOW, t) };
    }
    let mut out = std::io::stdout();
    let _ = write!(out, "{message}");
    let _ = out.flush();
    std::process::exit(code);
}

fn connect(host: &str, port: u16) -> Result<TcpStream, String> {
    let addrs = (host, port).to_socket_addrs().map_err(|e| format!("{host}: {e}"))?;
    let mut last = format!("{host}: no address");
    for addr in addrs {
        match TcpStream::connect_timeout(&addr, CONNECT) {
            Ok(s) => return Ok(s),
            Err(e) => last = format!("{addr}: {e}"),
        }
    }
    Err(last)
}

/// Runs the client and exits when the binary was started as one; returns
/// at once otherwise.
pub fn run_if_asked() {
    if std::env::args().nth(1).as_deref() != Some(SUBCOMMAND) {
        return;
    }
    let args = args().unwrap_or_else(|e| leave(2, &format!("{e}\r\n")));
    let password = args.password_file.as_ref().and_then(|path| {
        let pw = std::fs::read_to_string(path).ok();
        let _ = std::fs::remove_file(path);
        pw.map(|p| p.trim_end_matches(['\r', '\n']).to_string())
    });
    let mut out = std::io::stdout();
    let _ = write!(out, "{}:{} 에 Telnet으로 연결하는 중… (암호화되지 않는 연결입니다)\r\n", args.host, args.port);
    let _ = out.flush();
    let sock = connect(&args.host, args.port).unwrap_or_else(|e| leave(1, &format!("연결할 수 없습니다: {e}\r\n")));
    let _ = sock.set_nodelay(true);
    raw_mode();
    // SAFETY: the handler only stores to an atomic.
    unsafe { libc::signal(libc::SIGWINCH, on_winch as *const () as libc::sighandler_t) };

    let telnet = Arc::new(Mutex::new(Telnet::new(window())));
    let writer = Arc::new(Mutex::new(sock.try_clone().unwrap_or_else(|e| leave(1, &format!("{e}\r\n")))));
    let send = {
        let writer = writer.clone();
        move |bytes: &[u8]| {
            if !bytes.is_empty() {
                let _ = writer.lock().unwrap_or_else(|e| e.into_inner()).write_all(bytes);
            }
        }
    };
    send(&telnet.lock().unwrap_or_else(|e| e.into_inner()).hello());

    let reader = {
        let (telnet, send) = (telnet.clone(), send.clone());
        let mut login = AutoLogin::new(args.user.clone(), password);
        let started = Instant::now();
        let mut sock = sock;
        std::thread::spawn(move || {
            let mut buf = [0u8; 16 * 1024];
            let mut out = std::io::stdout();
            loop {
                let n = match sock.read(&mut buf) {
                    Ok(0) | Err(_) => leave(0, "\r\n[Telnet 연결이 닫혔습니다]\r\n"),
                    Ok(n) => n,
                };
                let mut t = telnet.lock().unwrap_or_else(|e| e.into_inner());
                let (screen, reply) = t.feed(&buf[..n]);
                send(&reply);
                let _ = out.write_all(&screen);
                let _ = out.flush();
                if started.elapsed() > LOGIN_WINDOW {
                    login.stop();
                }
                if let Some(line) = login.seen(&screen) {
                    send(&t.encode(line.as_bytes()));
                }
            }
        })
    };

    let mut stdin = std::io::stdin();
    let mut buf = [0u8; 4096];
    loop {
        if RESIZED.swap(false, Ordering::Relaxed) {
            let mut t = telnet.lock().unwrap_or_else(|e| e.into_inner());
            t.size = window();
            send(&t.window_size());
        }
        let mut fds = libc::pollfd { fd: 0, events: libc::POLLIN, revents: 0 };
        // SAFETY: one valid pollfd; a signal just ends the wait early.
        if unsafe { libc::poll(&mut fds, 1, 250) } <= 0 {
            if reader.is_finished() {
                break;
            }
            continue;
        }
        let n = match stdin.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        let t = telnet.lock().unwrap_or_else(|e| e.into_inner());
        if !t.server_echoes() {
            // The server leaves echoing to the client.
            let shown: Vec<u8> = buf[..n].iter().flat_map(|&b| if b == b'\r' { vec![b'\r', b'\n'] } else { vec![b] }).collect();
            let _ = out.write_all(&shown);
            let _ = out.flush();
        }
        send(&t.encode(&buf[..n]));
    }
    leave(0, "");
}
