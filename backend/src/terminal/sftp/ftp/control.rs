//! One FTP control connection: greeting, TLS upgrade, login, and commands
//! with their numbered replies. Data connections are opened passively and
//! always to the address the control connection reached, never to the one a
//! PASV reply names, so a server cannot point NabiMan somewhere else.

use std::net::IpAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader, ReadBuf};
use tokio::net::TcpStream;
use tokio::time::timeout;

use super::tls::Tls;
use crate::models::RemoteServer;

const CONNECT: Duration = Duration::from_secs(15);
const REPLY: Duration = Duration::from_secs(60);
/// A reply longer than this is not an FTP server talking.
const MAX_REPLY: usize = 64 * 1024;

/// A plain or TLS-wrapped connection, control or data.
pub(crate) enum Io {
    Plain(TcpStream),
    Tls(Box<tokio_rustls::client::TlsStream<TcpStream>>),
    /// Briefly, while the socket is handed over to TLS.
    Gone,
}

fn gone<T>() -> Poll<std::io::Result<T>> {
    Poll::Ready(Err(std::io::ErrorKind::NotConnected.into()))
}

impl AsyncRead for Io {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            Io::Plain(s) => Pin::new(s).poll_read(cx, buf),
            Io::Tls(s) => Pin::new(s.as_mut()).poll_read(cx, buf),
            Io::Gone => gone(),
        }
    }
}

impl AsyncWrite for Io {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<std::io::Result<usize>> {
        match self.get_mut() {
            Io::Plain(s) => Pin::new(s).poll_write(cx, buf),
            Io::Tls(s) => Pin::new(s.as_mut()).poll_write(cx, buf),
            Io::Gone => gone(),
        }
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            Io::Plain(s) => Pin::new(s).poll_flush(cx),
            Io::Tls(s) => Pin::new(s.as_mut()).poll_flush(cx),
            Io::Gone => gone(),
        }
    }
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            Io::Plain(s) => Pin::new(s).poll_shutdown(cx),
            Io::Tls(s) => Pin::new(s.as_mut()).poll_shutdown(cx),
            Io::Gone => gone(),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Reply {
    pub code: u16,
    pub text: String,
}

#[derive(Debug)]
pub(crate) enum FtpError {
    /// The server answered, and the answer was no.
    Refused(Reply),
    /// The connection itself failed; it cannot be used again.
    Broken(String),
}

pub(crate) type Result<T> = std::result::Result<T, FtpError>;

fn broken(what: impl std::fmt::Display) -> FtpError {
    FtpError::Broken(what.to_string())
}

impl std::fmt::Display for FtpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FtpError::Refused(r) => write!(f, "{} {}", r.code, r.text),
            FtpError::Broken(m) => f.write_str(m),
        }
    }
}

/// What the server said it can do, in answer to FEAT.
#[derive(Default, Clone, Copy)]
pub(crate) struct Features {
    pub mlst: bool,
    pub utf8: bool,
}

pub(crate) struct Control {
    io: BufReader<Io>,
    peer: IpAddr,
    /// The server's TLS settings, and this connection's own copy of them.
    tls: Option<(Arc<Tls>, Arc<rustls::ClientConfig>)>,
    epsv: bool,
    pub features: Features,
    /// Where the login lands, which `.` means.
    pub home: String,
}

impl Control {
    /// Connects and logs in. `tls` is set for FTPS: implicit TLS starts the
    /// handshake at once, explicit TLS asks for it with AUTH TLS.
    pub async fn open(server: &RemoteServer, password: &str, tls: Option<Arc<Tls>>) -> Result<Control> {
        let host = crate::member_servers::dial_host(server, server.ftp_port).map_err(broken)?;
        let addr = (host.as_str(), server.ftp_port);
        let tcp = match timeout(CONNECT, TcpStream::connect(addr)).await {
            Ok(Ok(tcp)) => tcp,
            Ok(Err(e)) => return Err(broken(format!("Cannot reach {}:{}: {e}", server.host, server.ftp_port))),
            Err(_) => return Err(broken(format!("Connecting to {}:{} timed out", server.host, server.ftp_port))),
        };
        let peer = tcp.peer_addr().map_err(broken)?.ip();
        let mut c = Control { io: BufReader::new(Io::Plain(tcp)), peer, tls: None, epsv: true, features: Features::default(), home: "/".into() };
        let implicit = tls.is_some() && server.ftps_implicit;
        if let (Some(tls), true) = (&tls, implicit) {
            c.upgrade(tls.clone()).await?;
        }
        let mut hello = match timeout(CONNECT, c.read_reply()).await {
            Ok(r) => r?,
            Err(_) if !implicit => return Err(broken("The server sent no FTP greeting; if it uses implicit FTPS (usually port 990), choose that")),
            Err(_) => return Err(broken("The server sent no FTP greeting")),
        };
        while hello.code == 120 {
            hello = c.reply().await?;
        }
        expect(hello, 220)?;
        if let Some(tls) = tls {
            if !implicit {
                let r = c.cmd("AUTH TLS").await?;
                if r.code != 234 {
                    return Err(FtpError::Refused(Reply { code: r.code, text: format!("The server does not offer FTPS (AUTH TLS: {})", r.text) }));
                }
                c.upgrade(tls).await?;
            }
            expect(c.cmd("PBSZ 0").await?, 200)?;
            expect(c.cmd("PROT P").await?, 200)?;
        }
        c.login(&server.user, password).await?;
        let (features, facts) = c.feat().await;
        c.features = features;
        if c.features.utf8 {
            let _ = c.cmd("OPTS UTF8 ON").await?;
        }
        if !facts.is_empty() {
            // Ask for permissions and owners too: many servers leave them off.
            let _ = c.cmd(&format!("OPTS MLST {facts}")).await?;
        }
        expect(c.cmd("TYPE I").await?, 200)?;
        c.home = c.pwd().await?;
        Ok(c)
    }

    async fn upgrade(&mut self, tls: Arc<Tls>) -> Result<()> {
        if !self.io.buffer().is_empty() {
            return Err(broken("The server spoke out of turn while starting TLS"));
        }
        let Io::Plain(tcp) = std::mem::replace(self.io.get_mut(), Io::Gone) else {
            return Err(broken("TLS is already on"));
        };
        let config = tls.connection_config();
        let connector = tokio_rustls::TlsConnector::from(config.clone());
        let stream = match timeout(CONNECT, connector.connect(tls.name.clone(), tcp)).await {
            Ok(Ok(s)) => s,
            Ok(Err(e)) => return Err(broken(tls.explain(&e))),
            Err(_) => return Err(broken("TLS handshake timed out")),
        };
        *self.io.get_mut() = Io::Tls(Box::new(stream));
        self.tls = Some((tls, config));
        Ok(())
    }

    async fn login(&mut self, user: &str, password: &str) -> Result<()> {
        let r = self.cmd(&format!("USER {user}")).await?;
        let r = if r.code == 331 { self.cmd(&format!("PASS {password}")).await? } else { r };
        match r.code {
            230 | 202 => Ok(()),
            530 | 331 | 332 => Err(FtpError::Refused(Reply { code: r.code, text: format!("Login failed: {}", r.text) })),
            _ => Err(FtpError::Refused(r)),
        }
    }

    /// What the server can do, and the MLST facts worth asking it for.
    async fn feat(&mut self) -> (Features, String) {
        let Ok(r) = self.cmd("FEAT").await else { return Default::default() };
        if r.code != 211 {
            return Default::default();
        }
        let has = |name: &str| r.text.lines().any(|l| l.trim().to_ascii_uppercase().starts_with(name));
        let facts = r.text.lines().map(str::trim).find(|l| l.to_ascii_uppercase().starts_with("MLST ")).map(wanted_facts);
        (Features { mlst: has("MLST"), utf8: has("UTF8") }, facts.unwrap_or_default())
    }

    /// Whether the server would take TLS. Asked with AUTH TLS itself: some
    /// servers (vsftpd) take it without listing it in FEAT.
    pub async fn offers_tls(server: &RemoteServer) -> Result<bool> {
        let host = crate::member_servers::dial_host(server, server.ftp_port).map_err(broken)?;
        let tcp = timeout(CONNECT, TcpStream::connect((host.as_str(), server.ftp_port)))
            .await
            .map_err(|_| broken("Connecting timed out"))?
            .map_err(broken)?;
        let peer = tcp.peer_addr().map_err(broken)?.ip();
        let mut c = Control { io: BufReader::new(Io::Plain(tcp)), peer, tls: None, epsv: true, features: Features::default(), home: "/".into() };
        expect(c.reply().await?, 220)?;
        // A yes is followed by a TLS handshake, which closing here abandons.
        Ok(c.cmd("AUTH TLS").await?.code == 234)
    }

    pub async fn pwd(&mut self) -> Result<String> {
        let r = expect(self.cmd("PWD").await?, 257)?;
        quoted_path(&r.text).ok_or_else(|| broken("The server did not say where it is"))
    }

    /// Sends one command and reads its final (or first preliminary) reply.
    pub async fn cmd(&mut self, line: &str) -> Result<Reply> {
        if line.contains(['\r', '\n', '\0']) {
            return Err(FtpError::Refused(Reply { code: 501, text: "Names cannot contain line breaks over FTP".into() }));
        }
        let write = async {
            self.io.get_mut().write_all(format!("{line}\r\n").as_bytes()).await?;
            self.io.get_mut().flush().await
        };
        timeout(REPLY, write).await.map_err(|_| broken("The server stopped answering"))?.map_err(broken)?;
        self.reply().await
    }

    /// Reads one reply, joining the lines of a multi-line one.
    pub async fn reply(&mut self) -> Result<Reply> {
        timeout(REPLY, self.read_reply()).await.map_err(|_| broken("The server stopped answering"))?
    }

    async fn read_reply(&mut self) -> Result<Reply> {
        let mut text = String::new();
        let mut first: Option<u16> = None;
        loop {
            let mut raw = Vec::new();
            let n = (&mut self.io).take(MAX_REPLY as u64).read_until(b'\n', &mut raw).await.map_err(broken)?;
            if n == 0 {
                return Err(broken("The server closed the connection"));
            }
            let line = String::from_utf8_lossy(&raw);
            let line = line.trim_end_matches(['\r', '\n']);
            if text.len() + line.len() > MAX_REPLY {
                return Err(broken("The server's reply is too long"));
            }
            let code = line.get(..3).and_then(|c| c.parse::<u16>().ok());
            let end = line.as_bytes().get(3) != Some(&b'-');
            match first {
                None => {
                    let code = code.ok_or_else(|| broken(format!("Not an FTP reply: {}", line.chars().take(80).collect::<String>())))?;
                    first = Some(code);
                    text.push_str(line.get(4..).unwrap_or(""));
                    if end {
                        return Ok(Reply { code, text });
                    }
                }
                Some(want) => {
                    text.push('\n');
                    if code == Some(want) && end {
                        text.push_str(line.get(4..).unwrap_or(""));
                        return Ok(Reply { code: want, text });
                    }
                    text.push_str(line);
                }
            }
        }
    }

    /// Opens a data connection and starts `line` (RETR, STOR, LIST…) on it.
    pub async fn start(&mut self, line: &str) -> Result<Io> {
        let port = self.passive().await?;
        let tcp = match timeout(CONNECT, TcpStream::connect((self.peer, port))).await {
            Ok(Ok(tcp)) => tcp,
            Ok(Err(e)) => return Err(broken(format!("Cannot open the data connection (passive port {port}): {e}"))),
            Err(_) => return Err(broken(format!("The data connection (passive port {port}) timed out"))),
        };
        let r = self.cmd(line).await?;
        if !(100..200).contains(&r.code) {
            return Err(refusal(r));
        }
        match &self.tls {
            None => Ok(Io::Plain(tcp)),
            Some((tls, config)) => {
                let connector = tokio_rustls::TlsConnector::from(config.clone());
                match timeout(CONNECT, connector.connect(tls.name.clone(), tcp)).await {
                    Ok(Ok(s)) => Ok(Io::Tls(Box::new(s))),
                    Ok(Err(e)) => Err(broken(tls.explain(&e))),
                    Err(_) => Err(broken("TLS on the data connection timed out")),
                }
            }
        }
    }

    /// The reply that closes a transfer.
    pub async fn finish(&mut self) -> Result<Reply> {
        let r = self.reply().await?;
        if (200..300).contains(&r.code) { Ok(r) } else { Err(refusal(r)) }
    }

    async fn passive(&mut self) -> Result<u16> {
        if self.epsv {
            let r = self.cmd("EPSV").await?;
            if r.code == 229 {
                if let Some(port) = epsv_port(&r.text) {
                    return Ok(port);
                }
            }
            self.epsv = false;
        }
        let r = expect(self.cmd("PASV").await?, 227)?;
        pasv_port(&r.text).ok_or_else(|| broken("The server's passive reply could not be read"))
    }
}

const FACTS: [&str; 8] = ["type", "size", "modify", "unix.mode", "unix.owner", "unix.group", "unix.uid", "unix.gid"];

/// From FEAT's `MLST type*;size*;unix.mode;…`, the facts NabiMan reads, as
/// `OPTS MLST` takes them; empty when the server offers none of them.
pub(crate) fn wanted_facts(line: &str) -> String {
    let offered = line.split_once(' ').map_or("", |(_, f)| f);
    offered
        .split(';')
        .map(|f| f.trim().trim_end_matches('*'))
        .filter(|f| FACTS.contains(&f.to_ascii_lowercase().as_str()))
        .map(|f| format!("{f};"))
        .collect()
}

/// A refusal as an error; 421 means the server is closing the connection.
fn refusal(r: Reply) -> FtpError {
    if r.code == 421 { FtpError::Broken(format!("{} {}", r.code, r.text)) } else { FtpError::Refused(r) }
}

pub(crate) fn expect(r: Reply, code: u16) -> Result<Reply> {
    if r.code == code { Ok(r) } else { Err(refusal(r)) }
}

/// `257 "/home/user" is the current directory`, with `""` for a quote.
pub(crate) fn quoted_path(text: &str) -> Option<String> {
    let start = text.find('"')? + 1;
    let mut out = String::new();
    let mut chars = text[start..].chars().peekable();
    while let Some(c) = chars.next() {
        if c == '"' {
            if chars.peek() == Some(&'"') {
                chars.next();
                out.push('"');
            } else {
                return Some(out);
            }
        } else {
            out.push(c);
        }
    }
    None
}

/// `229 Entering Extended Passive Mode (|||6446|)`.
pub(crate) fn epsv_port(text: &str) -> Option<u16> {
    let inner = text.split('(').nth(1)?.split(')').next()?;
    let d = inner.chars().next()?;
    inner.split(d).nth(3)?.parse().ok()
}

/// `227 Entering Passive Mode (192,168,1,2,19,136)`; only the port is used.
pub(crate) fn pasv_port(text: &str) -> Option<u16> {
    let start = text.find(|c: char| c.is_ascii_digit())?;
    let nums: Vec<u16> = text[start..]
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .take(6)
        .map(|s| s.parse().ok())
        .collect::<Option<_>>()?;
    match nums.as_slice() {
        [_, _, _, _, hi, lo] if *hi < 256 && *lo < 256 => Some(hi * 256 + lo),
        _ => None,
    }
}
