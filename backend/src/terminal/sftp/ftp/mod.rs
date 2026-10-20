//! A saved server's files over FTP or FTPS (explicit AUTH TLS, or implicit
//! TLS from the first byte), for a server that offers no SFTP. The file panel sees the same operations as over SFTP.
//!
//! FTP does one thing at a time per connection, so a server's entry keeps a
//! couple of logged-in connections: one answers listings and renames while
//! another carries a download or an upload.

mod control;
mod list;
mod stream;
pub(crate) mod tls;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};
use tokio::io::AsyncReadExt;

use super::fs::Meta;
use crate::models::RemoteServer;
use control::Control;
pub(crate) use control::FtpError;
pub(crate) use stream::{Download, Upload};

/// A server entry nobody has used for this long is closed.
const IDLE: Duration = Duration::from_secs(10 * 60);
/// A connection kept this long without use is checked before it is reused.
const STALE: Duration = Duration::from_secs(20);
/// Connections kept for reuse; any more are closed when they are done.
const SPARE: usize = 2;
/// A listing larger than this is not read.
const MAX_LISTING: u64 = 32 * 1024 * 1024;

type Result<T> = std::result::Result<T, FtpError>;

pub(crate) struct FtpConn {
    server: RemoteServer,
    password: String,
    tls: Option<Arc<tls::Tls>>,
    idle: Mutex<Vec<(Control, Instant)>>,
    last_used: Mutex<Instant>,
}

impl FtpConn {
    fn new(server: &RemoteServer) -> std::result::Result<Self, String> {
        let tls = match server.files.as_str() {
            "ftps" => Some(Arc::new(tls::Tls::new(&server.host, &server.tls_fingerprint)?)),
            _ => None,
        };
        // FTP has no keys: no stored password means an anonymous login.
        let password = crate::remote_servers::server_password(server).unwrap_or_default();
        Ok(FtpConn { server: server.clone(), password, tls, idle: Mutex::new(Vec::new()), last_used: Mutex::new(Instant::now()) })
    }

    fn idle_too_long(&self) -> bool {
        self.last_used.lock().map(|t| t.elapsed() > IDLE).unwrap_or(true)
    }

    /// A logged-in connection: a kept one that still answers, or a new one.
    async fn take(&self) -> Result<Control> {
        if let Ok(mut t) = self.last_used.lock() {
            *t = Instant::now();
        }
        loop {
            let kept = self.idle.lock().unwrap_or_else(|e| e.into_inner()).pop();
            let Some((mut ctl, since)) = kept else { break };
            if since.elapsed() < STALE || ctl.cmd("NOOP").await.is_ok_and(|r| r.code == 200) {
                return Ok(ctl);
            }
        }
        Control::open(&self.server, &self.password, self.tls.clone()).await
    }

    pub(crate) fn give_back(&self, ctl: Control) {
        let mut idle = self.idle.lock().unwrap_or_else(|e| e.into_inner());
        if idle.len() < SPARE {
            idle.push((ctl, Instant::now()));
        }
    }

    /// Keeps the connection unless the step broke it.
    fn settle<T>(&self, ctl: Control, result: Result<T>) -> Result<T> {
        if !matches!(result, Err(FtpError::Broken(_))) {
            self.give_back(ctl);
        }
        result
    }

    pub async fn canonicalize(&self, path: &str) -> Result<String> {
        let mut ctl = self.take().await?;
        let r = if path == "." {
            Ok(ctl.home.clone())
        } else {
            match ctl.cmd(&format!("CWD {path}")).await {
                Ok(r) if r.code == 250 => ctl.pwd().await,
                Ok(r) => Err(FtpError::Refused(r)),
                Err(e) => Err(e),
            }
        };
        self.settle(ctl, r)
    }

    pub async fn read_dir(&self, path: &str) -> Result<Vec<(String, Meta)>> {
        let mut ctl = self.take().await?;
        let r = listing(&mut ctl, path).await;
        self.settle(ctl, r)
    }

    /// Follows links, as far as FTP can tell.
    pub async fn stat(&self, path: &str) -> Result<Meta> {
        let mut ctl = self.take().await?;
        let r = stat(&mut ctl, path).await;
        self.settle(ctl, r)
    }

    /// The entry itself, read from its folder's listing, which is the only
    /// place plain FTP tells a link from what it points to.
    pub async fn lstat(&self, path: &str) -> Result<Meta> {
        let mut ctl = self.take().await?;
        let r = if ctl.features.mlst {
            mlst(&mut ctl, path).await
        } else {
            let (folder, name) = split(path);
            match listing(&mut ctl, folder).await {
                Ok(items) => items
                    .into_iter()
                    .find(|(n, _)| n == name)
                    .map(|(_, m)| m)
                    .ok_or_else(|| FtpError::Refused(control::Reply { code: 550, text: "No such file".into() })),
                Err(e) => Err(e),
            }
        };
        self.settle(ctl, r)
    }

    pub async fn open(self: &Arc<Self>, path: &str) -> Result<Download> {
        self.open_from(path, 0).await
    }

    /// RETR from `offset` on (REST first), for a range of the file.
    pub async fn open_from(self: &Arc<Self>, path: &str, offset: u64) -> Result<Download> {
        let mut ctl = self.take().await?;
        if offset > 0 {
            let rest = ctl.cmd(&format!("REST {offset}")).await.and_then(|r| control::expect(r, 350));
            if let Err(e) = rest {
                return self.settle(ctl, Err(e));
            }
        }
        match ctl.start(&format!("RETR {path}")).await {
            Ok(data) => Ok(Download::new(data, ctl, self.clone())),
            Err(e) => self.settle(ctl, Err(e)),
        }
    }

    /// Creates or truncates; with `append`, continues the file where it ends.
    pub async fn upload(self: &Arc<Self>, path: &str, append: bool) -> Result<Upload> {
        let mut ctl = self.take().await?;
        let verb = if append { "APPE" } else { "STOR" };
        match ctl.start(&format!("{verb} {path}")).await {
            Ok(data) => Ok(Upload::new(data, ctl, self.clone())),
            Err(e) => self.settle(ctl, Err(e)),
        }
    }

    /// Commands answered by one reply: MKD, DELE, RMD, and RNFR then RNTO.
    pub async fn simple(&self, steps: &[(String, u16)]) -> Result<()> {
        let mut ctl = self.take().await?;
        let mut r = Ok(());
        for (line, want) in steps {
            r = ctl.cmd(line).await.and_then(|reply| control::expect(reply, *want)).map(|_| ());
            if r.is_err() {
                break;
            }
        }
        self.settle(ctl, r)
    }
}

/// `/a/b/c` as (`/a/b`, `c`).
fn split(path: &str) -> (&str, &str) {
    match path.trim_end_matches('/').rsplit_once('/') {
        Some(("", name)) => ("/", name),
        Some((folder, name)) => (folder, name),
        None => (".", path),
    }
}

async fn listing(ctl: &mut Control, path: &str) -> Result<Vec<(String, Meta)>> {
    let mlsd = ctl.features.mlst;
    let raw = match read_all(ctl, &if mlsd { format!("MLSD {path}") } else { format!("LIST -a {path}") }).await {
        // A server that takes no options for LIST.
        Err(FtpError::Refused(r)) if !mlsd && r.code >= 500 => read_all(ctl, &format!("LIST {path}")).await?,
        other => other?,
    };
    let text = String::from_utf8_lossy(&raw);
    let today = chrono::Utc::now().date_naive();
    Ok(text
        .lines()
        .filter_map(|l| if mlsd { list::mlsd_line(l) } else { list::list_line(l, today) })
        .collect())
}

async fn read_all(ctl: &mut Control, line: &str) -> Result<Vec<u8>> {
    let mut data = ctl.start(line).await?;
    let mut raw = Vec::new();
    match (&mut data).take(MAX_LISTING).read_to_end(&mut raw).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {}
        Err(e) => return Err(FtpError::Broken(format!("The listing was cut off: {e}"))),
    }
    drop(data);
    ctl.finish().await?;
    Ok(raw)
}

async fn mlst(ctl: &mut Control, path: &str) -> Result<Meta> {
    let r = control::expect(ctl.cmd(&format!("MLST {path}")).await?, 250)?;
    let facts = r
        .text
        .lines()
        .map(str::trim_start)
        .find_map(|l| l.split_once(' ').and_then(|(facts, _)| list::facts_meta(facts)));
    let mut meta = facts.ok_or_else(|| FtpError::Broken("The server's MLST reply could not be read".into()))?;
    if meta.kind.is_empty() {
        meta.kind = "dir";
    }
    Ok(meta)
}

async fn stat(ctl: &mut Control, path: &str) -> Result<Meta> {
    if ctl.features.mlst {
        let meta = mlst(ctl, path).await?;
        if meta.kind != "link" {
            return Ok(meta);
        }
    }
    let size = ctl.cmd(&format!("SIZE {path}")).await?;
    if size.code == 213 {
        let mtime = match ctl.cmd(&format!("MDTM {path}")).await? {
            r if r.code == 213 => list::timestamp(r.text.trim()),
            _ => None,
        };
        let size = size.text.trim().parse().unwrap_or(0);
        return Ok(Meta { kind: "file", size, mtime, ..Meta::default() });
    }
    match ctl.cmd(&format!("CWD {path}")).await? {
        r if r.code == 250 => Ok(Meta { kind: "dir", ..Meta::default() }),
        _ => Err(FtpError::Refused(size)),
    }
}

fn pool() -> &'static Mutex<HashMap<String, Arc<FtpConn>>> {
    static POOL: OnceLock<Mutex<HashMap<String, Arc<FtpConn>>>> = OnceLock::new();
    POOL.get_or_init(Default::default)
}

/// Editing anything about how the server is reached makes a new entry.
fn key(user_id: &str, srv: &RemoteServer) -> String {
    let port = format!("{}{}", srv.ftp_port, if srv.ftps_implicit { "i" } else { "" });
    let parts = [user_id, &srv.id, &srv.host, &port, &srv.user, &srv.files, &srv.tls_fingerprint, &srv.password_enc];
    hex::encode(Sha256::digest(parts.join("\0")))
}

/// The operator's entry for `srv`, logged in at least once so a wrong
/// password or an untrusted certificate is reported right away.
pub(crate) async fn session(user_id: &str, srv: &RemoteServer) -> std::result::Result<Arc<FtpConn>, String> {
    let key = key(user_id, srv);
    {
        let mut map = pool().lock().unwrap_or_else(|e| e.into_inner());
        map.retain(|_, c| !c.idle_too_long());
        if let Some(conn) = map.get(&key) {
            return Ok(conn.clone());
        }
    }
    let conn = Arc::new(FtpConn::new(srv)?);
    let ctl = conn.take().await.map_err(|e| format!("FTP connection failed: {e}"))?;
    conn.give_back(ctl);
    pool().lock().unwrap_or_else(|e| e.into_inner()).insert(key, conn.clone());
    Ok(conn)
}

pub(crate) fn forget(user_id: &str, srv: &RemoteServer) {
    pool().lock().unwrap_or_else(|e| e.into_inner()).remove(&key(user_id, srv));
}

/// Logs in once and out again: the health check of an FTP server.
pub(crate) async fn check(srv: &RemoteServer) -> std::result::Result<(), String> {
    let conn = FtpConn::new(srv)?;
    let mut ctl = conn.take().await.map_err(|e| e.to_string())?;
    let _ = ctl.cmd("QUIT").await;
    Ok(())
}

/// Whether a plain FTP server would also take TLS.
pub(crate) async fn offers_tls(srv: &RemoteServer) -> std::result::Result<bool, String> {
    Control::offers_tls(srv).await.map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::control::{epsv_port, pasv_port, quoted_path, wanted_facts};
    use super::split;

    #[test]
    fn replies_are_read() {
        assert_eq!(quoted_path(r#""/home/a ""b""" is current"#).as_deref(), Some(r#"/home/a "b""#));
        assert_eq!(epsv_port("Entering Extended Passive Mode (|||6446|)"), Some(6446));
        assert_eq!(pasv_port("Entering Passive Mode (10,0,0,5,19,136)."), Some(5000));
        assert_eq!(pasv_port("Entering Passive Mode (10,0,0,5,300,1)"), None);
        assert_eq!(
            wanted_facts("MLST type*;perm*;size*;modify*;unique*;unix.mode;unix.uid;unix.gid;"),
            "type;size;modify;unix.mode;unix.uid;unix.gid;"
        );
        assert_eq!(wanted_facts("MLST perm*;unique*;"), "");
    }

    #[test]
    fn paths_split_into_folder_and_name() {
        assert_eq!(split("/a/b/c.txt"), ("/a/b", "c.txt"));
        assert_eq!(split("/top"), ("/", "top"));
        assert_eq!(split("/a/dir/"), ("/a", "dir"));
    }
}
