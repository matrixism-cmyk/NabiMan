//! Owners, groups and permission bits: shown in a listing, and changed for
//! one entry or many at once, optionally for everything inside folders.
//!
//! SFTP and this machine know owners by number. Names come from this
//! machine's /etc files, or from a server's own `getent` over its SSH shell;
//! an SFTP-only account has no shell, so there numbers are what it gets. FTP
//! listings name owners but FTP cannot change them, only permissions (SITE
//! CHMOD).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use actix_web::{web, HttpRequest, HttpResponse};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::fs::{Fs, Meta};
use super::paths::{join, parent, valid_path, Entry};
use super::{bad, resolve, Target};
use crate::models::{ApiResponse, RemoteServer};

/// How long a server's names are kept before they are asked for again.
const NAMES_TTL: Duration = Duration::from_secs(10 * 60);
/// A change that would touch more entries than this is refused.
const MAX_ENTRIES: usize = 20_000;
/// At most this many selected paths per request.
const MAX_PATHS: usize = 1000;
/// Names offered for completion.
const MAX_NAMES: usize = 1000;
const GROUPS_MARK: &str = "__NABIMAN_GROUPS__";

#[derive(Default)]
pub(crate) struct Names {
    pub users: HashMap<u32, String>,
    pub groups: HashMap<u32, String>,
}

/// `name:x:id:…` lines, as /etc/passwd, /etc/group and `getent` print them.
fn parse_ids(text: &str) -> HashMap<u32, String> {
    text.lines()
        .filter_map(|l| {
            let mut f = l.split(':');
            let name = f.next()?.trim();
            let id = f.nth(1)?.trim().parse().ok()?;
            (!name.is_empty()).then(|| (id, name.to_string()))
        })
        .collect()
}

pub(crate) fn parse_names(text: &str) -> Names {
    let (users, groups) = text.split_once(GROUPS_MARK).unwrap_or((text, ""));
    Names { users: parse_ids(users), groups: parse_ids(groups) }
}

fn cache() -> &'static Mutex<HashMap<String, (Instant, Arc<Names>)>> {
    static CACHE: OnceLock<Mutex<HashMap<String, (Instant, Arc<Names>)>>> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}

fn server_key(s: &RemoteServer) -> String {
    let port = s.port.to_string();
    let parts = [&s.id, &s.host, &port, &s.user, &s.host_trust_version, &s.password_enc];
    hex::encode(Sha256::digest(parts.map(|p| p.as_str()).join("\0")))
}

/// The names that go with owner numbers where they can be found: none for
/// FTP (its listings carry names) or an SFTP server without a shell.
pub(super) async fn names(t: &Target) -> Option<Arc<Names>> {
    let (key, server) = match &t.server {
        None => ("local".to_string(), None),
        Some(s) if s.terminal == "ssh" && s.files == "sftp" => (server_key(s), Some(s.clone())),
        Some(_) => return None,
    };
    if let Some((at, names)) = cache().lock().unwrap_or_else(|e| e.into_inner()).get(&key) {
        if at.elapsed() < NAMES_TTL {
            return Some(names.clone());
        }
    }
    let text = match server {
        None => {
            let users = tokio::fs::read_to_string("/etc/passwd").await.unwrap_or_default();
            let groups = tokio::fs::read_to_string("/etc/group").await.unwrap_or_default();
            format!("{users}\n{GROUPS_MARK}\n{groups}")
        }
        Some(server) => tokio::task::spawn_blocking(move || {
            let cmd = format!(
                "(getent passwd || cat /etc/passwd) 2>/dev/null | head -n 5000; echo {GROUPS_MARK}; (getent group || cat /etc/group) 2>/dev/null | head -n 5000"
            );
            let (mut ssh, _pw) = crate::remote_probe::ssh_command(&server, 8, &cmd).ok()?;
            let out = ssh.output().ok()?;
            out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
        })
        .await
        .ok()??,
    };
    let names = Arc::new(parse_names(&text));
    cache().lock().unwrap_or_else(|e| e.into_inner()).insert(key, (Instant::now(), names.clone()));
    Some(names)
}

/// Fills in names for owners known only by number.
pub(super) fn name_entries(entries: &mut [Entry], names: &Names) {
    for e in entries {
        if e.owner.is_none() {
            e.owner = e.uid.and_then(|u| names.users.get(&u).cloned());
        }
        if e.group.is_none() {
            e.group = e.gid.and_then(|g| names.groups.get(&g).cloned());
        }
    }
}

/// What the panel may offer for this target, and names to complete with.
pub(super) fn capabilities(fs: &Fs, names: Option<&Names>) -> serde_json::Value {
    let sorted = |m: &HashMap<u32, String>| {
        let mut v: Vec<&String> = m.values().collect();
        v.sort();
        v.dedup();
        v.truncate(MAX_NAMES);
        v.into_iter().cloned().collect::<Vec<_>>()
    };
    serde_json::json!({
        "mode": true,
        "owner": !matches!(fs, Fs::Ftp(_)),
        "users": names.map(|n| sorted(&n.users)).unwrap_or_default(),
        "groups": names.map(|n| sorted(&n.groups)).unwrap_or_default(),
    })
}

#[derive(Deserialize)]
pub(super) struct ChangeBody {
    paths: Vec<String>,
    /// Octal, such as `755` or `0644`.
    #[serde(default)]
    mode: Option<String>,
    /// A name or a number; absent or empty leaves it as it is.
    #[serde(default)]
    owner: Option<String>,
    #[serde(default)]
    group: Option<String>,
    /// Also everything inside the selected folders.
    #[serde(default)]
    recursive: bool,
}

pub(crate) fn parse_mode(text: &str) -> Option<u32> {
    let t = text.trim();
    if !(3..=4).contains(&t.len()) || !t.chars().all(|c| ('0'..='7').contains(&c)) {
        return None;
    }
    u32::from_str_radix(t, 8).ok()
}

/// A number, or a name the target knows.
fn id_of(text: &str, known: Option<&HashMap<u32, String>>, what: &str) -> Result<u32, String> {
    if let Ok(n) = text.parse() {
        return Ok(n);
    }
    known
        .and_then(|m| m.iter().find(|(_, n)| n.as_str() == text).map(|(id, _)| *id))
        .ok_or_else(|| format!("Unknown {what} '{text}'; give its number instead"))
}

/// A folder given read access also gets search (x), as `chmod -R a+X` does,
/// so applying a file's mode to a tree does not lock its folders.
pub(crate) fn folder_mode(mode: u32) -> u32 {
    mode | ((mode & 0o444) >> 2)
}

pub(super) async fn change(req: HttpRequest, b: web::Json<ChangeBody>) -> HttpResponse {
    let t = match resolve(&req) {
        Ok(t) => t,
        Err(r) => return r,
    };
    if b.paths.is_empty() || b.paths.len() > MAX_PATHS || !b.paths.iter().all(|p| valid_path(p)) {
        return bad("Invalid selection");
    }
    if b.recursive && b.paths.iter().any(|p| parent(p).is_none()) {
        return bad("Not for everything under the root folder; use the terminal");
    }
    let mode = match b.mode.as_deref().filter(|m| !m.trim().is_empty()) {
        Some(m) => match parse_mode(m) {
            Some(mode) => Some(mode),
            None => return bad("Permissions are three or four octal digits, such as 755"),
        },
        None => None,
    };
    let owner = b.owner.as_deref().map(str::trim).filter(|o| !o.is_empty());
    let group = b.group.as_deref().map(str::trim).filter(|g| !g.is_empty());
    if mode.is_none() && owner.is_none() && group.is_none() {
        return bad("Nothing to change");
    }
    let fs = match t.fs().await {
        Ok(fs) => fs,
        Err(r) => return r,
    };
    if matches!(fs, Fs::Ftp(_)) && (owner.is_some() || group.is_some()) {
        return bad("FTP cannot change owners or groups; only permissions");
    }
    let known = if owner.is_some() || group.is_some() { names(&t).await } else { None };
    let uid = match owner.map(|o| id_of(o, known.as_ref().map(|n| &n.users), "user")).transpose() {
        Ok(v) => v,
        Err(e) => return bad(&e),
    };
    let gid = match group.map(|g| id_of(g, known.as_ref().map(|n| &n.groups), "group")).transpose() {
        Ok(v) => v,
        Err(e) => return bad(&e),
    };
    let (targets, links) = match collect(&fs, &b.paths, b.recursive).await {
        Ok(found) => found,
        Err(e) => return e.into_response(&t),
    };
    let mut failed = Vec::new();
    let mut failures = 0usize;
    for (path, meta) in &targets {
        // The owner first: changing it can clear set-id bits a mode would set.
        let mut result = Ok(());
        if uid.is_some() || gid.is_some() {
            result = fs.chown(path, uid, gid).await;
        }
        if let (Ok(()), Some(mode)) = (&result, mode) {
            let mode = if b.recursive && meta.is_dir() { folder_mode(mode) } else { mode };
            result = fs.chmod(path, mode).await;
        }
        if let Err(e) = result {
            if e.broken {
                return t.error(e);
            }
            failures += 1;
            if failed.len() < 20 {
                failed.push(serde_json::json!({ "path": path, "message": e.message }));
            }
        }
    }
    HttpResponse::Ok().json(ApiResponse::ok(serde_json::json!({
        "changed": targets.len() - failures, "failed": failed, "failed_count": failures, "links_skipped": links,
    })))
}

enum CollectError {
    Fs(super::fs::FsError),
    TooMany,
}

impl CollectError {
    fn into_response(self, t: &Target) -> HttpResponse {
        match self {
            CollectError::Fs(e) => t.error(e),
            CollectError::TooMany => bad("Too many entries to change from here; use the terminal"),
        }
    }
}

/// The entries to change, and how many links were left alone: changing a
/// link would change what it points to, which may be anywhere.
async fn collect(fs: &Fs, paths: &[String], recursive: bool) -> Result<(Vec<(String, Meta)>, usize), CollectError> {
    let mut out = Vec::new();
    let mut links = 0;
    let mut folders = Vec::new();
    for path in paths {
        let meta = fs.lstat(path).await.map_err(CollectError::Fs)?;
        if meta.kind == "link" {
            links += 1;
            continue;
        }
        if recursive && meta.is_dir() {
            folders.push(path.clone());
        }
        out.push((path.clone(), meta));
    }
    while let Some(folder) = folders.pop() {
        for (name, meta) in fs.read_dir(&folder).await.map_err(CollectError::Fs)? {
            let path = join(&folder, &name);
            match meta.kind {
                "link" => links += 1,
                "dir" => {
                    folders.push(path.clone());
                    out.push((path, meta));
                }
                _ => out.push((path, meta)),
            }
        }
        if out.len() > MAX_ENTRIES {
            return Err(CollectError::TooMany);
        }
    }
    Ok((out, links))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_are_three_or_four_octal_digits() {
        assert_eq!(parse_mode("755"), Some(0o755));
        assert_eq!(parse_mode(" 0644 "), Some(0o644));
        assert_eq!(parse_mode("4755"), Some(0o4755));
        for bad in ["", "75", "789", "12345", "rwx", "-755"] {
            assert_eq!(parse_mode(bad), None, "{bad}");
        }
    }

    #[test]
    fn folders_get_search_where_they_get_read() {
        assert_eq!(folder_mode(0o644), 0o755);
        assert_eq!(folder_mode(0o640), 0o750);
        assert_eq!(folder_mode(0o600), 0o700);
        assert_eq!(folder_mode(0o000), 0o000);
    }

    #[test]
    fn names_come_from_passwd_and_group_lines() {
        let text = "root:x:0:0:root:/root:/bin/bash\nweb:x:1001:1001::/home/web:/bin/sh\n__NABIMAN_GROUPS__\nroot:x:0:\nwww:x:33:web\nbroken\n";
        let n = parse_names(text);
        assert_eq!(n.users.get(&1001).map(String::as_str), Some("web"));
        assert_eq!(n.groups.get(&33).map(String::as_str), Some("www"));
        assert_eq!(n.groups.len(), 2);
        assert_eq!(id_of("web", Some(&n.users), "user"), Ok(1001));
        assert_eq!(id_of("1234", None, "user"), Ok(1234));
        assert!(id_of("nobody-here", Some(&n.users), "user").is_err());
    }
}
