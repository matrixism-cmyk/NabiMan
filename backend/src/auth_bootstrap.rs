use super::*;

pub fn new_password_store() -> PasswordStore {
    // Existing user databases contain their own password hashes.
    if data_dir().join("users.json").exists() {
        return Arc::new(std::sync::Mutex::new(String::new()));
    }
    let configured = std::env::var("NABIMAN_PASSWORD").ok();
    let raw = match configured {
        Some(password) => {
            validate_bootstrap_password(&password, true).unwrap_or_else(|error| panic!("{error}"));
            password
        }
        None => {
            let password = load_password_from_file()
                .expect("Could not read legacy password file")
                .expect("Set NABIMAN_PASSWORD before the first start");
            validate_bootstrap_password(&password, false).unwrap_or_else(|error| panic!("{error}"));
            password
        }
    };
    let hash = if raw.starts_with("$2b$") || raw.starts_with("$2a$") {
        raw
    } else {
        let h = bcrypt::hash(&raw, 10).expect("bcrypt hash failed");
        save_hash_to_file(&h).expect("Could not persist bootstrap password");
        println!("Password auto-migrated to bcrypt");
        h
    };
    Arc::new(std::sync::Mutex::new(hash))
}

pub fn new_jwt_secret() -> JwtSecret {
    Arc::new(load_or_create_jwt_secret(&data_dir().join("jwt_secret")))
}

pub(super) fn load_or_create_jwt_secret(path: &std::path::Path) -> String {
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    if let Ok(metadata) = std::fs::symlink_metadata(&path) {
        assert!(
            metadata.file_type().is_file(),
            "JWT secret must be a regular file"
        );
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
            .expect("Could not restrict JWT secret permissions");
        let secret = std::fs::read_to_string(&path).expect("Could not read JWT secret");
        let secret = secret.trim();
        assert!(!secret.is_empty(), "JWT secret file is empty");
        return secret.to_string();
    }
    use rand::Rng;
    let secret: String = rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(64)
        .map(char::from)
        .collect();
    std::fs::create_dir_all(path.parent().unwrap()).expect("Could not create JWT secret directory");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .expect("Could not create JWT secret");
    use std::io::Write;
    file.write_all(secret.as_bytes())
        .expect("Could not write JWT secret");
    file.sync_all().expect("Could not sync JWT secret");
    println!("JWT secret generated");
    secret
}

pub(super) fn data_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(
        std::env::var("NABIMAN_DATA_DIR").unwrap_or_else(|_| "/var/lib/nabiman".into()),
    )
}
pub(super) fn password_file_path() -> std::path::PathBuf {
    data_dir().join("password")
}
pub(super) fn load_password_from_file() -> std::io::Result<Option<String>> {
    match std::fs::read_to_string(password_file_path()) {
        Ok(password) => Ok(Some(password.trim_end_matches(['\r', '\n']).to_string())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

pub(crate) fn validate_bootstrap_password(password: &str, configured: bool) -> Result<(), String> {
    let known = ["nabiman", "changeme"];
    if password.is_empty() {
        return Err("Bootstrap password is empty".into());
    }
    if password.starts_with("$2b$") || password.starts_with("$2a$") {
        if configured {
            return Err("NABIMAN_PASSWORD must be plaintext, not a bcrypt hash".into());
        }
        if known
            .iter()
            .any(|value| bcrypt::verify(value, password).unwrap_or(false))
        {
            return Err("Replace the published default password before starting".into());
        }
    } else if known.contains(&password) {
        return Err("Replace the published default password before starting".into());
    } else if configured && password.len() < 12 {
        return Err("NABIMAN_PASSWORD must be at least 12 characters".into());
    }
    Ok(())
}
pub(super) fn save_hash_to_file(hash: &str) -> Result<(), String> {
    use std::os::unix::fs::OpenOptionsExt;
    let path = password_file_path();
    if let Some(p) = path.parent() {
        let _ = std::fs::create_dir_all(p);
    }
    std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&path)
        .and_then(|mut f| {
            use std::io::Write;
            f.write_all(hash.as_bytes())
        })
        .map_err(|e| e.to_string())
}
