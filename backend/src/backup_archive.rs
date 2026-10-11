//! Bounded archive worker. An archive is published only after tar succeeds and fsync.
use std::os::unix::{fs::OpenOptionsExt, process::CommandExt};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

pub(super) use nabiman_server::backup_crypto::validate;
struct Stage(PathBuf);
impl Drop for Stage {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub(super) fn create(
    dir: &Path,
    name: &str,
    paths: &[String],
    compressed: bool,
) -> Result<String, String> {
    validate(name, paths)?;
    fs::create_dir_all(dir).map_err(|_| "Backup storage unavailable")?;
    let destination = fs::canonicalize(dir).map_err(|_| "Backup storage unavailable")?;
    let mut sources = Vec::new();
    for path in paths {
        let source = fs::canonicalize(path).map_err(|_| "Backup source unavailable")?;
        let meta = fs::metadata(&source).map_err(|_| "Backup source unavailable")?;
        if (!meta.is_file() && !meta.is_dir()) || destination.starts_with(&source) {
            return Err("Unsupported source or source contains backup storage".into());
        }
        sources.push(source);
    }
    let id = uuid::Uuid::new_v4();
    let stage = Stage(destination.join(format!(".pending-{id}")));
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&stage.0)
        .map_err(|_| "Cannot stage backup")?;
    let output = stage.0.join("archive");
    let file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&output)
        .map_err(|_| "Cannot stage backup")?;
    let mut command = Command::new("tar");
    command
        .args([if compressed { "-czf" } else { "-cf" }, "-"])
        .arg("--")
        .args(sources)
        .env_remove("TAR_OPTIONS")
        .env_remove("GZIP")
        .stdin(Stdio::null())
        .stdout(file)
        .stderr(Stdio::null())
        .process_group(0);
    let mut child = command.spawn().map_err(|_| "Archive worker unavailable")?;
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None)
                if started.elapsed() < Duration::from_secs(300)
                    && fs::metadata(&output).is_ok_and(|m| m.len() <= 1024 * 1024 * 1024) =>
            {
                std::thread::sleep(Duration::from_millis(100))
            }
            _ => {
                // Only the new worker's process group, never the API or a user's shell.
                unsafe {
                    libc::kill(-(child.id() as i32), libc::SIGKILL);
                }
                let _ = child.wait();
                return Err(
                    "Archive worker exceeded time/size limit or could not be monitored".into(),
                );
            }
        }
    };
    if fs::metadata(&output)
        .map_err(|_| "Archive output unavailable")?
        .len()
        > 1024 * 1024 * 1024
    {
        return Err("Archive exceeded 1 GiB limit".into());
    }
    if !status.success() {
        return Err("Archive failed; no completed backup was published".into());
    }
    fs::File::open(&output)
        .and_then(|f| f.sync_all())
        .map_err(|_| "Cannot persist archive")?;
    let filename = format!("backup-{id}.{}", if compressed { "tar.gz" } else { "tar" });
    // hard_link never overwrites another artifact, even on an unlikely identifier collision.
    fs::hard_link(&output, destination.join(&filename)).map_err(|_| "Cannot publish archive")?;
    fs::File::open(&destination)
        .and_then(|f| f.sync_all())
        .map_err(|_| "Archive published but durability could not be confirmed")?;
    Ok(filename)
}
use std::os::unix::fs::DirBuilderExt;
