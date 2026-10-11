//! Serialized local persistence. A failed append never appears as persisted in memory.
use super::{compute_hash, AuditEntry};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::PathBuf,
};
pub(super) const MAX_FILE: u64 = 12_000_000;
pub struct State {
    pub entries: Vec<AuditEntry>,
    pub path: PathBuf,
    pub error: Option<&'static str>,
    pub last_persisted_at: Option<String>,
    pub unparsed_lines: usize,
    previous: String,
    identity: Option<(u64, u64, u64)>,
}
impl State {
    pub fn load(path: PathBuf) -> Self {
        let mut state = Self {
            entries: vec![],
            path,
            error: None,
            last_persisted_at: None,
            unparsed_lines: 0,
            previous: "0".into(),
            identity: None,
        };
        let result = (|| {
            match fs::symlink_metadata(&state.path) {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
                Err(_) => return Err("read_failed"),
                Ok(_) => (),
            }
            let bytes = read(&state.path)?;
            if !bytes.is_empty() && !bytes.ends_with(b"\n") {
                return Err("incomplete_record");
            }
            for line in bytes.split(|b| *b == b'\n').filter(|line| !line.is_empty()) {
                let entry: AuditEntry = match serde_json::from_slice(line) {
                    Ok(entry) => entry,
                    Err(_) => {
                        state.unparsed_lines += 1;
                        continue;
                    }
                };
                state.previous = entry.hash.clone().unwrap_or_else(|| state.previous.clone());
                state.entries.push(entry);
            }
            let len = state.entries.len();
            if len > 1000 {
                state.entries.drain(..len - 1000);
            }
            state.last_persisted_at = state.entries.last().map(|e| e.timestamp.clone());
            state.identity = Some(identity(&state.path)?);
            Ok(())
        })();
        state.error = result.err();
        state
    }
    pub fn append(&mut self, mut entry: AuditEntry) -> Result<AuditEntry, &'static str> {
        if let Some(error) = self.error {
            return Err(error);
        }
        entry.hash = Some("v2:".into());
        entry.hash = Some(compute_hash(&self.previous, &entry));
        let result = (|| {
            let parent = self.path.parent().ok_or("invalid_path")?;
            fs::create_dir_all(parent).map_err(|_| "directory_failed")?;
            match self.identity {
                Some(expected) if identity(&self.path)? != expected => return Err("file_changed"),
                None if self.path.exists() => return Err("file_changed"),
                _ => (),
            }
            let mut options = OpenOptions::new();
            options
                .write(true)
                .append(true)
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
            if self.identity.is_none() {
                options.create_new(true);
            }
            let mut file = options.open(&self.path).map_err(|_| "open_failed")?;
            let meta = file.metadata().map_err(|_| "metadata_failed")?;
            if !meta.is_file() {
                return Err("not_regular");
            }
            let mut bytes = serde_json::to_vec(&entry).map_err(|_| "encode_failed")?;
            bytes.push(b'\n');
            file.write_all(&bytes).map_err(|_| "write_failed")?;
            file.sync_all().map_err(|_| "sync_failed")?;
            fs::File::open(parent)
                .and_then(|f| f.sync_all())
                .map_err(|_| "directory_sync_failed")?;
            self.identity = Some(identity(&self.path)?);
            self.previous = entry.hash.clone().unwrap();
            if self.identity.unwrap().2 > 10_000_000 {
                for i in 1..=5 {
                    let path = self.path.with_extension(format!("jsonl.{i}"));
                    match fs::symlink_metadata(&path) {
                        Ok(meta) if !meta.is_file() => return Err("rotation_target_invalid"),
                        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                            return Err("rotation_failed")
                        }
                        _ => (),
                    }
                }
                for i in (1..5).rev() {
                    let from = self.path.with_extension(format!("jsonl.{i}"));
                    if from.exists() {
                        fs::rename(from, self.path.with_extension(format!("jsonl.{}", i + 1)))
                            .map_err(|_| "rotation_failed")?;
                    }
                }
                fs::rename(&self.path, self.path.with_extension("jsonl.1"))
                    .map_err(|_| "rotation_failed")?;
                fs::File::open(parent)
                    .and_then(|f| f.sync_all())
                    .map_err(|_| "directory_sync_failed")?;
                self.previous = "0".into();
                self.identity = None;
            }
            Ok(())
        })();
        if let Err(error) = result {
            self.error = Some(error);
            eprintln!("Local audit persistence unavailable ({error}); operation outcome is unchanged; inspect storage before restarting");
            return Err(error);
        }
        self.last_persisted_at = Some(entry.timestamp.clone());
        self.entries.push(entry.clone());
        if self.entries.len() > 1000 {
            self.entries.remove(0);
        }
        Ok(entry)
    }
}
fn identity(path: &std::path::Path) -> Result<(u64, u64, u64), &'static str> {
    let meta = fs::symlink_metadata(path).map_err(|_| "metadata_failed")?;
    if !meta.is_file() {
        return Err("not_regular");
    }
    Ok((meta.dev(), meta.ino(), meta.len()))
}
pub(super) fn read(path: &std::path::Path) -> Result<Vec<u8>, &'static str> {
    use std::io::Read;
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|_| "read_failed")?;
    let meta = file.metadata().map_err(|_| "metadata_failed")?;
    if !meta.is_file() {
        return Err("not_regular");
    }
    if meta.len() > MAX_FILE {
        return Err("too_large");
    }
    let mut bytes = vec![];
    file.take(MAX_FILE + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "read_failed")?;
    if bytes.len() as u64 > MAX_FILE {
        return Err("too_large");
    }
    Ok(bytes)
}
