//! Build a constrained archive directly into the encryption stream; no plaintext archive on disk.
use std::os::unix::fs::OpenOptionsExt;
use std::{
    collections::HashSet,
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
pub(super) struct Bounded<W> {
    inner: W,
    remaining: u64,
    started: Instant,
}
impl<W: Write> Write for Bounded<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() as u64 > self.remaining || self.started.elapsed() > Duration::from_secs(300)
        {
            return Err(io::Error::other("Archive size or time limit exceeded"));
        }
        let n = self.inner.write(bytes)?;
        self.remaining -= n as u64;
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
struct Walker {
    seen: HashSet<PathBuf>,
    bytes: u64,
    output: PathBuf,
    prefix: PathBuf,
}
impl Walker {
    fn append<W: Write>(
        &mut self,
        archive: &mut tar::Builder<W>,
        source: &Path,
        depth: usize,
    ) -> Result<(), String> {
        if depth > 64 || self.seen.len() >= 100_000 {
            return Err("Archive depth or entry limit exceeded".into());
        }
        let name = source
            .strip_prefix(&self.prefix)
            .map_err(|_| "Invalid source path")?;
        if name.components().count() > 64 {
            return Err("Archive path depth exceeded".into());
        }
        if !self.seen.insert(name.to_owned()) {
            return Err("Overlapping backup source paths".into());
        }
        let meta = fs::symlink_metadata(source).map_err(|_| "Backup source unavailable")?;
        if meta.file_type().is_symlink() || (!meta.is_file() && !meta.is_dir()) {
            return Err("Archive links and special files are not supported".into());
        }
        if source == self.output || self.output.starts_with(source) {
            return Err("Source contains backup storage".into());
        }
        let mut header = tar::Header::new_gnu();
        header.set_uid(0);
        header.set_gid(0);
        header.set_mtime(0);
        if meta.is_dir() {
            header.set_entry_type(tar::EntryType::Directory);
            header.set_mode(0o700);
            header.set_size(0);
            header.set_cksum();
            archive
                .append_data(&mut header, name, io::empty())
                .map_err(|_| "Cannot archive directory")?;
            for child in fs::read_dir(source).map_err(|_| "Cannot read source directory")? {
                self.append(
                    archive,
                    &child.map_err(|_| "Cannot read source directory")?.path(),
                    depth + 1,
                )?;
            }
        } else {
            let mut file = fs::OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
                .open(source)
                .map_err(|_| "Cannot open source file")?;
            let meta = file.metadata().map_err(|_| "Cannot inspect source file")?;
            if !meta.is_file() {
                return Err("Source changed to a non-regular file".into());
            }
            self.bytes = self
                .bytes
                .checked_add(meta.len())
                .ok_or("Source size limit exceeded")?;
            if self.bytes > super::restore::MAX_BYTES {
                return Err("Source size limit exceeded".into());
            }
            header.set_entry_type(tar::EntryType::Regular);
            header.set_mode(0o600);
            header.set_size(meta.len());
            header.set_cksum();
            archive
                .append_data(
                    &mut header,
                    name,
                    Exact {
                        inner: &mut file,
                        remaining: meta.len(),
                    },
                )
                .map_err(|_| "Source changed or could not be archived")?;
            let after = file.metadata().map_err(|_| "Cannot inspect source file")?;
            if after.len() != meta.len() || after.modified().ok() != meta.modified().ok() {
                return Err("Source changed while archiving".into());
            }
        }
        Ok(())
    }
}
pub(super) fn write<W: Write>(
    output: W,
    paths: &[String],
    destination: &Path,
) -> Result<W, String> {
    write_with_prefix(output, paths, destination, Path::new("/"))
}
pub(super) fn write_tree<W: Write>(
    output: W,
    root: &Path,
    destination: &Path,
) -> Result<W, String> {
    let root = fs::canonicalize(root).map_err(|_| "Bundle root unavailable")?;
    let paths = fs::read_dir(&root)
        .map_err(|_| "Bundle root unavailable")?
        .map(|entry| {
            entry
                .map_err(|_| "Cannot read bundle".to_string())
                .and_then(|e| {
                    e.path()
                        .into_os_string()
                        .into_string()
                        .map_err(|_| "Non-UTF8 bundle path".to_string())
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    write_with_prefix(output, &paths, destination, &root)
}
fn write_with_prefix<W: Write>(
    output: W,
    paths: &[String],
    destination: &Path,
    prefix: &Path,
) -> Result<W, String> {
    let gzip = flate2::write::GzEncoder::new(output, flate2::Compression::default());
    let bounded = Bounded {
        inner: gzip,
        remaining: super::restore::MAX_BYTES,
        started: Instant::now(),
    };
    let mut archive = tar::Builder::new(bounded);
    let mut walker = Walker {
        prefix: prefix.to_path_buf(),
        seen: HashSet::new(),
        bytes: 0,
        output: fs::canonicalize(destination).map_err(|_| "Backup storage unavailable")?,
    };
    for path in paths {
        if fs::symlink_metadata(path)
            .map_err(|_| "Backup source unavailable")?
            .file_type()
            .is_symlink()
        {
            return Err("Source symlinks are not supported".into());
        }
        let canonical = fs::canonicalize(path).map_err(|_| "Backup source unavailable")?;
        walker.append(&mut archive, &canonical, 0)?;
    }
    archive
        .into_inner()
        .map_err(|_| "Cannot finish archive")?
        .inner
        .finish()
        .map_err(|_| "Cannot finish archive compression".into())
}

// tar::Builder trusts the caller's declared size; reject both early EOF and growth.
struct Exact<R> {
    inner: R,
    remaining: u64,
}
impl<R: Read> Read for Exact<R> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        if self.remaining == 0 {
            let mut extra = [0u8];
            if self.inner.read(&mut extra)? != 0 {
                return Err(io::Error::other("Source grew"));
            }
            return Ok(0);
        }
        let length = output.len().min(self.remaining as usize);
        let count = self.inner.read(&mut output[..length])?;
        if count == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Source shrank",
            ));
        }
        self.remaining -= count as u64;
        Ok(count)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_source_size_changes() {
        for (data, expected, valid) in [
            (b"abc".as_slice(), 3, true),
            (b"ab".as_slice(), 3, false),
            (b"abcd".as_slice(), 3, false),
        ] {
            let result = Exact {
                inner: data,
                remaining: expected,
            }
            .read_to_end(&mut Vec::new());
            assert_eq!(result.is_ok(), valid);
        }
    }
}
