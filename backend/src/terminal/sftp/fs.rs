//! The places a file panel can look at: a saved server over SFTP or over
//! FTP/FTPS, or the machine NabiMan runs on, which is what its local shell
//! sees. The handlers do not need to know which.

use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::pin::Pin;
use std::sync::Arc;

use actix_web::http::StatusCode;
use russh_sftp::client::error::Error as SftpError;
use russh_sftp::client::fs::Metadata as SftpMeta;
use tokio::io::{AsyncRead, AsyncWrite};

use super::ftp::{FtpConn, FtpError};
use super::pool::Conn;

#[derive(Clone)]
pub(crate) enum Fs {
    Remote(Arc<Conn>),
    Ftp(Arc<FtpConn>),
    Local,
}

pub(crate) struct FsError {
    pub message: String,
    pub status: StatusCode,
    /// The connection itself failed; it should not be reused.
    pub broken: bool,
}

impl From<SftpError> for FsError {
    fn from(e: SftpError) -> Self {
        match e {
            SftpError::Status(status) => FsError {
                message: if status.error_message.is_empty() {
                    format!("{:?}", status.status_code)
                } else {
                    status.error_message
                },
                status: StatusCode::CONFLICT,
                broken: false,
            },
            other => FsError {
                message: format!("SFTP connection lost ({other}); try again"),
                status: StatusCode::BAD_GATEWAY,
                broken: true,
            },
        }
    }
}

impl From<FtpError> for FsError {
    fn from(e: FtpError) -> Self {
        match e {
            FtpError::Refused(reply) => FsError {
                message: if reply.text.trim().is_empty() { format!("FTP error {}", reply.code) } else { reply.text.trim().to_string() },
                status: StatusCode::CONFLICT,
                broken: false,
            },
            FtpError::Broken(message) => FsError {
                message: format!("FTP connection lost ({message}); try again"),
                status: StatusCode::BAD_GATEWAY,
                broken: true,
            },
        }
    }
}

impl From<std::io::Error> for FsError {
    fn from(e: std::io::Error) -> Self {
        let message = match e.kind() {
            std::io::ErrorKind::NotFound => "No such file".to_string(),
            std::io::ErrorKind::PermissionDenied => "Permission denied".to_string(),
            std::io::ErrorKind::AlreadyExists => "Already exists".to_string(),
            _ => e.to_string(),
        };
        FsError { message, status: StatusCode::CONFLICT, broken: false }
    }
}

pub(crate) type Result<T> = std::result::Result<T, FsError>;
pub(crate) type Reader = Pin<Box<dyn AsyncRead + Send>>;
pub(crate) type Writer = Pin<Box<dyn AsyncWrite + Send>>;

/// What a listing shows about an entry. `kind` is `dir`, `file`, `link` or
/// `other`, and describes the entry itself, never what a link points to.
/// Owners come as numbers (SFTP, this machine) or names (FTP listings).
#[derive(Clone, Default)]
pub(crate) struct Meta {
    pub kind: &'static str,
    pub size: u64,
    pub mtime: Option<u32>,
    pub mode: Option<u32>,
    pub uid: Option<u32>,
    pub gid: Option<u32>,
    pub owner: Option<String>,
    pub group: Option<String>,
}

impl Meta {
    pub fn is_dir(&self) -> bool {
        self.kind == "dir"
    }
    fn remote(m: &SftpMeta) -> Self {
        Meta {
            kind: super::paths::kind_of(m),
            size: m.size.unwrap_or(0),
            mtime: m.mtime,
            mode: m.permissions.map(|p| p & 0o7777),
            uid: m.uid,
            gid: m.gid,
            ..Meta::default()
        }
    }
    fn local(m: &std::fs::Metadata) -> Self {
        let t = m.file_type();
        Meta {
            kind: if t.is_symlink() { "link" } else if t.is_dir() { "dir" } else if t.is_file() { "file" } else { "other" },
            size: m.len(),
            mtime: u32::try_from(m.mtime()).ok(),
            mode: Some(m.permissions().mode() & 0o7777),
            uid: Some(m.uid()),
            gid: Some(m.gid()),
            ..Meta::default()
        }
    }
}

fn home() -> String {
    std::env::var("HOME").ok().filter(|h| h.starts_with('/')).unwrap_or_else(|| "/".into())
}

impl Fs {
    /// The absolute form of `path`; `.` means the home folder.
    pub async fn canonicalize(&self, path: &str) -> Result<String> {
        match self {
            Fs::Remote(c) => Ok(c.sftp.canonicalize(path).await?),
            Fs::Ftp(c) => Ok(c.canonicalize(path).await?),
            Fs::Local => {
                let path = if path == "." { home() } else { path.to_string() };
                Ok(tokio::fs::canonicalize(path).await?.to_string_lossy().into_owned())
            }
        }
    }

    pub async fn read_dir(&self, path: &str) -> Result<Vec<(String, Meta)>> {
        match self {
            Fs::Remote(c) => Ok(c
                .sftp
                .read_dir(path)
                .await?
                .map(|e| (e.file_name(), Meta::remote(&e.metadata())))
                .collect()),
            Fs::Ftp(c) => Ok(c.read_dir(path).await?),
            Fs::Local => {
                let mut dir = tokio::fs::read_dir(path).await?;
                let mut out = Vec::new();
                while let Some(entry) = dir.next_entry().await? {
                    // DirEntry::metadata does not follow links, like SFTP's listing.
                    if let Ok(meta) = entry.metadata().await {
                        out.push((entry.file_name().to_string_lossy().into_owned(), Meta::local(&meta)));
                    }
                }
                Ok(out)
            }
        }
    }

    /// Follows links.
    pub async fn stat(&self, path: &str) -> Result<Meta> {
        match self {
            Fs::Remote(c) => Ok(Meta::remote(&c.sftp.metadata(path).await?)),
            Fs::Ftp(c) => Ok(c.stat(path).await?),
            Fs::Local => Ok(Meta::local(&tokio::fs::metadata(path).await?)),
        }
    }

    /// Does not follow links.
    pub async fn lstat(&self, path: &str) -> Result<Meta> {
        match self {
            Fs::Remote(c) => Ok(Meta::remote(&c.sftp.symlink_metadata(path).await?)),
            Fs::Ftp(c) => Ok(c.lstat(path).await?),
            Fs::Local => Ok(Meta::local(&tokio::fs::symlink_metadata(path).await?)),
        }
    }

    pub async fn exists(&self, path: &str) -> bool {
        self.lstat(path).await.is_ok()
    }

    pub async fn open(&self, path: &str) -> Result<Reader> {
        match self {
            Fs::Remote(c) => Ok(Box::pin(c.sftp.open(path).await?)),
            Fs::Ftp(c) => Ok(Box::pin(c.open(path).await?)),
            Fs::Local => Ok(Box::pin(tokio::fs::File::open(path).await?)),
        }
    }

    /// Creates or truncates.
    pub async fn create(&self, path: &str) -> Result<Writer> {
        match self {
            Fs::Remote(c) => Ok(Box::pin(c.sftp.create(path).await?)),
            Fs::Ftp(c) => Ok(Box::pin(c.upload(path, false).await?)),
            Fs::Local => Ok(Box::pin(tokio::fs::File::create(path).await?)),
        }
    }

    /// Opens an existing file for writing at `offset`, keeping what is there:
    /// the next piece of an upload that arrives in parts.
    pub async fn open_at(&self, path: &str, offset: u64) -> Result<Writer> {
        use tokio::io::AsyncSeekExt;
        let start = std::io::SeekFrom::Start(offset);
        match self {
            Fs::Remote(c) => {
                let flags = russh_sftp::protocol::OpenFlags::WRITE;
                let mut file = c.sftp.open_with_flags(path, flags).await?;
                file.seek(start).await?;
                Ok(Box::pin(file))
            }
            // The caller has checked the file ends at `offset`.
            Fs::Ftp(c) => Ok(Box::pin(c.upload(path, offset > 0).await?)),
            Fs::Local => {
                let mut file = tokio::fs::OpenOptions::new().write(true).open(path).await?;
                file.seek(start).await?;
                Ok(Box::pin(file))
            }
        }
    }

    pub async fn mkdir(&self, path: &str) -> Result<()> {
        match self {
            Fs::Remote(c) => Ok(c.sftp.create_dir(path).await?),
            Fs::Ftp(c) => Ok(c.simple(&[(format!("MKD {path}"), 257)]).await?),
            Fs::Local => Ok(tokio::fs::create_dir(path).await?),
        }
    }

    pub async fn rename(&self, from: &str, to: &str) -> Result<()> {
        match self {
            Fs::Remote(c) => Ok(c.sftp.rename(from, to).await?),
            Fs::Ftp(c) => Ok(c.simple(&[(format!("RNFR {from}"), 350), (format!("RNTO {to}"), 250)]).await?),
            Fs::Local => Ok(tokio::fs::rename(from, to).await?),
        }
    }

    pub async fn remove_file(&self, path: &str) -> Result<()> {
        match self {
            Fs::Remote(c) => Ok(c.sftp.remove_file(path).await?),
            Fs::Ftp(c) => Ok(c.simple(&[(format!("DELE {path}"), 250)]).await?),
            Fs::Local => Ok(tokio::fs::remove_file(path).await?),
        }
    }

    /// Sets the permission bits. Follows links, so callers leave links alone.
    pub async fn chmod(&self, path: &str, mode: u32) -> Result<()> {
        match self {
            Fs::Remote(c) => {
                let attrs = SftpMeta { permissions: Some(mode), ..SftpMeta::empty() };
                Ok(c.sftp.set_metadata(path, attrs).await?)
            }
            Fs::Ftp(c) => Ok(c.simple(&[(format!("SITE CHMOD {mode:o} {path}"), 200)]).await?),
            Fs::Local => Ok(tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).await?),
        }
    }

    /// Sets the owner and/or group by number. FTP has no way to.
    pub async fn chown(&self, path: &str, uid: Option<u32>, gid: Option<u32>) -> Result<()> {
        match self {
            Fs::Remote(c) => {
                // SFTP sets both or neither; the one not asked for stays as it is.
                let now = c.sftp.metadata(path).await?;
                let attrs = SftpMeta { uid: uid.or(now.uid), gid: gid.or(now.gid), ..SftpMeta::empty() };
                Ok(c.sftp.set_metadata(path, attrs).await?)
            }
            Fs::Ftp(_) => Err(FsError {
                message: "FTP cannot change owners or groups".into(),
                status: StatusCode::BAD_REQUEST,
                broken: false,
            }),
            Fs::Local => Ok(std::os::unix::fs::chown(path, uid, gid)?),
        }
    }

    pub async fn remove_dir(&self, path: &str) -> Result<()> {
        match self {
            Fs::Remote(c) => Ok(c.sftp.remove_dir(path).await?),
            Fs::Ftp(c) => Ok(c.simple(&[(format!("RMD {path}"), 250)]).await?),
            Fs::Local => Ok(tokio::fs::remove_dir(path).await?),
        }
    }
}
