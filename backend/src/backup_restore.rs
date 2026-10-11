//! Extract authenticated archives into a private staging tree, then publish without overwrite.
use std::os::unix::{
    ffi::OsStrExt,
    fs::{DirBuilderExt, OpenOptionsExt},
};
use std::{
    fs,
    io::{self, Read},
    path::{Component, Path, PathBuf},
};
pub(super) const MAX_BYTES: u64 = 1024 * 1024 * 1024;
pub(super) struct Stage(pub PathBuf);
impl Stage {
    pub fn new(parent: &Path) -> Result<Self, String> {
        let p = parent.join(format!(".private-backup-{}", uuid::Uuid::new_v4()));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&p)
            .map_err(|_| "Cannot create private backup staging")?;
        Ok(Self(p))
    }
}
impl Drop for Stage {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
pub(super) fn private_file(path: &Path) -> io::Result<fs::File> {
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
}
pub(super) fn directory(path: &Path) -> io::Result<()> {
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path)
}
/// No archive metadata is applied: restored files 0600, directories 0700, API UID owns them.
pub(super) fn extract(input: impl Read, target: &Path) -> Result<(), String> {
    let decoded = flate2::read::GzDecoder::new(input).take(MAX_BYTES + 1);
    let mut archive = tar::Archive::new(decoded);
    let mut total = 0u64;
    let mut count = 0usize;
    for entry in archive.entries().map_err(|_| "Invalid archive")? {
        let mut entry = entry.map_err(|_| "Invalid archive entry")?;
        count += 1;
        if count > 100_000 {
            return Err("Archive has too many entries".into());
        }
        let path = entry
            .path()
            .map_err(|_| "Invalid archive path")?
            .into_owned();
        if path.components().count() > 64
            || path.as_os_str().is_empty()
            || path
                .components()
                .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
        {
            return Err("Unsafe archive path".into());
        }
        let kind = entry.header().entry_type();
        if !kind.is_file() && !kind.is_dir() {
            return Err("Archive links and special files are not supported".into());
        }
        let dest = target.join(&path);
        if kind.is_dir() {
            directory(&dest).map_err(|_| "Cannot restore directory")?;
            continue;
        }
        total = total
            .checked_add(entry.size())
            .ok_or("Archive is too large")?;
        if total > MAX_BYTES {
            return Err("Expanded archive exceeds 1 GiB".into());
        }
        directory(dest.parent().ok_or("Invalid archive path")?)
            .map_err(|_| "Cannot restore parent directory")?;
        let mut output = private_file(&dest).map_err(|_| "Duplicate or invalid archive file")?;
        let bytes = io::copy(&mut entry, &mut output).map_err(|_| "Cannot restore file")?;
        if bytes != entry.size() {
            return Err("Truncated archive file".into());
        }
        output
            .sync_all()
            .map_err(|_| "Cannot persist restored file")?;
    }
    // Drain through gzip EOF to verify its trailer and enforce total decompression bound,
    // including metadata and trailing padding rather than only extracted file sizes.
    let mut decoded = archive.into_inner();
    io::copy(&mut decoded, &mut io::sink()).map_err(|_| "Invalid compressed archive trailer")?;
    if decoded.limit() == 0 {
        return Err("Expanded archive exceeds 1 GiB".into());
    }
    sync_tree(target)?;
    Ok(())
}
fn sync_tree(path: &Path) -> Result<(), String> {
    for entry in fs::read_dir(path).map_err(|_| "Cannot inspect restored tree")? {
        let entry = entry.map_err(|_| "Cannot inspect restored tree")?;
        if entry
            .file_type()
            .map_err(|_| "Cannot inspect restored tree")?
            .is_dir()
        {
            sync_tree(&entry.path())?;
        }
    }
    fs::File::open(path)
        .and_then(|f| f.sync_all())
        .map_err(|_| "Cannot persist restored directory".into())
}
pub(super) fn target(path: &str) -> Result<PathBuf, String> {
    let target = Path::new(path);
    if !target.is_absolute()
        || target
            .components()
            .any(|c| !matches!(c, Component::RootDir | Component::Normal(_)))
    {
        return Err("Use an absolute new directory path without parent traversal".into());
    }
    let name = target
        .file_name()
        .ok_or("Choose a new directory below an existing parent")?;
    let parent = fs::canonicalize(target.parent().ok_or("Missing restore parent")?)
        .map_err(|_| "Restore parent unavailable")?;
    let target = parent.join(name);
    if fs::symlink_metadata(&target).is_ok() {
        return Err("Restore target already exists; choose a new directory".into());
    }
    Ok(target)
}
pub(super) fn publish(staged: &Path, target: &Path) -> Result<(), String> {
    let from = std::ffi::CString::new(staged.as_os_str().as_bytes())
        .map_err(|_| "Invalid restore path")?;
    let to = std::ffi::CString::new(target.as_os_str().as_bytes())
        .map_err(|_| "Invalid restore path")?;
    // Linux renameat2 NOREPLACE is atomic even if another actor creates target after validation.
    let result = unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            from.as_ptr(),
            libc::AT_FDCWD,
            to.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if result != 0 {
        return Err(
            "Cannot publish restored directory; existing target was not overwritten".into(),
        );
    }
    fs::File::open(target.parent().ok_or("Missing parent")?)
        .and_then(|f| f.sync_all())
        .map_err(|_| "Restore published but durability could not be confirmed".into())
}
