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

fn data_dir() -> String {
    std::env::var("NABIMAN_DATA_DIR").unwrap_or_else(|_| "/var/lib/nabiman".to_string())
}

fn key_path() -> String {
    format!("{}/secret.key", data_dir())
}

fn load_or_create_key() -> Result<[u8; 32], String> {
    let path = key_path();
    if let Ok(content) = fs::read_to_string(&path) {
        if let Ok(bytes) = hex::decode(content.trim()) {
            if bytes.len() == 32 {
                let mut key = [0u8; 32];
                key.copy_from_slice(&bytes);
                return Ok(key);
            }
        }
    }
    let mut key = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut key);
    fs::create_dir_all(data_dir()).map_err(|e| format!("data dir: {}", e))?;
    let mut f = fs::OpenOptions::new()
        .write(true).create(true).truncate(true).mode(0o600)
        .open(&path).map_err(|e| format!("key file: {}", e))?;
    f.write_all(hex::encode(key).as_bytes()).map_err(|e| format!("key write: {}", e))?;
    let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    Ok(key)
}

/// Encrypt a secret for storage. Returns `v1:<nonce>:<ciphertext>` in hex.
pub fn encrypt(plain: &str) -> Result<String, String> {
    let key = load_or_create_key()?;
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let ct = cipher
        .encrypt(Nonce::from_slice(&nonce_bytes), plain.as_bytes())
        .map_err(|_| "encrypt failed".to_string())?;
    Ok(format!("v1:{}:{}", hex::encode(nonce_bytes), hex::encode(ct)))
}

/// Decrypt a value produced by [`encrypt`].
pub fn decrypt(stored: &str) -> Result<String, String> {
    let parts: Vec<&str> = stored.splitn(3, ':').collect();
    if parts.len() != 3 || parts[0] != "v1" {
        return Err("unsupported secret format".into());
    }
    let nonce_bytes = hex::decode(parts[1]).map_err(|_| "bad nonce".to_string())?;
    let ct = hex::decode(parts[2]).map_err(|_| "bad ciphertext".to_string())?;
    if nonce_bytes.len() != 12 {
        return Err("bad nonce length".into());
    }
    let key = load_or_create_key()?;
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
    let plain = cipher
        .decrypt(Nonce::from_slice(&nonce_bytes), ct.as_ref())
        .map_err(|_| "decrypt failed (wrong key?)".to_string())?;
    String::from_utf8(plain).map_err(|_| "secret is not valid UTF-8".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let dir = std::env::temp_dir().join(format!("nabiman_sec_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var("NABIMAN_DATA_DIR", &dir);
        let enc = encrypt("hunter2!@#").unwrap();
        assert!(enc.starts_with("v1:"));
        assert!(!enc.contains("hunter2"));
        assert_eq!(decrypt(&enc).unwrap(), "hunter2!@#");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
