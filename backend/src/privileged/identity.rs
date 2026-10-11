//! Root-published API process identity excludes terminals sharing the API Unix UID.
use super::policy::validate_parent;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProcessIdentity {
    pub pid: u32,
    pub start_ticks: u64,
    pub boot_id: String,
}
pub(crate) fn path(socket: &Path) -> PathBuf {
    socket.with_extension("api-peer.json")
}
pub(crate) fn capture(pid: u32, uid: u32) -> io::Result<ProcessIdentity> {
    if pid == 0 {
        return Err(invalid());
    }
    let status = fs::read_to_string(format!("/proc/{pid}/status"))?;
    let ids = status
        .lines()
        .find_map(|line| line.strip_prefix("Uid:"))
        .ok_or_else(invalid)?
        .split_whitespace()
        .map(str::parse::<u32>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| invalid())?;
    if ids.len() != 4 || ids.iter().any(|value| *value != uid) {
        return Err(invalid());
    }
    let stat = fs::read_to_string(format!("/proc/{pid}/stat"))?;
    let fields = stat.rsplit_once(')').ok_or_else(invalid)?.1;
    let start_ticks = fields
        .split_whitespace()
        .nth(19)
        .ok_or_else(invalid)?
        .parse()
        .map_err(|_| invalid())?;
    let boot_id = fs::read_to_string("/proc/sys/kernel/random/boot_id")?
        .trim()
        .to_string();
    if !super::protocol::is_id(&boot_id) {
        return Err(invalid());
    }
    Ok(ProcessIdentity {
        pid,
        start_ticks,
        boot_id,
    })
}
pub(crate) fn load(socket: &Path) -> io::Result<ProcessIdentity> {
    let path = path(socket);
    validate_parent(&path)?;
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || metadata.uid() != 0
        || metadata.mode() & 0o077 != 0
        || metadata.len() > 4096
    {
        return Err(invalid());
    }
    let mut bytes = Vec::new();
    file.take(4097).read_to_end(&mut bytes)?;
    if bytes.len() > 4096 {
        return Err(invalid());
    }
    let value: ProcessIdentity = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    if value.pid == 0 || !super::protocol::is_id(&value.boot_id) {
        return Err(invalid());
    }
    Ok(value)
}
pub fn publish(socket: &Path, pid: u32, api_uid: u32) -> io::Result<()> {
    if unsafe { libc::geteuid() } != 0 || api_uid == 0 {
        return Err(invalid());
    }
    let path = path(socket);
    validate_parent(&path)?;
    let identity = capture(pid, api_uid)?;
    let temporary = path.with_file_name(format!(".api-peer-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)?;
        file.write_all(&serde_json::to_vec(&identity).map_err(io::Error::other)?)?;
        file.sync_all()?;
        fs::rename(&temporary, &path)?;
        fs::File::open(path.parent().unwrap())?.sync_all()
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}
fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "Invalid API process identity",
    )
}
