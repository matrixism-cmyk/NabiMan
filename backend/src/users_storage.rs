use super::*;

pub(super) fn rotate_published_admin_password(
    users: &mut [NabimanUser],
    replacement: Option<&str>,
) -> Result<bool, String> {
    let admin = match users
        .iter_mut()
        .find(|user| user.username == "admin" && !user.password_hash.is_empty())
    {
        Some(user) => user,
        None => return Ok(false),
    };
    let published = ["nabiman", "changeme"];
    if !published
        .iter()
        .any(|password| bcrypt::verify(password, &admin.password_hash).unwrap_or(false))
    {
        return Ok(false);
    }
    let replacement = replacement.ok_or_else(|| {
        "Existing admin uses a published default password; set NABIMAN_PASSWORD to rotate it"
            .to_string()
    })?;
    crate::auth::validate_bootstrap_password(replacement, true)?;
    admin.share_authorization_version = uuid::Uuid::new_v4().to_string();
    admin.password_hash = bcrypt::hash(replacement, 10).map_err(|error| error.to_string())?;
    Ok(true)
}

pub(super) fn users_file_path() -> std::path::PathBuf {
    #[cfg(test)]
    if let Some(path) = TEST_FILE.with(|slot| slot.borrow().clone()) {
        return path;
    }
    let dir = std::env::var("NABIMAN_DATA_DIR").unwrap_or_else(|_| "/var/lib/nabiman".into());
    std::path::PathBuf::from(dir).join("users.json")
}

/// Flush the observed database before accepting it after an interrupted publication.
pub(super) fn read_users_at(path: &std::path::Path) -> std::io::Result<String> {
    use std::{io::Read, os::unix::fs::OpenOptionsExt};
    // Nonblocking open lets us reject FIFOs without waiting for a writer.
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)?;
    (|| {
        if !file.metadata()?.is_file() {
            return Err(std::io::Error::other("User database is not a regular file"));
        }
        file.sync_all()?;
        sync_directory(
            path.parent()
                .ok_or_else(|| std::io::Error::other("Invalid users path"))?,
        )?;
        let mut data = String::new();
        file.read_to_string(&mut data)?;
        Ok(data)
    })()
    .map_err(|error: std::io::Error| {
        std::io::Error::other(format!("User database validation failed: {error}"))
    })
    // Only the initial open may report NotFound and allow first-time bootstrap.
}

pub(super) fn sync_directory(path: &std::path::Path) -> std::io::Result<()> {
    std::fs::File::open(path)?.sync_all()
}

#[derive(Debug)]
pub(super) struct UserPublicationFailure {
    error: String,
    pub(super) published: bool,
}
impl std::fmt::Display for UserPublicationFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.error)
    }
}

/// A post-rename error cannot safely roll memory back to the old authority.
/// Stop all workers/connections; restart must flush and validate the observed file.
pub(super) fn reject_ambiguous_publication(failure: UserPublicationFailure) -> String {
    if failure.published {
        eprintln!(
            "User database publication is uncertain; terminating server for storage recovery"
        );
        std::process::exit(78);
    }
    failure.error
}

pub(super) fn save_users(users: &[NabimanUser]) -> Result<(), String> {
    save_users_at(users, &users_file_path()).map_err(reject_ambiguous_publication)
}

pub(super) fn save_users_at(
    users: &[NabimanUser],
    path: &std::path::Path,
) -> Result<(), UserPublicationFailure> {
    save_users_with_sync(users, path, sync_directory)
}

pub(super) fn save_users_with_sync(
    users: &[NabimanUser],
    path: &std::path::Path,
    sync_parent: impl FnOnce(&std::path::Path) -> std::io::Result<()>,
) -> Result<(), UserPublicationFailure> {
    use std::os::unix::fs::OpenOptionsExt;
    let mut published = false;
    let parent = path.parent().ok_or_else(|| UserPublicationFailure {
        error: "Invalid users path".into(),
        published: false,
    })?;
    let temp = parent.join(format!(".users-{}.tmp", generate_id()));
    let result = (|| -> Result<(), String> {
        use std::io::Write;
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let json = serialize_users(users)?;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temp)
            .map_err(|e| e.to_string())?;
        file.write_all(json.as_bytes()).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        std::fs::rename(&temp, path).map_err(|e| e.to_string())?;
        published = true;
        sync_parent(parent).map_err(|e| e.to_string())?;
        Ok(())
    })();
    if result.is_err() && !published {
        let _ = std::fs::remove_file(&temp);
    }
    result.map_err(|error| UserPublicationFailure { error, published })
}

pub(super) fn serialize_users(users: &[NabimanUser]) -> Result<String, String> {
    let stored: Vec<StoredUser<'_>> = users
        .iter()
        .map(|user| StoredUser {
            id: &user.id,
            username: &user.username,
            share_authorization_version: &user.share_authorization_version,
            password_hash: &user.password_hash,
            role: &user.role,
            status: &user.status,
            memberships: &user.memberships,
            requested_memberships: &user.requested_memberships,
            scope_restricted: user.scope_restricted,
            created_at: &user.created_at,
            last_login: &user.last_login,
            totp_enabled: user.totp_enabled,
            totp_secret: &user.totp_secret,
            mfa: &user.mfa,
        })
        .collect();
    serde_json::to_string_pretty(&stored).map_err(|e| e.to_string())
}

/// Rotate on security changes, never on last-login or scope bookkeeping.
pub(super) fn refresh_share_versions(users: &mut [NabimanUser], previous: &[NabimanUser]) {
    for user in users {
        let changed = previous
            .iter()
            .find(|old| old.id == user.id)
            .map(|old| {
                old.username != user.username
                    || old.role != user.role
                    || old.status != user.status
                    || old.password_hash != user.password_hash
                    || old.totp_enabled != user.totp_enabled
                    || old.totp_secret != user.totp_secret
                    || old.mfa.revision != user.mfa.revision
            })
            .unwrap_or(true);
        if changed || user.share_authorization_version.is_empty() {
            user.share_authorization_version = uuid::Uuid::new_v4().to_string();
        }
    }
}

pub(crate) fn commit_users(
    users: &mut Vec<NabimanUser>,
    previous: Vec<NabimanUser>,
) -> Result<(), String> {
    commit_users_with(users, previous, save_users)
}

pub(super) fn commit_users_with(
    users: &mut Vec<NabimanUser>,
    previous: Vec<NabimanUser>,
    persist: impl FnOnce(&[NabimanUser]) -> Result<(), String>,
) -> Result<(), String> {
    refresh_share_versions(users, &previous);
    if let Err(error) = persist(users) {
        *users = previous;
        return Err(error);
    }
    Ok(())
}

pub(crate) fn generate_id() -> String {
    use rand::Rng;
    let id: u64 = rand::thread_rng().gen();
    format!("{:016x}", id)
}

#[cfg(test)]
thread_local! { static TEST_FILE: std::cell::RefCell<Option<std::path::PathBuf>> = const { std::cell::RefCell::new(None) }; }
#[cfg(test)]
pub(crate) struct TestStorage(std::path::PathBuf);
#[cfg(test)]
impl TestStorage {
    pub(crate) fn file(&self) -> std::path::PathBuf {
        self.0.join("users.json")
    }
}
#[cfg(test)]
impl Drop for TestStorage {
    fn drop(&mut self) {
        TEST_FILE.with(|slot| *slot.borrow_mut() = None);
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[cfg(test)]
pub(crate) fn isolated_test_storage() -> TestStorage {
    let dir = std::env::temp_dir().join(format!("nb-user-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&dir).unwrap();
    TEST_FILE.with(|slot| {
        assert!(slot.borrow().is_none());
        *slot.borrow_mut() = Some(dir.join("users.json"));
    });
    TestStorage(dir)
}
