use super::ApiKey;
use std::{
    fs,
    io::{self, Read, Write},
    os::unix::fs::OpenOptionsExt,
    path::PathBuf,
};
pub struct Store {
    pub keys: Vec<ApiKey>,
    pub failed: bool,
    path: PathBuf,
}
impl Store {
    pub fn open(path: PathBuf) -> io::Result<Self> {
        let mut bytes = Vec::new();
        match fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(&path)
        {
            Ok(file) => {
                if !file.metadata()?.is_file() {
                    return Err(io::Error::other("Token database must be a regular file"));
                }
                file.take(2 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
                if bytes.len() > 2 * 1024 * 1024 {
                    return Err(io::Error::other("Token database too large"));
                }
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => bytes.extend_from_slice(b"[]"),
            Err(e) => return Err(e),
        }
        let keys: Vec<ApiKey> = serde_json::from_slice(&bytes)?;
        let mut ids = std::collections::HashSet::new();
        if keys.len() > 1000 || keys.iter().any(|k| !ids.insert(&k.id) || k.version > 1) {
            return Err(io::Error::other("Unsupported or duplicate token records"));
        }
        Ok(Self {
            keys,
            failed: false,
            path,
        })
    }
    pub fn commit(&mut self, keys: Vec<ApiKey>) -> io::Result<()> {
        if self.failed {
            return Err(io::Error::other("Token storage unavailable"));
        }
        self.failed = true;
        let parent = self
            .path
            .parent()
            .ok_or_else(|| io::Error::other("Missing parent"))?;
        fs::create_dir_all(parent)?;
        let bytes = serde_json::to_vec_pretty(&keys)?;
        if bytes.len() > 2 * 1024 * 1024 {
            return Err(io::Error::other("Token database too large"));
        }
        let stage = parent.join(format!(".api-tokens-{}", uuid::Uuid::new_v4()));
        let result = (|| {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&stage)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            fs::rename(&stage, &self.path)?;
            fs::File::open(parent)?.sync_all()?;
            Ok(())
        })();
        if result.is_err() {
            // Includes uncertain publication after rename: freeze authentication and mutation.
            self.failed = true;
            let _ = fs::remove_file(stage);
        } else {
            self.keys = keys;
            self.failed = false;
        }
        result
    }
}
