//! The three ways `usage` counts a tree: this machine's folders read directly,
//! a server's own GNU `find` over its SSH shell, or folders listed one by one
//! over the file connection (an SFTP-only account, FTP), several at a time.

use std::io::{BufRead, BufReader};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::stream::{FuturesUnordered, StreamExt};

use super::fs::Fs;
use super::paths::join;
use super::usage::{Count, Job, SYSTEM};
use crate::models::RemoteServer;

/// Folders listed at once over a file connection.
const PARALLEL: usize = 8;
/// Entries counted between looks at the limits.
const BATCH: u64 = 4096;
const NO_FIND: &str = "__NABIMAN_NO_FIND__";
const DONE: &str = "__NABIMAN_DONE__";

fn system(path: &str) -> bool {
    SYSTEM.contains(&path)
}

/// This machine: `std::fs`, on a blocking thread.
pub(super) fn local(roots: &[String], job: &Job) {
    let mut folders: Vec<std::path::PathBuf> = roots.iter().map(Into::into).collect();
    while let Some(dir) = folders.pop() {
        if job.over() {
            return;
        }
        let Ok(list) = std::fs::read_dir(&dir) else {
            job.tally(|c| c.unreadable += 1);
            continue;
        };
        let mut here = Count::default();
        for item in list.flatten() {
            // A DirEntry's metadata does not follow links.
            let Ok(meta) = item.metadata() else { continue };
            let t = meta.file_type();
            let kind = if t.is_symlink() { "link" } else if t.is_dir() { "dir" } else if t.is_file() { "file" } else { "other" };
            here.add(kind, meta.len());
            if kind == "dir" {
                let path = item.path();
                if !system(&path.to_string_lossy()) {
                    folders.push(path);
                }
            }
            if here.entries() >= BATCH {
                job.tally(|c| c.merge(&here));
                here = Count::default();
                if job.over() {
                    return;
                }
            }
        }
        job.tally(|c| c.merge(&here));
    }
}

/// The command a server counts with: `%y %s` per entry, `find:` lines for
/// what it could not read, and a last line to tell a finished count from a
/// dropped connection. Without GNU find (BusyBox has no -printf) it says so.
fn find_command(roots: &[String]) -> String {
    let starts: Vec<String> = roots.iter().map(|r| crate::terminal::ssh_command::quote(r)).collect();
    let prune: Vec<String> = SYSTEM.iter().map(|p| format!("-path {p}")).collect();
    format!(
        "find / -maxdepth 0 -printf '' >/dev/null 2>&1 || {{ echo {NO_FIND}; exit 0; }}; \
         LC_ALL=C find {} -mindepth 1 \\( {} \\) -prune -o -printf '%y %s\\n' 2>&1; echo {DONE}",
        starts.join(" "),
        prune.join(" -o "),
    )
}

/// What one line of the count says.
fn read_line(line: &str, c: &mut Count) {
    if line.starts_with("find:") {
        c.unreadable += 1;
        return;
    }
    let Some((kind, size)) = line.split_once(' ') else { return };
    let size = size.trim().parse().unwrap_or(0);
    let kind = match kind {
        "d" => "dir",
        "f" => "file",
        "l" => "link",
        _ => "other",
    };
    c.add(kind, size);
}

/// A server with an SSH shell, by its own `find`. False when it could not
/// count that way (no GNU find, no shell), so the caller walks instead.
pub(super) fn find(server: &RemoteServer, roots: &[String], job: &Job) -> bool {
    let Ok((mut cmd, _password)) = crate::remote_probe::ssh_command(server, 8, &find_command(roots)) else {
        return false;
    };
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());
    let Ok(mut child) = cmd.spawn() else { return false };
    let Some(out) = child.stdout.take() else { return false };
    let child = Arc::new(Mutex::new(child));
    let finished = Arc::new(AtomicBool::new(false));
    // Reading blocks while the server is quiet, so the limits are watched apart.
    std::thread::scope(|s| {
        s.spawn(|| {
            while !finished.load(Ordering::Relaxed) {
                if job.over() {
                    let _ = child.lock().unwrap_or_else(|e| e.into_inner()).kill();
                    return;
                }
                std::thread::sleep(Duration::from_millis(200));
            }
        });
        let (mut done, mut no_find, mut lines) = (false, false, 0u64);
        let mut here = Count::default();
        let mut reader = BufReader::new(out);
        let mut buf = Vec::new();
        while matches!(reader.read_until(b'\n', &mut buf), Ok(n) if n > 0) {
            let line = String::from_utf8_lossy(&buf);
            let line = line.trim_end();
            match line {
                DONE => done = true,
                NO_FIND => no_find = true,
                _ => read_line(line, &mut here),
            }
            buf.clear();
            lines += 1;
            if lines % BATCH == 0 {
                job.tally(|c| c.merge(&here));
                here = Count::default();
            }
        }
        job.tally(|c| c.merge(&here));
        finished.store(true, Ordering::Relaxed);
        let _ = child.lock().unwrap_or_else(|e| e.into_inner()).wait();
        !no_find && (done || job.over())
    })
}

/// Over the file connection: folders listed a few at a time.
pub(super) async fn walk(fs: &Fs, roots: Vec<String>, job: &Job) {
    let mut folders = roots;
    let mut running = FuturesUnordered::new();
    loop {
        while running.len() < PARALLEL {
            let Some(dir) = folders.pop() else { break };
            let fs = fs.clone();
            running.push(async move {
                let listed = fs.read_dir(&dir).await;
                (dir, listed)
            });
        }
        let Some((dir, listed)) = running.next().await else { return };
        match listed {
            Ok(list) => {
                let mut here = Count::default();
                for (name, meta) in list {
                    here.add(meta.kind, meta.size);
                    if meta.is_dir() {
                        let path = join(&dir, &name);
                        if !system(&path) {
                            folders.push(path);
                        }
                    }
                }
                job.tally(|c| c.merge(&here));
            }
            Err(e) if e.broken => {
                job.give_up("lost");
                return;
            }
            Err(_) => job.tally(|c| c.unreadable += 1),
        }
        if job.over() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_lines_count_by_kind_and_errors_are_unreadable_folders() {
        let mut c = Count::default();
        for l in ["d 4096", "f 120", "f 30", "l 9", "s 0", "find: '/x/secret': Permission denied", "", "garbage"] {
            read_line(l, &mut c);
        }
        assert_eq!(c, Count { dirs: 1, files: 2, links: 1, other: 1, bytes: 150, unreadable: 1 });
    }

    #[test]
    fn the_find_command_quotes_its_starts_and_skips_system_folders() {
        let cmd = find_command(&["/home/me/it's here".to_string(), "/srv".to_string()]);
        assert!(cmd.contains(r"find '/home/me/it'\''s here' '/srv' -mindepth 1"), "{cmd}");
        assert!(cmd.contains(r"\( -path /proc -o -path /sys -o -path /dev \) -prune"), "{cmd}");
        assert!(cmd.ends_with(DONE));
    }

    #[test]
    fn this_machine_is_counted_without_following_links() {
        let dir = std::env::temp_dir().join(format!("nabiman-usage-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("a/b")).unwrap();
        std::fs::write(dir.join("a/one"), b"12345").unwrap();
        std::fs::write(dir.join("a/b/two"), b"123").unwrap();
        std::os::unix::fs::symlink("/", dir.join("a/root")).unwrap();
        let job = Job::for_test();
        local(&[dir.to_string_lossy().into_owned()], &job);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(job.snapshot(), Count { dirs: 2, files: 2, links: 1, other: 0, bytes: 8, unreadable: 0 });
    }
}
