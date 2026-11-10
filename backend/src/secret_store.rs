//! At-rest encryption for stored secrets (remote-server and console
//! passwords, the dictation key, audit-forwarding credentials).
//!
//! The key lives in `NABIMAN_SECRET_KEY_FILE`, by default
//! `$NABIMAN_DATA_DIR/secret.key` (0600, generated on first use). Keeping it
//! outside the data directory means a copy of the data alone decrypts
//! nothing.
//!
//! AES-256-GCM, written as `<version>:<hex nonce>:<hex ciphertext>`:
//! - `v2` binds the ciphertext to where it is stored ([`Binding`]: record
//!   kind, record id, field) as associated data, so a value copied into
//!   another record, or another field, no longer decrypts.
//! - `v1` (no associated data) is still read; a record re-encrypts it as v2
//!   the next time it is saved ([`upgrade`]).

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use rand::RngCore;
use std::fs;
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;

fn data_dir() -> String {
    nabiman_server::data_dir().to_string_lossy().into_owned()
}

fn key_path() -> String {
    std::env::var("NABIMAN_SECRET_KEY_FILE")
        .ok()
        .filter(|p| !p.trim().is_empty())
        .unwrap_or_else(|| format!("{}/secret.key", data_dir()))
}

fn load_or_create_key() -> Result<[u8; 32], String> {
    load_or_create_key_in(Path::new(&key_path()), Path::new(&data_dir()))
}

fn read_key(path: &Path) -> Result<[u8; 32], String> {
    let metadata = fs::symlink_metadata(path).map_err(|e| format!("key metadata: {e}"))?;
    if !metadata.file_type().is_file() {
        return Err("secret key must be a regular file".into());
    }
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|e| format!("key permissions: {e}"))?;
    let content = fs::read_to_string(path).map_err(|e| format!("key read: {e}"))?;
    let bytes = hex::decode(content.trim()).map_err(|_| "secret key is corrupt".to_string())?;
    if bytes.len() != 32 {
        return Err("secret key has the wrong length".into());
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(&bytes);
    Ok(key)
}

#[cfg(test)]
fn load_or_create_key_at(path: &Path) -> Result<[u8; 32], String> {
    load_or_create_key_in(path, path.parent().unwrap_or(Path::new("/")))
}

/// The key at `path`, created when missing — unless `data` (the data
/// directory, which may not be where the key is) holds secrets that only
/// the missing key could open.
fn load_or_create_key_in(path: &Path, data: &Path) -> Result<[u8; 32], String> {
    match fs::symlink_metadata(path) {
        Ok(_) => return read_key(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("key metadata: {error}")),
    }
    let parent = path
        .parent()
        .ok_or_else(|| "invalid key path".to_string())?;
    fs::create_dir_all(parent).map_err(|e| format!("key dir: {e}"))?;
    refuse_lost_key(data)?;
    let mut key = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut key);
    let temp = parent.join(format!(".secret-key-{}.tmp", uuid::Uuid::new_v4()));
    let linked = (|| -> std::io::Result<()> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temp)?;
        file.write_all(hex::encode(key).as_bytes())?;
        file.sync_all()?;
        fs::hard_link(&temp, path)
    })();
    if let Err(error) = fs::remove_file(&temp) {
        if error.kind() != std::io::ErrorKind::NotFound {
            return Err(format!("temporary key cleanup: {error}"));
        }
    }
    match linked {
        Ok(()) => {
            fs::File::open(parent)
                .and_then(|dir| dir.sync_all())
                .map_err(|_| "key directory sync failed".to_string())?;
            Ok(key)
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => read_key(path),
        Err(error) => Err(format!("key creation: {error}")),
    }
}

fn refuse_lost_key(parent: &Path) -> Result<(), String> {
    let servers = parent.join("remote_servers.json");
    if servers.exists() {
        if fs::metadata(&servers)
            .map_err(|_| "Cannot inspect stored credential references")?
            .len()
            > 16 * 1024 * 1024
        {
            return Err("Credential inventory too large to verify missing key".into());
        }
        let value: serde_json::Value = serde_json::from_slice(
            &fs::read(servers).map_err(|_| "Cannot read credential inventory")?,
        )
        .map_err(|_| "Cannot verify credential inventory")?;
        let rows = value.as_array().ok_or("Invalid credential inventory")?;
        if rows.iter().any(|r| {
            r.get("password_enc")
                .and_then(|v| v.as_str())
                .is_some_and(|v| !v.is_empty())
        }) {
            return Err("Stored credential key is missing; restore the original key".into());
        }
    }
    let forwarding = parent.join("audit-forwarding.sqlite3");
    if forwarding.exists() {
        let db = rusqlite::Connection::open_with_flags(
            forwarding,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .map_err(|_| "Cannot inspect forwarding credential references")?;
        let count: i64 = db
            .query_row(
                "SELECT count(*) FROM destinations WHERE auth_enc!=''",
                [],
                |r| r.get(0),
            )
            .map_err(|_| "Cannot verify forwarding credential references")?;
        if count > 0 {
            return Err("Stored credential key is missing; restore the original key".into());
        }
    }
    Ok(())
}

/// Where a secret is stored: bound into its ciphertext (v2), so it opens
/// only there. `kind` names the store, `id` the record, `field` the column.
#[derive(Clone, Copy, Debug)]
pub struct Binding<'a> {
    pub kind: &'a str,
    pub id: &'a str,
    pub field: &'a str,
}

impl Binding<'_> {
    fn aad(&self) -> Vec<u8> {
        // Length-prefixed, so no two different bindings read the same.
        let mut out = b"nabiman-secret-v2".to_vec();
        for part in [self.kind, self.id, self.field] {
            out.extend_from_slice(&(part.len() as u32).to_be_bytes());
            out.extend_from_slice(part.as_bytes());
        }
        out
    }
}

impl<'a> Binding<'a> {
    /// A saved server's login password.
    pub fn server_password(id: &'a str) -> Self {
        Binding { kind: "remote-server", id, field: "password" }
    }
    /// A remote console's (RDP) password.
    pub fn console_password(id: &'a str) -> Self {
        Binding { kind: "console", id, field: "password" }
    }
    /// The dictation service's API key (one per installation).
    pub fn dictation_key() -> Self {
        Binding { kind: "dictation", id: "settings", field: "key" }
    }
}

/// Encrypt a secret not tied to a record (v1). Prefer [`seal`].
pub fn encrypt(plain: &str) -> Result<String, String> {
    let key = load_or_create_key()?;
    encrypt_with_key(&key, plain, None)
}

/// Encrypt a secret for the record and field `at` (v2).
pub fn seal(at: Binding, plain: &str) -> Result<String, String> {
    let key = load_or_create_key()?;
    encrypt_with_key(&key, plain, Some(at))
}

fn encrypt_with_key(key: &[u8; 32], plain: &str, at: Option<Binding>) -> Result<String, String> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let aad = at.map(|b| b.aad()).unwrap_or_default();
    let ct = cipher
        .encrypt(Nonce::from_slice(&nonce_bytes), Payload { msg: plain.as_bytes(), aad: &aad })
        .map_err(|_| "encrypt failed".to_string())?;
    let version = if at.is_some() { "v2" } else { "v1" };
    Ok(format!("{version}:{}:{}", hex::encode(nonce_bytes), hex::encode(ct)))
}

/// Decrypt a v1 value (one not tied to a record).
pub fn decrypt(stored: &str) -> Result<String, String> {
    let key = read_key(Path::new(&key_path()))?;
    decrypt_with_key(&key, stored, None)
}

/// Decrypt the secret stored at `at`: a v2 value sealed for exactly that
/// record and field, or a v1 value from before bindings.
pub fn open(at: Binding, stored: &str) -> Result<String, String> {
    let key = read_key(Path::new(&key_path()))?;
    decrypt_with_key(&key, stored, Some(at))
}

/// A v1 value re-encrypted as v2 for `at`, for the record's next save;
/// `None` when it is already v2 (or empty, or unreadable: left as it is).
pub fn upgrade(at: Binding, stored: &str) -> Option<String> {
    if !stored.starts_with("v1:") {
        return None;
    }
    upgrade_with_key(&read_key(Path::new(&key_path())).ok()?, at, stored)
}

fn upgrade_with_key(key: &[u8; 32], at: Binding, stored: &str) -> Option<String> {
    let plain = decrypt_with_key(key, stored, Some(at)).ok()?;
    encrypt_with_key(key, &plain, Some(at)).ok()
}

fn decrypt_with_key(key: &[u8; 32], stored: &str, at: Option<Binding>) -> Result<String, String> {
    let parts: Vec<&str> = stored.splitn(3, ':').collect();
    let aad = match (parts.first().copied(), at) {
        (Some("v1"), _) if parts.len() == 3 => Vec::new(),
        (Some("v2"), Some(at)) if parts.len() == 3 => at.aad(),
        (Some("v2"), None) => return Err("this secret belongs to a record; open it with its binding".into()),
        _ => return Err("unsupported secret format".into()),
    };
    let nonce_bytes = hex::decode(parts[1]).map_err(|_| "bad nonce".to_string())?;
    let ct = hex::decode(parts[2]).map_err(|_| "bad ciphertext".to_string())?;
    if nonce_bytes.len() != 12 {
        return Err("bad nonce length".into());
    }
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let plain = cipher
        .decrypt(Nonce::from_slice(&nonce_bytes), Payload { msg: &ct, aad: &aad })
        .map_err(|_| "decrypt failed (wrong key, or not this record's secret)".to_string())?;
    String::from_utf8(plain).map_err(|_| "secret is not valid UTF-8".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temporary_key_path() -> PathBuf {
        std::env::temp_dir()
            .join(format!("nabiman-key-test-{}", uuid::Uuid::new_v4()))
            .join("secret.key")
    }

    #[test]
    fn roundtrip() {
        let path = temporary_key_path();
        let key = load_or_create_key_at(&path).unwrap();
        let enc = encrypt_with_key(&key, "hunter2!@#", None).unwrap();
        assert!(enc.starts_with("v1:"));
        assert!(!enc.contains("hunter2"));
        assert_eq!(decrypt_with_key(&key, &enc, None).unwrap(), "hunter2!@#");
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    const A: Binding = Binding { kind: "remote-server", id: "srv_a", field: "password" };

    #[test]
    fn v2_opens_only_where_it_was_sealed() {
        let key = [7u8; 32];
        let enc = encrypt_with_key(&key, "s3cret", Some(A)).unwrap();
        assert!(enc.starts_with("v2:"));
        assert_eq!(decrypt_with_key(&key, &enc, Some(A)).unwrap(), "s3cret");
        for other in [
            Binding { id: "srv_b", ..A },
            Binding { field: "key", ..A },
            Binding { kind: "console", ..A },
            // Moving bytes between parts does not make another binding read the same.
            Binding { kind: "remote-serversrv_a", id: "", ..A },
        ] {
            assert!(decrypt_with_key(&key, &enc, Some(other)).is_err(), "{other:?}");
        }
        assert!(decrypt_with_key(&key, &enc, None).is_err(), "no binding, no v2");
        assert!(decrypt_with_key(&key, &enc.replacen("v2:", "v1:", 1), Some(A)).is_err(), "no downgrade");
    }

    #[test]
    fn v1_values_still_open_with_or_without_a_binding() {
        let key = [9u8; 32];
        let old = encrypt_with_key(&key, "legacy", None).unwrap();
        assert_eq!(decrypt_with_key(&key, &old, Some(A)).unwrap(), "legacy");
        assert_eq!(decrypt_with_key(&key, &old, None).unwrap(), "legacy");
        assert!(decrypt_with_key(&key, "v3:00:00", Some(A)).is_err());
        let upgraded = upgrade_with_key(&key, A, &old).unwrap();
        assert!(upgraded.starts_with("v2:"));
        assert_eq!(decrypt_with_key(&key, &upgraded, Some(A)).unwrap(), "legacy");
        assert!(upgrade_with_key(&[1u8; 32], A, &old).is_none(), "unreadable stays as it is");
    }

    #[test]
    fn a_key_kept_outside_the_data_directory_is_still_guarded() {
        let base = std::env::temp_dir().join(format!("nabiman-key-split-{}", uuid::Uuid::new_v4()));
        let (keys, data) = (base.join("keys"), base.join("data"));
        fs::create_dir_all(&data).unwrap();
        fs::write(data.join("remote_servers.json"), br#"[{"password_enc":"v2:00:00"}]"#).unwrap();
        assert!(load_or_create_key_in(&keys.join("secret.key"), &data).is_err(), "would orphan stored secrets");
        fs::write(data.join("remote_servers.json"), b"[]").unwrap();
        let key = load_or_create_key_in(&keys.join("secret.key"), &data).unwrap();
        assert_eq!(read_key(&keys.join("secret.key")).unwrap(), key);
        assert!(!data.join("secret.key").exists());
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn corrupt_existing_key_is_never_replaced() {
        let path = temporary_key_path();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "invalid-key").unwrap();
        assert!(load_or_create_key_at(&path).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "invalid-key");
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn concurrent_first_use_shares_one_private_key() {
        let path = temporary_key_path();
        let handles: Vec<_> = (0..12)
            .map(|_| {
                let path = path.clone();
                std::thread::spawn(move || load_or_create_key_at(&path).unwrap())
            })
            .collect();
        let keys: Vec<_> = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect();
        assert!(keys.iter().all(|key| key == &keys[0]));
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
}

#[cfg(test)]
mod missing_key_tests {
    use super::*;
    #[test]
    fn existing_ciphertexts_prevent_silent_key_replacement() {
        let dir = std::env::temp_dir().join(format!("nb-missing-key-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&dir).unwrap();
        let key = dir.join("secret.key");
        fs::write(
            dir.join("remote_servers.json"),
            br#"[{"password_enc":"v1:existing:ciphertext"}]"#,
        )
        .unwrap();
        assert!(load_or_create_key_at(&key).is_err());
        assert!(!key.exists());
        fs::write(dir.join("remote_servers.json"), b"[]").unwrap();
        let db = rusqlite::Connection::open(dir.join("audit-forwarding.sqlite3")).unwrap();
        db.execute_batch("CREATE TABLE destinations(auth_enc TEXT); INSERT INTO destinations VALUES('v1:existing:ciphertext');").unwrap();
        assert!(load_or_create_key_at(&key).is_err());
        assert!(!key.exists());
        drop(db);
        fs::remove_dir_all(dir).unwrap();
    }
}
