//! State and secrets are for this server alone: at startup the data directory
//! and everything in it lose their group and other permissions (files 0600-
//! like, directories 0700-like). Files this process owns are fixed; others
//! (another account's, e.g. after a manual copy) are reported, not touched.
//!
//! Exceptions: the data directory may keep group/other *execute* (traverse,
//! no listing), because `serial/` (device links shared with NABIMAN_SERIAL_GROUP)
//! and `guac-drive/` (the RDP transfer drive guacd reads) are reached through
//! it; those two directories are left exactly as they are. Public keys
//! (`*.pub`) are public by nature and not reported.

use std::fs;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

pub const SHARED: &[&str] = &["serial", "guac-drive"];
const MAX_DEPTH: usize = 16;
const MAX_ENTRIES: usize = 100_000;

#[derive(Default, Debug)]
pub struct Report {
    pub fixed: Vec<PathBuf>,
    /// Too open, but not ours to change: (path, mode, owner uid).
    pub foreign: Vec<(PathBuf, u32, u32)>,
}

#[derive(Debug, PartialEq)]
pub(crate) enum Action {
    Fine,
    Fix(u32),
    Warn,
}

/// `mask`: the bits that must be clear.
pub(crate) fn plan(mode: u32, owner: u32, me: u32, mask: u32) -> Action {
    let mode = mode & 0o7777;
    if mode & mask == 0 {
        Action::Fine
    } else if owner == me {
        Action::Fix(mode & !mask)
    } else {
        Action::Warn
    }
}

pub fn enforce(dir: &Path) -> Report {
    let me = unsafe { libc::geteuid() };
    let mut report = Report::default();
    let mut count = 0;
    // The directory itself: no listing or writing by others; traversal stays.
    check(dir, true, 0o066, me, &mut report);
    walk(dir, 0, me, &mut report, &mut count);
    report
}

fn walk(dir: &Path, depth: usize, me: u32, report: &mut Report, count: &mut usize) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        *count += 1;
        if *count > MAX_ENTRIES || depth > MAX_DEPTH {
            return;
        }
        let path = entry.path();
        if depth == 0 && entry.file_name().to_str().is_some_and(|n| SHARED.contains(&n)) {
            continue;
        }
        let Ok(meta) = fs::symlink_metadata(&path) else { continue };
        if meta.is_dir() {
            check(&path, true, 0o077, me, report);
            walk(&path, depth + 1, me, report, count);
        } else if meta.is_file() && path.extension().is_none_or(|e| e != "pub") {
            check(&path, false, 0o077, me, report);
        }
        // Links, sockets and FIFOs: never followed, never changed.
    }
}

fn check(path: &Path, directory: bool, mask: u32, me: u32, report: &mut Report) {
    let Ok(meta) = fs::symlink_metadata(path) else { return };
    match plan(meta.mode(), meta.uid(), me, mask) {
        Action::Fine => {}
        Action::Warn => report.foreign.push((path.to_path_buf(), meta.mode() & 0o7777, meta.uid())),
        Action::Fix(mode) => {
            // Through a descriptor opened without following links, so a swapped-in
            // symlink can never redirect the change.
            let flags = libc::O_NOFOLLOW | libc::O_NONBLOCK | if directory { libc::O_DIRECTORY } else { 0 };
            let fixed = fs::OpenOptions::new()
                .read(true)
                .custom_flags(flags)
                .open(path)
                .and_then(|f| f.set_permissions(fs::Permissions::from_mode(mode)));
            if fixed.is_ok() {
                report.fixed.push(path.to_path_buf());
            } else {
                report.foreign.push((path.to_path_buf(), meta.mode() & 0o7777, meta.uid()));
            }
        }
    }
}

/// One summary for the log (the first few paths, then a count).
pub fn log(dir: &Path, report: &Report) {
    let show = |paths: Vec<String>| {
        let more = paths.len().saturating_sub(8);
        let mut text = paths.into_iter().take(8).collect::<Vec<_>>().join(", ");
        if more > 0 {
            text += &format!(" and {more} more");
        }
        text
    };
    let name = |p: &Path| p.strip_prefix(dir).ok().filter(|r| !r.as_os_str().is_empty()).unwrap_or(p).display().to_string();
    if !report.fixed.is_empty() {
        log_info!(
            "data_dir",
            "data directory permissions: removed group/other access",
            files = show(report.fixed.iter().map(|p| name(p)).collect())
        );
    }
    if !report.foreign.is_empty() {
        log_warn!(
            "data_dir",
            "data directory permissions: readable or writable by group/others but not owned by this server, left as they are; fix with chmod go-rwx (go-rw for the directory)",
            uid = unsafe { libc::geteuid() },
            files = show(report.foreign.iter().map(|(p, mode, uid)| format!("{} ({:o}, uid {uid})", name(p), mode)).collect()),
            path = dir.display()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mode(path: &Path) -> u32 {
        fs::symlink_metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[test]
    fn own_files_and_directories_lose_group_and_other_access_shared_ones_do_not() {
        let dir = std::env::temp_dir().join(format!("nabiman-perms-{}", uuid::Uuid::new_v4()));
        for sub in ["ssh-ca", "serial", "claude-voice/chats"] {
            fs::create_dir_all(dir.join(sub)).unwrap();
        }
        let set = |p: &str, m: u32| fs::set_permissions(dir.join(p), fs::Permissions::from_mode(m)).unwrap();
        for (file, m) in [("ssh-ca/ca.pub", 0o644), ("users.json", 0o644), ("secret.key", 0o600), ("ssh-ca/ca", 0o640), ("serial/link.ack", 0o664), ("claude-voice/chats/1.json", 0o666)] {
            fs::write(dir.join(file), b"x").unwrap();
            set(file, m);
        }
        std::os::unix::fs::symlink("/etc/passwd", dir.join("link")).unwrap();
        set("", 0o775);
        set("ssh-ca", 0o755);
        set("serial", 0o775);
        let before_target = mode(Path::new("/etc/passwd"));

        let report = enforce(&dir);
        assert!(report.foreign.is_empty(), "{report:?}");
        assert_eq!(mode(&dir), 0o711, "traversal kept for the shared subdirectories");
        assert_eq!(mode(&dir.join("users.json")), 0o600);
        assert_eq!(mode(&dir.join("secret.key")), 0o600);
        assert_eq!(mode(&dir.join("ssh-ca")), 0o700);
        assert_eq!(mode(&dir.join("ssh-ca/ca")), 0o600);
        assert_eq!(mode(&dir.join("ssh-ca/ca.pub")), 0o644, "public keys stay readable");
        assert_eq!(mode(&dir.join("claude-voice/chats/1.json")), 0o600);
        assert_eq!(mode(&dir.join("serial")), 0o775, "shared with the serial group");
        assert_eq!(mode(&dir.join("serial/link.ack")), 0o664);
        assert_eq!(mode(Path::new("/etc/passwd")), before_target, "links are never followed");
        assert_eq!(report.fixed.len(), 7, "the directory, users.json, ssh-ca, ssh-ca/ca, claude-voice, chats, 1.json");
        // Nothing left to do the second time.
        assert!(enforce(&dir).fixed.is_empty());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn only_the_owner_is_changed_anything_else_is_reported() {
        assert_eq!(plan(0o100600, 1000, 1000, 0o077), Action::Fine);
        assert_eq!(plan(0o100644, 1000, 1000, 0o077), Action::Fix(0o600));
        assert_eq!(plan(0o100644, 1000, 0, 0o077), Action::Warn, "root does not take over another account's file");
        assert_eq!(plan(0o40775, 0, 0, 0o066), Action::Fix(0o711));
        assert_eq!(plan(0o40711, 0, 0, 0o066), Action::Fine);
    }
}
