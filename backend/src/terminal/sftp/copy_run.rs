//! The copy itself: everything to copy is found first (so the total is
//! known), the folders are made, then each file is read from one place and
//! written to the other as it arrives. Nothing is kept here and nothing goes
//! through the browser.
//!
//! A file is written under a hidden part name and takes its real name only
//! once all of it is there, so a copy that fails or is cancelled never leaves
//! a short file behind; one already there is replaced only at that moment.

use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::copy::Job;
use super::fs::{Fs, FsError};
use super::paths::{base_name, join, parent};

/// A copy gives up rather than walk an enormous tree.
const MAX_ENTRIES: usize = 20_000;
const CHUNK: usize = 256 * 1024;
/// Notes kept about what was left out; the count goes on past these.
const MAX_NOTES: usize = 50;

struct Item {
    from: String,
    to: String,
    size: u64,
    mode: Option<u32>,
    /// The path below the copied item, for messages.
    label: String,
}

#[derive(Default)]
struct Plan {
    /// Folders to make, parents first, with the mode to give them at the end.
    dirs: Vec<(String, Option<u32>)>,
    files: Vec<Item>,
}

/// Everything under `paths`, to land in `dir`. Links inside folders are not
/// followed (as a folder download does not follow them); a selected link is.
async fn plan(job: &Job, src: &Fs, dst: &Fs, paths: &[String], dir: &str) -> Result<Plan, String> {
    let mut plan = Plan::default();
    let mut entries = 0usize;
    for path in paths {
        let name = base_name(path).to_string();
        let to = join(dir, &name);
        let meta = match src.stat(path).await {
            Ok(m) => m,
            Err(e) => {
                job.note(format!("{name}  ({})", e.message));
                continue;
            }
        };
        let there = dst.lstat(&to).await.ok();
        match meta.kind {
            "file" => {
                if there.as_ref().is_some_and(|m| m.is_dir()) {
                    job.note(format!("{name}  (a folder of that name is already there)"));
                    continue;
                }
                plan.files.push(Item { from: path.clone(), to, size: meta.size, mode: meta.mode, label: name });
            }
            "dir" => {
                if there.as_ref().is_some_and(|m| !m.is_dir()) {
                    job.note(format!("{name}/  (a file of that name is already there)"));
                    continue;
                }
                plan.dirs.push((to.clone(), meta.mode));
                let mut stack = vec![(path.clone(), to, name)];
                while let Some((from_dir, to_dir, rel)) = stack.pop() {
                    if job.cancelled() {
                        return Ok(plan);
                    }
                    let items = match src.read_dir(&from_dir).await {
                        Ok(items) => items,
                        Err(e) => {
                            job.note(format!("{rel}/  ({})", e.message));
                            continue;
                        }
                    };
                    for (child, meta) in items {
                        entries += 1;
                        if entries > MAX_ENTRIES {
                            return Err(format!("More than {MAX_ENTRIES} files and folders; copy this from the terminal"));
                        }
                        let (from, to, label) = (join(&from_dir, &child), join(&to_dir, &child), format!("{rel}/{child}"));
                        match meta.kind {
                            "dir" => {
                                plan.dirs.push((to.clone(), meta.mode));
                                stack.push((from, to, label));
                            }
                            "file" => plan.files.push(Item { from, to, size: meta.size, mode: meta.mode, label }),
                            "link" => job.note(format!("{label}  (link, not followed)")),
                            _ => job.note(format!("{label}  (special file)")),
                        }
                    }
                }
            }
            _ => job.note(format!("{name}  (special file)")),
        }
    }
    Ok(plan)
}

/// How one file's copy ended, short of an error.
enum Copied {
    Done,
    Cancelled,
}

/// Errors before any data moves (cannot open, cannot create) leave this file
/// out; an error while data moves stops the whole copy, since the connection
/// or the disk is what failed.
enum Failure {
    File(FsError),
    Copy(String),
}

async fn copy_file(job: &Job, src: &Fs, dst: &Fs, item: &Item) -> Result<Copied, Failure> {
    let dir = parent(&item.to).unwrap_or_else(|| "/".into());
    let part = super::parts::part_name(base_name(&item.to), &job.id)
        .map(|p| join(&dir, &p))
        .ok_or_else(|| Failure::File(FsError::from(std::io::Error::other("Invalid file name"))))?;
    let mut reader = src.open(&item.from).await.map_err(Failure::File)?;
    let mut writer = dst.create(&part).await.map_err(Failure::File)?;
    let moved: Result<Copied, String> = async {
        let mut buf = vec![0u8; CHUNK];
        loop {
            if job.cancelled() {
                return Ok(Copied::Cancelled);
            }
            let n = reader.read(&mut buf).await.map_err(|e| format!("Reading {} failed: {e}", item.label))?;
            if n == 0 {
                break;
            }
            writer.write_all(&buf[..n]).await.map_err(|e| format!("Writing {} failed: {e}", item.label))?;
            job.moved(n as u64);
        }
        writer.shutdown().await.map_err(|e| format!("Writing {} failed: {e}", item.label))?;
        Ok(Copied::Done)
    }
    .await;
    drop(writer);
    drop(reader);
    match moved {
        Ok(Copied::Done) => {}
        other => {
            let _ = dst.remove_file(&part).await;
            return other.map_err(Failure::Copy);
        }
    }
    // Only now is a file already there replaced (the operator said so).
    let placed = async {
        if dst.lstat(&item.to).await.is_ok() {
            dst.remove_file(&item.to).await?;
        }
        dst.rename(&part, &item.to).await
    }
    .await;
    if let Err(e) = placed {
        let _ = dst.remove_file(&part).await;
        return Err(Failure::File(e));
    }
    if let Some(mode) = item.mode {
        // Best effort: FTP servers may not take SITE CHMOD.
        let _ = dst.chmod(&item.to, mode).await;
    }
    Ok(Copied::Done)
}

pub(super) async fn run(job: Arc<Job>, src: Fs, dst: Fs, paths: Vec<String>, dir: String) {
    let plan = match plan(&job, &src, &dst, &paths, &dir).await {
        Ok(plan) => plan,
        Err(message) => return job.finish("failed", message),
    };
    job.planned(plan.files.iter().map(|f| f.size).sum(), plan.files.len());
    for (path, _) in &plan.dirs {
        if job.cancelled() {
            return job.finish("cancelled", String::new());
        }
        if !dst.lstat(path).await.is_ok_and(|m| m.is_dir()) {
            if let Err(e) = dst.mkdir(path).await {
                if e.broken {
                    return job.finish("failed", e.message);
                }
                job.note(format!("{path}/  ({})", e.message));
            }
        }
    }
    for item in &plan.files {
        job.working_on(&item.label);
        match copy_file(&job, &src, &dst, item).await {
            Ok(Copied::Done) => job.file_done(),
            Ok(Copied::Cancelled) => return job.finish("cancelled", String::new()),
            Err(Failure::File(e)) if e.broken => return job.finish("failed", e.message),
            Err(Failure::File(e)) => job.note(format!("{}  ({})", item.label, e.message)),
            Err(Failure::Copy(message)) => return job.finish("failed", message),
        }
    }
    // Folder modes last: a read-only folder would have refused its files.
    for (path, mode) in plan.dirs.iter().rev() {
        if let Some(mode) = mode {
            let _ = dst.chmod(path, *mode).await;
        }
    }
    job.finish("done", String::new());
}

pub(super) const fn max_notes() -> usize {
    MAX_NOTES
}

#[cfg(test)]
#[path = "copy_tests.rs"]
mod tests;
