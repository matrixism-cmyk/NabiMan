//! Verify a portable, authenticated management-state bundle before publication.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::{Component, Path},
};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    sha256: String,
    size: u64,
    mode: u32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema_version: u32,
    kind: String,
    snapshot_id: String,
    created_at: String,
    files: BTreeMap<String, Entry>,
    excluded: Vec<String>,
}
#[derive(Serialize)]
pub struct Verification {
    pub snapshot_id: String,
    pub created_at: String,
    pub files: usize,
    pub accounts: usize,
    pub remote_servers: usize,
    pub encrypted_credentials_verified: usize,
    pub database_integrity: String,
    pub runtime_sha256: String,
    pub exclusions: Vec<String>,
}
pub fn digest(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path).map_err(|_| "Cannot read bundle file")?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|_| "Cannot hash bundle file")?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(hex::encode(hash.finalize()))
}
fn inventory(root: &Path, dir: &Path, found: &mut BTreeSet<String>) -> Result<(), String> {
    for item in fs::read_dir(dir).map_err(|_| "Cannot inspect bundle")? {
        let item = item.map_err(|_| "Cannot inspect bundle")?;
        let meta = item.file_type().map_err(|_| "Cannot inspect bundle")?;
        if meta.is_dir() {
            inventory(root, &item.path(), found)?;
        } else if meta.is_file() {
            let name = item
                .path()
                .strip_prefix(root)
                .map_err(|_| "Invalid bundle path")?
                .to_str()
                .ok_or("Invalid bundle path")?
                .to_string();
            if name != "manifest.json" {
                found.insert(name);
            }
        } else {
            return Err("Bundle contains a link or special file".into());
        }
    }
    Ok(())
}
fn json(path: &Path) -> Result<serde_json::Value, String> {
    let file = fs::File::open(path).map_err(|_| "Required bundle data missing")?;
    serde_json::from_reader(file.take(16 * 1024 * 1024))
        .map_err(|_| "Bundle JSON is invalid".into())
}
pub fn verify(root: &Path) -> Result<Verification, String> {
    let manifest: Manifest = serde_json::from_value(json(&root.join("manifest.json"))?)
        .map_err(|_| "Invalid bundle manifest")?;
    if manifest.schema_version != 1
        || manifest.kind != "nabiman-management-state"
        || uuid::Uuid::parse_str(&manifest.snapshot_id).is_err()
        || chrono::DateTime::parse_from_rfc3339(&manifest.created_at).is_err()
        || manifest.files.is_empty()
        || manifest.files.len() > 100_000
    {
        return Err("Unsupported bundle manifest".into());
    }
    for needed in [
        "auth/users.json",
        "auth/jwt_secret",
        "auth/terminal_sessions.json",
        "auth/terminal_settings.json",
        "auth/terminal_shares.json",
        "database/mec.db",
        "runtime/nabiman-server",
        "runtime/static/index.html",
    ] {
        if !manifest.files.contains_key(needed) {
            return Err("Required management data is missing".into());
        }
    }
    let mut found = BTreeSet::new();
    inventory(root, root, &mut found)?;
    if found != manifest.files.keys().cloned().collect() {
        return Err("Bundle inventory does not match manifest".into());
    }
    for (name, entry) in &manifest.files {
        let path = Path::new(name);
        if path
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
            || !(name.starts_with("auth/")
                || name.starts_with("runtime/static/")
                || matches!(name.as_str(), "runtime/nabiman-server" | "database/mec.db"))
            || name.ends_with(".lock")
            || name.starts_with("auth/backups/")
            || name.starts_with("auth/ssh/pw/")
        {
            return Err("Invalid management bundle member".into());
        }
        let expected_mode = if name == "runtime/nabiman-server" {
            0o700
        } else {
            0o600
        };
        if entry.mode != expected_mode
            || entry.sha256.len() != 64
            || !entry.sha256.bytes().all(|b| b.is_ascii_hexdigit())
            || fs::metadata(root.join(path))
                .map_err(|_| "Bundle member missing")?
                .len()
                != entry.size
            || digest(&root.join(path))? != entry.sha256
        {
            return Err("Bundle member integrity mismatch".into());
        }
    }
    let users = json(&root.join("auth/users.json"))?;
    let users = users.as_array().ok_or("Invalid user store")?;
    if users.is_empty() {
        return Err("User store is empty; refusing accidental bootstrap recovery".into());
    }
    let mut ids = BTreeSet::new();
    for user in users {
        let id = user
            .get("id")
            .and_then(|v| v.as_str())
            .filter(|v| !v.is_empty())
            .ok_or("Missing immutable user identity")?;
        if !ids.insert(id) {
            return Err("Duplicate user identity".into());
        }
    }
    if fs::read(root.join("auth/jwt_secret"))
        .map_err(|_| "Missing JWT key")?
        .len()
        < 32
    {
        return Err("Invalid JWT key".into());
    }
    for name in [
        "terminal_sessions.json",
        "terminal_settings.json",
        "terminal_shares.json",
    ] {
        json(&root.join("auth").join(name))?;
    }
    let servers = if root.join("auth/remote_servers.json").exists() {
        json(&root.join("auth/remote_servers.json"))?
    } else {
        serde_json::json!([])
    };
    let servers = servers.as_array().ok_or("Invalid server inventory")?;
    let mut encrypted: Vec<String> = servers
        .iter()
        .filter_map(|server| server.get("password_enc").and_then(|v| v.as_str()))
        .filter(|value| value.starts_with("v1:"))
        .map(str::to_owned)
        .collect();
    let forwarding = root.join("auth/audit-forwarding.sqlite3");
    if forwarding.exists() {
        let db = rusqlite::Connection::open_with_flags(
            &forwarding,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .map_err(|_| "Cannot open forwarding state")?;
        let check: String = db
            .query_row("PRAGMA quick_check", [], |r| r.get(0))
            .map_err(|_| "Invalid forwarding database")?;
        if check != "ok" {
            return Err("Invalid forwarding database".into());
        }
        let mut statement = db
            .prepare("SELECT auth_enc FROM destinations WHERE auth_enc!=''")
            .map_err(|_| "Invalid forwarding schema")?;
        let rows = statement
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(|_| "Cannot read forwarding credential references")?;
        for value in rows {
            let value = value.map_err(|_| "Invalid forwarding credential")?;
            if !value.starts_with("v1:") {
                return Err("Unsupported forwarding credential".into());
            }
            encrypted.push(value);
        }
    }
    if root.join("auth/secret.key").exists() || !encrypted.is_empty() {
        use aes_gcm::{
            aead::{Aead, KeyInit},
            Aes256Gcm, Nonce,
        };
        let key = hex::decode(
            fs::read_to_string(root.join("auth/secret.key"))
                .map_err(|_| "Missing stored credential key")?
                .trim(),
        )
        .map_err(|_| "Invalid stored credential key")?;
        let cipher =
            Aes256Gcm::new_from_slice(&key).map_err(|_| "Invalid stored credential key")?;
        for value in &encrypted {
            let parts: Vec<_> = value.split(':').collect();
            if parts.len() != 3 {
                return Err("Invalid encrypted credential".into());
            }
            let nonce = hex::decode(parts[1]).map_err(|_| "Invalid credential nonce")?;
            let data = hex::decode(parts[2]).map_err(|_| "Invalid credential ciphertext")?;
            if nonce.len() != 12 {
                return Err("Invalid credential nonce".into());
            }
            let _plain = cipher
                .decrypt(Nonce::from_slice(&nonce), data.as_slice())
                .map_err(|_| "Credential key does not decrypt stored server credentials")?;
        }
    }
    let db = rusqlite::Connection::open_with_flags(
        root.join("database/mec.db"),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .map_err(|_| "Cannot open recovered database")?;
    let integrity: String = db
        .query_row("PRAGMA quick_check", [], |r| r.get(0))
        .map_err(|_| "Database integrity check failed")?;
    if integrity != "ok" {
        return Err("Database integrity check failed".into());
    }
    for table in ["tenant_profiles", "jobs"] {
        let n: i64 = db
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name=?1",
                [table],
                |r| r.get(0),
            )
            .map_err(|_| "Invalid database schema")?;
        if n != 1 {
            return Err("Missing required database table".into());
        }
    }
    Ok(Verification {
        snapshot_id: manifest.snapshot_id,
        created_at: manifest.created_at,
        files: manifest.files.len(),
        accounts: users.len(),
        remote_servers: servers.len(),
        encrypted_credentials_verified: encrypted.len(),
        database_integrity: integrity,
        runtime_sha256: manifest.files["runtime/nabiman-server"].sha256.clone(),
        exclusions: manifest.excluded,
    })
}
/// Called only after verification, inside private staging, before atomic publication.
pub fn prepare(root: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    verify(root)?;
    fs::set_permissions(
        root.join("runtime/nabiman-server"),
        fs::Permissions::from_mode(0o700),
    )
    .map_err(|_| "Cannot restore runtime execution permission")?;
    fs::File::open(root.join("runtime/nabiman-server"))
        .and_then(|f| f.sync_all())
        .map_err(|_| "Cannot persist runtime metadata".into())
}
#[cfg(test)]
#[path = "recovery_bundle_tests.rs"]
mod tests;
