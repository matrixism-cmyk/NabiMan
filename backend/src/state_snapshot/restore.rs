//! Restoring a snapshot into a new directory: extract into private staging,
//! check every file against the manifest, add the separately kept key, then
//! publish with an atomic no-replace rename. An existing directory is never touched.
use super::create::digest;
use super::*;
use crate::backup_crypto::restore as staging;
use serde::Serialize;
use std::collections::BTreeSet;
use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

#[derive(Serialize, Debug)]
pub struct Restored {
    pub created_at: String,
    pub nabiman_version: String,
    pub files: usize,
    /// Each database and the schema version it was captured at.
    pub databases: BTreeMap<String, i64>,
    /// "included" (in the archive), "supplied" (given at restore) or "missing".
    pub secret_key: String,
    /// Stored credentials (server passwords, forwarding tokens); checked to decrypt
    /// with the key unless `secret_key` is "missing".
    pub encrypted_credentials: usize,
}

/// Restore `archive` into `target`, which must not exist yet (its parent must).
/// `secret_key` is the separately kept `secret.key`, when the archive lacks it.
pub fn restore(archive: &Path, target: &str, secret_key: Option<&Path>) -> Result<Restored, String> {
    let target = staging::target(target)?;
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(archive)
        .map_err(|_| "Snapshot archive unavailable")?;
    if !file.metadata().map_err(|_| "Snapshot archive unavailable")?.is_file() {
        return Err("Snapshot archive is not a regular file".into());
    }
    let stage = staging::Stage::new(target.parent().ok_or("Missing restore parent")?)?;
    let tree = stage.0.join("tree");
    staging::directory(&tree).map_err(|_| "Cannot stage restore")?;
    staging::extract(file, &tree)?;
    let manifest = check(&tree)?;
    let mut key_state = if manifest.secret_key_included { "included" } else { "missing" };
    if let Some(path) = secret_key {
        let key = read_key(path)?;
        let staged = tree.join(SECRET_KEY);
        if staged.exists() {
            if read_key(&staged)? != key {
                return Err("The supplied secret.key differs from the one in the archive".into());
            }
        } else {
            let mut out = staging::private_file(&staged).map_err(|_| "Cannot stage secret.key")?;
            out.write_all(key.as_bytes()).and_then(|_| out.sync_all()).map_err(|_| "Cannot stage secret.key")?;
            key_state = "supplied";
        }
    }
    let credentials = credentials(&tree)?;
    if !credentials.is_empty() && key_state != "missing" {
        decrypts(&read_key(&tree.join(SECRET_KEY))?, &credentials)?;
    }
    // Kept for provenance, hidden so the restored directory's own snapshots skip it.
    fs::rename(tree.join(MANIFEST), tree.join(RESTORED_FROM)).map_err(|_| "Cannot stage restore")?;
    staging::publish(&tree, &target)?;
    Ok(summary(&manifest, key_state, credentials.len()))
}

/// Check an extracted snapshot tree against its manifest.
pub fn verify(tree: &Path) -> Result<Restored, String> {
    let manifest = check(tree)?;
    let key = if tree.join(SECRET_KEY).exists() { "included" } else { "missing" };
    Ok(summary(&manifest, key, 0))
}

fn summary(manifest: &Manifest, key: &str, credentials: usize) -> Restored {
    Restored {
        created_at: manifest.created_at.clone(),
        nabiman_version: manifest.nabiman_version.clone(),
        files: manifest.files.len(),
        databases: manifest
            .files
            .iter()
            .filter_map(|(name, e)| e.sqlite_user_version.map(|v| (name.clone(), v)))
            .collect(),
        secret_key: key.into(),
        encrypted_credentials: credentials,
    }
}

fn check(tree: &Path) -> Result<Manifest, String> {
    let mut text = String::new();
    fs::File::open(tree.join(MANIFEST))
        .and_then(|f| f.take(64 * 1024 * 1024).read_to_string(&mut text))
        .map_err(|_| "Snapshot manifest missing")?;
    let manifest: Manifest = serde_json::from_str(&text).map_err(|_| "Snapshot manifest is invalid")?;
    if manifest.format != FORMAT || manifest.format_version != FORMAT_VERSION {
        return Err("Not a NabiMan state snapshot, or a newer format than this NabiMan reads".into());
    }
    let mut found = BTreeSet::new();
    inventory(tree, tree, &mut found)?;
    found.remove(MANIFEST);
    if manifest.secret_key_included != found.contains(SECRET_KEY) {
        return Err("Snapshot secret.key does not match its manifest".into());
    }
    if found != manifest.files.keys().cloned().collect() {
        return Err("Snapshot contents do not match its manifest".into());
    }
    for (name, entry) in &manifest.files {
        let path = tree.join(name);
        if digest(&path).map_err(|_| "Cannot read restored file")? != (entry.sha256.clone(), entry.size) {
            return Err(format!("Snapshot file {name} is damaged (checksum mismatch)"));
        }
        if entry.sqlite_user_version.is_some() {
            let db = rusqlite::Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
                .map_err(|_| format!("Cannot open restored database {name}"))?;
            let ok: String = db
                .query_row("PRAGMA quick_check", [], |r| r.get(0))
                .map_err(|_| format!("Restored database {name} is damaged"))?;
            if ok != "ok" {
                return Err(format!("Restored database {name} is damaged"));
            }
        }
    }
    Ok(manifest)
}

fn inventory(root: &Path, dir: &Path, found: &mut BTreeSet<String>) -> Result<(), String> {
    for item in fs::read_dir(dir).map_err(|_| "Cannot inspect restored tree")? {
        let item = item.map_err(|_| "Cannot inspect restored tree")?;
        let kind = item.file_type().map_err(|_| "Cannot inspect restored tree")?;
        if kind.is_dir() {
            inventory(root, &item.path(), found)?;
        } else if kind.is_file() {
            let path = item.path();
            let name = path.strip_prefix(root).ok().and_then(|p| p.to_str()).ok_or("Invalid restored path")?;
            found.insert(name.to_string());
        } else {
            return Err("Restored tree contains a link or special file".into());
        }
    }
    Ok(())
}

fn read_key(path: &Path) -> Result<String, String> {
    let mut text = String::new();
    fs::File::open(path)
        .and_then(|f| f.take(256).read_to_string(&mut text))
        .map_err(|_| "Cannot read secret.key")?;
    let key = text.trim().to_string();
    if key.len() != 64 || !key.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("secret.key is not a NabiMan key (64 hex characters)".into());
    }
    Ok(key)
}

/// Every `v1:` ciphertext the restored state holds.
fn credentials(tree: &Path) -> Result<Vec<String>, String> {
    let mut found = Vec::new();
    if let Ok(text) = fs::read_to_string(tree.join("remote_servers.json")) {
        let servers: serde_json::Value = serde_json::from_str(&text).map_err(|_| "Restored remote_servers.json is invalid")?;
        for server in servers.as_array().into_iter().flatten() {
            if let Some(value) = server.get("password_enc").and_then(|v| v.as_str()).filter(|v| v.starts_with("v1:")) {
                found.push(value.to_string());
            }
        }
    }
    let forwarding = tree.join("audit-forwarding.sqlite3");
    if forwarding.exists() {
        let db = rusqlite::Connection::open_with_flags(&forwarding, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|_| "Cannot open restored forwarding state")?;
        let mut q = db
            .prepare("SELECT auth_enc FROM destinations WHERE auth_enc LIKE 'v1:%'")
            .map_err(|_| "Invalid restored forwarding state")?;
        let rows = q.query_map([], |r| r.get::<_, String>(0)).map_err(|_| "Invalid restored forwarding state")?;
        for row in rows {
            found.push(row.map_err(|_| "Invalid restored forwarding state")?);
        }
    }
    Ok(found)
}

/// The key must actually open what it is restored with (a wrong key is refused here, not at first use).
fn decrypts(key: &str, values: &[String]) -> Result<(), String> {
    use aes_gcm::aead::{Aead, KeyInit};
    let key = hex::decode(key).map_err(|_| "Invalid secret.key")?;
    let cipher = aes_gcm::Aes256Gcm::new_from_slice(&key).map_err(|_| "Invalid secret.key")?;
    for value in values {
        let parts: Vec<_> = value.split(':').collect();
        let (Some(nonce), Some(data)) = (parts.get(1).and_then(|n| hex::decode(n).ok()), parts.get(2).and_then(|d| hex::decode(d).ok())) else {
            return Err("A stored credential is malformed".into());
        };
        if parts.len() != 3 || nonce.len() != 12 {
            return Err("A stored credential is malformed".into());
        }
        cipher
            .decrypt(aes_gcm::Nonce::from_slice(&nonce), data.as_slice())
            .map_err(|_| "This secret.key does not decrypt the stored credentials (wrong key?)")?;
    }
    Ok(())
}
