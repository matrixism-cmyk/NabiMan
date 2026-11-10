//! Taking a snapshot: copy into private staging, write the manifest, pack, publish.
use super::*;
use crate::backup_crypto::{restore as staging, source};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{self, Read, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;

const MAX_DEPTH: usize = 32;
const MAX_ENTRIES: usize = 100_000;

struct Copier {
    include_key: bool,
    key_present: bool,
    tree: PathBuf,
    files: BTreeMap<String, Entry>,
    excluded: Vec<String>,
    count: usize,
}

/// Snapshot `data_dir` into `output_dir/<prefix>-<UTC time>.tar.gz` (0600).
/// Safe while the server runs. Returns the archive's file name.
pub fn create(data_dir: &Path, output_dir: &Path, prefix: &str, options: &Options) -> Result<String, String> {
    if prefix.is_empty() || !prefix.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err("Invalid snapshot name prefix".into());
    }
    let root = fs::canonicalize(data_dir).map_err(|_| "Data directory unavailable")?;
    staging::directory(output_dir).map_err(|_| "Backup storage unavailable")?;
    let output = fs::canonicalize(output_dir).map_err(|_| "Backup storage unavailable")?;
    let stage = staging::Stage::new(&output)?;
    let tree = stage.0.join("tree");
    staging::directory(&tree).map_err(|_| "Cannot stage snapshot")?;
    let mut copier = Copier { include_key: options.include_secret_key, key_present: false, tree, files: BTreeMap::new(), excluded: Vec::new(), count: 0 };
    for (name, why) in EXCLUDED_TOP {
        if root.join(name).exists() {
            copier.excluded.push(format!("{name}: {why}"));
        }
    }
    copier.directory(&root, "", 0, &output)?;
    if let Some(mec) = options.mec_db.as_ref().filter(|p| p.is_file()) {
        let inside = fs::canonicalize(mec).is_ok_and(|p| p.starts_with(&root));
        if !inside && !copier.files.contains_key("mec.db") {
            copier.sqlite(mec, "mec.db")?;
        }
    }
    if copier.key_present {
        copier.excluded.push(format!("{SECRET_KEY}: kept out by default; store it separately and supply it at restore"));
    }
    let manifest = Manifest {
        format: FORMAT.into(),
        format_version: FORMAT_VERSION,
        created_at: chrono::Utc::now().to_rfc3339(),
        nabiman_version: env!("CARGO_PKG_VERSION").into(),
        secret_key_included: copier.files.contains_key(SECRET_KEY),
        files: copier.files,
        excluded: copier.excluded,
    };
    let json = serde_json::to_vec_pretty(&manifest).map_err(|_| "Cannot encode snapshot manifest")?;
    let mut file = staging::private_file(&copier.tree.join(MANIFEST)).map_err(|_| "Cannot stage snapshot")?;
    file.write_all(&json).and_then(|_| file.sync_all()).map_err(|_| "Cannot stage snapshot")?;
    let packed = stage.0.join("snapshot.tar.gz");
    let out = staging::private_file(&packed).map_err(|_| "Cannot stage snapshot")?;
    source::write_tree(out, &copier.tree, &output)?
        .sync_all()
        .map_err(|_| "Cannot persist snapshot")?;
    let name = format!("{prefix}-{}.tar.gz", chrono::Utc::now().format("%Y%m%dT%H%M%SZ"));
    // hard_link never replaces an existing archive.
    fs::hard_link(&packed, output.join(&name)).map_err(|_| "Cannot publish snapshot (name already taken?)")?;
    fs::File::open(&output)
        .and_then(|d| d.sync_all())
        .map_err(|_| "Snapshot published but durability could not be confirmed")?;
    Ok(name)
}

impl Copier {
    fn directory(&mut self, dir: &Path, relative: &str, depth: usize, output: &Path) -> Result<(), String> {
        if depth > MAX_DEPTH {
            return Err("Data directory is nested too deeply".into());
        }
        let mut entries: Vec<_> = fs::read_dir(dir)
            .map_err(|_| "Cannot read data directory")?
            .collect::<Result<_, _>>()
            .map_err(|_| "Cannot read data directory")?;
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            self.count += 1;
            if self.count > MAX_ENTRIES {
                return Err("Data directory has too many entries".into());
            }
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                self.excluded.push(format!("{relative}<non-UTF-8 name>: not representable"));
                continue;
            };
            let rel = if relative.is_empty() { name.clone() } else { format!("{relative}/{name}") };
            let path = entry.path();
            if depth == 0 && name == SECRET_KEY && !self.include_key {
                self.key_present = true;
                continue;
            }
            if skipped_name(&name)
                || (depth == 0 && (name == MANIFEST || EXCLUDED_TOP.iter().any(|(n, _)| *n == name)))
                || rel == "ssh/pw"
                || path == output
            {
                continue;
            }
            let meta = fs::symlink_metadata(&path).map_err(|_| "Cannot inspect data directory")?;
            if meta.is_dir() {
                staging::directory(&self.tree.join(&rel)).map_err(|_| "Cannot stage snapshot")?;
                self.directory(&path, &rel, depth + 1, output)?;
            } else if !meta.is_file() {
                self.excluded.push(format!("{rel}: link or special file"));
            } else if is_sqlite(&path) {
                self.sqlite(&path, &rel)?;
            } else {
                self.file(&path, &rel)?;
            }
        }
        Ok(())
    }

    fn file(&mut self, path: &Path, rel: &str) -> Result<(), String> {
        let source = fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(path)
            .map_err(|_| format!("Cannot read {rel}"))?;
        let length = source.metadata().map_err(|_| format!("Cannot read {rel}"))?.len();
        let mut target = staging::private_file(&self.tree.join(rel)).map_err(|_| "Cannot stage snapshot")?;
        let mut hasher = Sha256::new();
        let mut buffer = vec![0u8; 64 * 1024];
        // Up to the length seen at open: a log appended meanwhile is captured as a prefix.
        let mut reader = source.take(length);
        let mut size = 0u64;
        loop {
            let n = reader.read(&mut buffer).map_err(|_| format!("Cannot read {rel}"))?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
            target.write_all(&buffer[..n]).map_err(|_| "Cannot stage snapshot")?;
            size += n as u64;
        }
        target.sync_all().map_err(|_| "Cannot stage snapshot")?;
        self.files.insert(rel.into(), Entry { sha256: hex::encode(hasher.finalize()), size, sqlite_user_version: None });
        Ok(())
    }

    fn sqlite(&mut self, path: &Path, rel: &str) -> Result<(), String> {
        let copy = self.tree.join(rel);
        let fail = |what: &str| format!("Cannot snapshot database {rel}: {what}");
        let db = rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE)
            .map_err(|e| fail(&e.to_string()))?;
        db.busy_timeout(std::time::Duration::from_secs(30)).map_err(|e| fail(&e.to_string()))?;
        db.execute("VACUUM INTO ?1", [copy.to_string_lossy()]).map_err(|e| fail(&e.to_string()))?;
        drop(db);
        fs::set_permissions(&copy, fs::Permissions::from_mode(0o600)).map_err(|_| fail("permissions"))?;
        let check = rusqlite::Connection::open_with_flags(&copy, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| fail(&e.to_string()))?;
        let ok: String = check.query_row("PRAGMA quick_check", [], |r| r.get(0)).map_err(|e| fail(&e.to_string()))?;
        if ok != "ok" {
            return Err(fail("integrity check of the copy failed"));
        }
        let version: i64 = check.query_row("PRAGMA user_version", [], |r| r.get(0)).map_err(|e| fail(&e.to_string()))?;
        drop(check);
        fs::File::open(&copy).and_then(|f| f.sync_all()).map_err(|_| fail("sync"))?;
        let (sha256, size) = digest(&copy).map_err(|_| fail("hash"))?;
        self.files.insert(rel.into(), Entry { sha256, size, sqlite_user_version: Some(version) });
        Ok(())
    }
}

pub(crate) fn digest(path: &Path) -> io::Result<(String, u64)> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let size = io::copy(&mut file, &mut hasher)?;
    Ok((hex::encode(hasher.finalize()), size))
}
