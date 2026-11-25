//! Saving moved devices: a NEW file beside the configured one, never a
//! change to a file that exists.
//!
//! In order: read what the configured path leads to and check it (the
//! caller's version, the moves); claim `rack_layout_YYYYMMDD_vN.json`
//! (Korea date, N one more than the folder has for that date) with an
//! exclusive create, going on to N+1 while the name is taken; write the
//! text to a temporary file, sync it, rename it onto the claimed name;
//! then point the configured path at it — a link is replaced by a new link
//! (made under a temporary name, renamed over it); a plain file is replaced
//! by a copy the same way, the versioned files staying as its history.
//! Whatever fails after the claim removes the new file again.
//!
//! Every name written is made here, in the configured file's own folder.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use chrono::{DateTime, FixedOffset, SecondsFormat, Utc};

use super::json::Json;
use super::layout::{self, Layout};
use super::moves::{self, Move, Moved, Refusal};

const PREFIX: &str = "rack_layout_";
/// Names tried past the first before giving up on a folder others fill.
const MAX_CLAIMS: u32 = 100;

#[derive(Debug)]
pub(crate) enum Failure {
    /// The feature is off on this install.
    Off,
    /// The layout cannot be read or is not a layout (`Fault`'s codes).
    Unusable { code: &'static str, detail: String },
    /// The file now has another version than the editor loaded.
    Conflict { current: String },
    Refused(Refusal),
    /// Nothing was left behind; why writing failed.
    Storage(String),
}

#[derive(Debug)]
pub(crate) struct Saved {
    pub layout: Layout,
    pub version: String,
    /// The new file's name (in the configured file's folder).
    pub file: String,
    pub moved: Vec<Moved>,
}

/// `(date, n)` of a name of the form `rack_layout_YYYYMMDD_vN.json`.
pub(super) fn versioned(name: &str) -> Option<(&str, u32)> {
    let (date, n) = name.strip_prefix(PREFIX)?.strip_suffix(".json")?.split_once("_v")?;
    let plain = |digits: &str| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit());
    (date.len() == 8 && plain(date) && plain(n) && !n.starts_with('0')).then(|| n.parse().ok().map(|n| (date, n))).flatten()
}

/// One more than the highest N the folder has for `date`.
fn next_number(dir: &Path, date: &str) -> std::io::Result<u32> {
    let mut highest = 0;
    for entry in fs::read_dir(dir)? {
        let name = entry?.file_name();
        if let Some((_, n)) = name.to_str().and_then(versioned).filter(|(d, _)| *d == date) {
            highest = highest.max(n);
        }
    }
    Ok(highest.saturating_add(1))
}

fn sync_dir(dir: &Path) {
    if let Ok(handle) = fs::File::open(dir) {
        let _ = handle.sync_all();
    }
}

/// `bytes` under a new temporary name beside `target`, synced, then renamed onto it.
fn replace(dir: &Path, target: &Path, bytes: &[u8], mode: u32) -> std::io::Result<()> {
    let temp = dir.join(format!(".{}.tmp-{}", target.file_name().and_then(|n| n.to_str()).unwrap_or("layout"), uuid::Uuid::new_v4().simple()));
    let written = (|| {
        let mut file = OpenOptions::new().write(true).create_new(true).mode(mode).open(&temp)?;
        // Whatever the process's umask is.
        file.set_permissions(fs::Permissions::from_mode(mode))?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&temp, target)
    })();
    if written.is_err() {
        let _ = fs::remove_file(&temp);
    }
    written
}

#[cfg(test)]
thread_local! {
    /// A test's switch: the step that points the configured path at the new file fails.
    pub(super) static FAIL_REPOINT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// A test's hook, run after the folder was listed and before a name is claimed.
    pub(super) static AFTER_LISTING: std::cell::RefCell<Option<Box<dyn Fn()>>> = const { std::cell::RefCell::new(None) };
    /// A test's hook, run after the new file is written and before the configured path is replaced.
    pub(super) static BEFORE_REPOINT: std::cell::RefCell<Option<Box<dyn Fn()>>> = const { std::cell::RefCell::new(None) };
}

/// Point the configured path at the new file: a link by a new link renamed
/// over it, a plain file by a copy renamed over it.
fn repoint(dir: &Path, path: &Path, name: &str, target: &Target, file: &str, text: &[u8]) -> std::io::Result<()> {
    #[cfg(test)]
    if FAIL_REPOINT.with(|f| f.get()) {
        return Err(std::io::Error::other("made to fail"));
    }
    match target {
        Target::Link(_) => {
            let temp = dir.join(format!(".{name}.tmp-{}", uuid::Uuid::new_v4().simple()));
            std::os::unix::fs::symlink(file, &temp).and_then(|()| fs::rename(&temp, path)).inspect_err(|_| {
                let _ = fs::remove_file(&temp);
            })
        }
        Target::File { mode } => replace(dir, path, text, *mode),
    }
}

/// What the configured path is, read once at the start and checked again
/// before it is replaced: a link's text, or a plain file's bytes.
enum Target {
    Link(PathBuf),
    File { mode: u32 },
}

fn version_now(path: &Path) -> String {
    fs::read(path).ok().and_then(|bytes| layout::parse(&bytes).ok()).map_or_else(|| "?".into(), |l| l.version)
}

/// Save `moves` for `by`, as of `now`. The caller holds the lock that
/// keeps two saves of this process apart.
pub(super) fn save(path: &Path, base_version: &str, requested: &[Move], by: &str, now: DateTime<Utc>) -> Result<Saved, Failure> {
    let unreadable = |e: std::io::Error| Failure::Unusable { code: "unreadable", detail: format!("{}: {e}", path.display()) };
    let dir = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return Err(Failure::Unusable { code: "unreadable", detail: format!("{}: not a file name", path.display()) });
    };
    let meta = fs::symlink_metadata(path).map_err(unreadable)?;
    let target = if meta.file_type().is_symlink() {
        Target::Link(fs::read_link(path).map_err(unreadable)?)
    } else if !meta.is_file() {
        return Err(Failure::Unusable { code: "unreadable", detail: format!("{}: not a file", path.display()) });
    } else if versioned(name).is_some() {
        // Replacing it would change a versioned file, which a save never does.
        return Err(Failure::Storage(format!("{name} is itself a versioned layout file; point rack_layout_file at a link or a file of another name to save from the page")));
    } else {
        Target::File { mode: meta.permissions().mode() & 0o777 }
    };
    let before = fs::read(path).map_err(unreadable)?;
    let invalid = |detail: String| Failure::Unusable { code: "invalid", detail };
    let current = layout::parse(&before).map_err(invalid)?;
    let mut doc = Json::parse(&before).map_err(invalid)?;
    if current.version != base_version {
        return Err(Failure::Conflict { current: current.version });
    }
    let moved = moves::plan(&current, requested).map_err(Failure::Refused)?;
    moves::rewrite(&mut doc, &moved, &now.to_rfc3339_opts(SecondsFormat::Secs, true), by).map_err(invalid)?;

    let storage = |what: &str, e: std::io::Error| Failure::Storage(format!("{what} in {}: {e}", dir.display()));
    let date = now.with_timezone(&FixedOffset::east_opt(9 * 3600).unwrap_or_else(|| FixedOffset::east_opt(0).expect("zero offset"))).format("%Y%m%d").to_string();
    let mut n = next_number(dir, &date).map_err(|e| storage("cannot list the layout files", e))?;
    let mut claims = 0;
    #[cfg(test)]
    AFTER_LISTING.with(|hook| hook.borrow().as_ref().map(|run| run()));
    // Claim the name: only one writer can create it, here or elsewhere.
    let (file, version) = loop {
        let file = format!("{PREFIX}{date}_v{n}.json");
        match OpenOptions::new().write(true).create_new(true).mode(0o644).open(dir.join(&file)) {
            Ok(_) => break (file, format!("{date}_v{n}")),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && claims < MAX_CLAIMS => (n, claims) = (n.saturating_add(1), claims + 1),
            Err(e) => return Err(storage("cannot create a new layout file", e)),
        }
    };
    let new = dir.join(&file);
    let undo = |failure: Failure| {
        let _ = fs::remove_file(&new);
        failure
    };
    moves::set_version(&mut doc, &version);
    let text = doc.write();
    // Never leave a file the page could not read.
    let layout = layout::parse(text.as_bytes()).map_err(|e| undo(Failure::Storage(format!("the moved layout would not be valid: {e}"))))?;
    replace(dir, &new, text.as_bytes(), 0o644).map_err(|e| undo(storage("cannot write the new layout file", e)))?;
    sync_dir(dir);

    #[cfg(test)]
    BEFORE_REPOINT.with(|hook| hook.borrow().as_ref().map(|run| run()));
    // Somebody else may have saved meanwhile: then theirs stands.
    let unchanged = match &target {
        Target::Link(to) => fs::read_link(path).is_ok_and(|now| &now == to),
        Target::File { .. } => fs::read(path).is_ok_and(|now| now == before),
    };
    if !unchanged {
        return Err(undo(Failure::Conflict { current: version_now(path) }));
    }
    repoint(dir, path, name, &target, &file, text.as_bytes()).map_err(|e| undo(storage("cannot point the layout at the new file", e)))?;
    sync_dir(dir);
    Ok(Saved { layout, version, file, moved })
}
