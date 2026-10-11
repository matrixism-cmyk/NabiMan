use super::protocol::ServiceAction;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, Read},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceRule {
    pub unit: String,
    pub actions: Vec<ServiceAction>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub api_uid: u32,
    pub services: Vec<ServiceRule>,
}

pub fn canonical_unit(name: &str) -> Option<String> {
    let unit = if name.ends_with(".service") {
        name.to_string()
    } else {
        format!("{name}.service")
    };
    let stem = unit.strip_suffix(".service")?;
    if stem.is_empty()
        || unit.len() > 255
        || !stem.as_bytes()[0].is_ascii_alphanumeric()
        || !stem
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.@".contains(&b))
        || stem.ends_with('@')
        || stem.contains("..")
    {
        return None;
    }
    Some(unit)
}
impl Policy {
    pub fn validate(&self) -> io::Result<()> {
        if self.api_uid == 0 || self.services.len() > 256 {
            return Err(invalid());
        }
        let mut names = std::collections::HashSet::new();
        for rule in &self.services {
            if canonical_unit(&rule.unit).as_deref() != Some(&rule.unit)
                || matches!(
                    rule.unit.as_str(),
                    "nabiman.service" | "nabiman-privileged-helper.service"
                )
                || !names.insert(&rule.unit)
                || rule.actions.is_empty()
                || rule.actions.len() > 5
            {
                return Err(invalid());
            }
            let mut actions = std::collections::HashSet::new();
            if rule.actions.iter().any(|a| !actions.insert(a.as_str())) {
                return Err(invalid());
            }
        }
        Ok(())
    }
    pub fn version(&self) -> String {
        use sha2::{Digest, Sha256};
        hex::encode(Sha256::digest(
            serde_json::to_vec(self).expect("Typed helper policy serializes"),
        ))
    }
    pub fn permits(&self, uid: u32, unit: &str, action: ServiceAction) -> bool {
        uid == self.api_uid
            && self
                .services
                .iter()
                .any(|r| r.unit == unit && r.actions.contains(&action))
    }
    /// Recheck the root-owned approval immediately before invoking a privileged command.
    /// Formatting-only rewrites keep the same semantic approval; unsafe/unreadable files deny.
    pub(crate) fn still_approved(&self, path: &Path) -> bool {
        Self::load(path).is_ok_and(|current| current.version() == self.version())
    }
    pub fn load(path: &Path) -> io::Result<Self> {
        validate_parent(path)?;
        let file = fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(path)?;
        let metadata = file.metadata()?;
        if !metadata.is_file()
            || metadata.uid() != 0
            || metadata.mode() & 0o077 != 0
            || metadata.len() > 65536
        {
            return Err(invalid());
        }
        let mut bytes = Vec::new();
        file.take(65537).read_to_end(&mut bytes)?;
        if bytes.len() > 65536 {
            return Err(invalid());
        }
        let policy: Self = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
        policy.validate()?;
        Ok(policy)
    }
}
pub(crate) fn validate_parent(path: &Path) -> io::Result<()> {
    if !path.is_absolute()
        || path.components().any(|c| {
            matches!(
                c,
                std::path::Component::ParentDir | std::path::Component::CurDir
            )
        })
    {
        return Err(invalid());
    }
    for parent in path.parent().ok_or_else(invalid)?.ancestors() {
        let metadata = fs::symlink_metadata(parent)?;
        if !metadata.is_dir() || metadata.uid() != 0 || metadata.mode() & 0o022 != 0 {
            return Err(invalid());
        }
    }
    Ok(())
}
fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "Unsafe privileged-helper configuration",
    )
}
