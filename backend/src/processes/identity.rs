use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    os::{fd::AsRawFd, unix::fs::MetadataExt},
    path::Path,
};
pub(super) struct Identity {
    pub value: String,
    pub command: String,
    pub uid: u32,
    pub start: u64,
    pub parent: u32,
}
fn read(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    let mut b = vec![];
    fs::File::open(path)
        .map_err(|_| "Process is unavailable")?
        .take(limit + 1)
        .read_to_end(&mut b)
        .map_err(|_| "Process data is unavailable")?;
    if b.len() as u64 > limit {
        return Err("Process data exceeds read limit".into());
    }
    Ok(b)
}
pub(super) fn stat(value: &str) -> Result<(u32, u64, String), String> {
    let begin = value.find('(').ok_or("Invalid process stat")?;
    let end = value.rfind(')').ok_or("Invalid process stat")?;
    if end <= begin {
        return Err("Invalid process stat".into());
    }
    let p: Vec<_> = value[end + 1..].split_whitespace().collect();
    if p.len() < 20 {
        return Err("Incomplete process stat".into());
    }
    Ok((
        p[1].parse().map_err(|_| "Invalid parent PID")?,
        p[19].parse().map_err(|_| "Invalid process start")?,
        value[begin + 1..end].into(),
    ))
}
pub(super) fn inspect(pid: u32) -> Result<Identity, String> {
    if pid < 1 || pid > i32::MAX as u32 {
        return Err("Invalid process PID".into());
    }
    let dir = fs::File::open(format!("/proc/{pid}"))
        .map_err(|_| "Process disappeared or is unavailable")?;
    let meta = dir
        .metadata()
        .map_err(|_| "Cannot inspect process directory")?;
    let base = format!("/proc/self/fd/{}", dir.as_raw_fd());
    let text = String::from_utf8(read(Path::new(&format!("{base}/stat")), 65536)?)
        .map_err(|_| "Invalid process stat encoding")?;
    let (parent, start, name) = stat(&text)?;
    let cmd = read(Path::new(&format!("{base}/cmdline")), 65536)?;
    let boot = read(Path::new("/proc/sys/kernel/random/boot_id"), 128)?;
    let mut hash = Sha256::new();
    hash.update(b"nabiman-process-v1\0");
    hash.update(&boot);
    hash.update(pid.to_be_bytes());
    hash.update(meta.ino().to_be_bytes());
    hash.update(meta.uid().to_be_bytes());
    hash.update(start.to_be_bytes());
    hash.update(&cmd);
    let command = if cmd.is_empty() {
        format!("[{name}]")
    } else {
        String::from_utf8_lossy(&cmd)
            .replace('\0', " ")
            .trim_end()
            .into()
    };
    Ok(Identity {
        value: hex::encode(hash.finalize()),
        command,
        uid: meta.uid(),
        start,
        parent,
    })
}
pub(super) fn protected() -> Result<Vec<u32>, String> {
    let mut ids = vec![0, 1];
    let mut pid = std::process::id();
    for _ in 0..64 {
        if ids.contains(&pid) {
            return Ok(ids);
        }
        ids.push(pid);
        pid = inspect(pid)?.parent;
    }
    Err("Cannot establish management process ancestry".into())
}
