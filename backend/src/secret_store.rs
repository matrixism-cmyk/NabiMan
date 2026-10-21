//! At-rest encryption for stored secrets (currently remote-server passwords).
//!
//! The key lives in `$NABIMAN_DATA_DIR/secret.key` (0600, generated on first
//! use). Ciphertext is stored as `v1:<hex nonce>:<hex ciphertext>` so the
//! format can be rotated later without guessing what an old value is.

use aes_gcm::aead::{Aead, KeyInit};
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
    format!("{}/secret.key", data_dir())
}

fn load_or_create_key() -> Result<[u8; 32], String> {
    load_or_create_key_at(Path::new(&key_path()))
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

fn load_or_create_key_at(path: &Path) -> Result<[u8; 32], String> {
    match fs::symlink_metadata(path) {
        Ok(_) => return read_key(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("key metadata: {error}")),
    }
    let parent = path
        .parent()
        .ok_or_else(|| "invalid key path".to_string())?;
    fs::create_dir_all(parent).map_err(|e| format!("data dir: {e}"))?;
    refuse_lost_key(parent)?;
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

/// Encrypt a secret for storage. Returns `v1:<nonce>:<ciphertext>` in hex.
pub fn encrypt(plain: &str) -> Result<String, String> {
    let key = load_or_create_key()?;
    encrypt_with_key(&key, plain)
}

fn encrypt_with_key(key: &[u8; 32], plain: &str) -> Result<String, String> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let ct = cipher
        .encrypt(Nonce::from_slice(&nonce_bytes), plain.as_bytes())
        .map_err(|_| "encrypt failed".to_string())?;
    Ok(format!(
        "v1:{}:{}",
        hex::encode(nonce_bytes),
        hex::encode(ct)
    ))
}

/// Decrypt a value produced by [`encrypt`].
pub fn decrypt(stored: &str) -> Result<String, String> {
    let key = read_key(Path::new(&key_path()))?;
    decrypt_with_key(&key, stored)
}

fn decrypt_with_key(key: &[u8; 32], stored: &str) -> Result<String, String> {
    let parts: Vec<&str> = stored.splitn(3, ':').collect();
    if parts.len() != 3 || parts[0] != "v1" {
        return Err("unsupported secret format".into());
    }
    let nonce_bytes = hex::decode(parts[1]).map_err(|_| "bad nonce".to_string())?;
    let ct = hex::decode(parts[2]).map_err(|_| "bad ciphertext".to_string())?;
    if nonce_bytes.len() != 12 {
        return Err("bad nonce length".into());
    }
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let plain = cipher
        .decrypt(Nonce::from_slice(&nonce_bytes), ct.as_ref())
        .map_err(|_| "decrypt failed (wrong key?)".to_string())?;
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
        let enc = encrypt_with_key(&key, "hunter2!@#").unwrap();
        assert!(enc.starts_with("v1:"));
        assert!(!enc.contains("hunter2"));
        assert_eq!(decrypt_with_key(&key, &enc).unwrap(), "hunter2!@#");
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
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
