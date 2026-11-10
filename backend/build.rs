//! Records the git commit the server is built from, for `/api/diagnostics/runtime`.
//!
//! `NABIMAN_GIT_COMMIT` is the 12-character commit id, with `-dirty` when
//! tracked files differ from it, or `unknown` when there is no git to ask (a
//! build from a release tarball, or a container that copied in no `.git`).
//!
//! The script runs again only when what it read can change: the checked-out
//! commit (HEAD, the branch it points to, packed refs) and the backend's own
//! sources, which rebuild the server anyway. Not the index, which `git
//! status` rewrites in passing: that would rebuild for nothing. So an edit
//! outside `backend/` shows as `-dirty` only from the next build that has to
//! run this script again.

use std::path::Path;
use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).current_dir(env!("CARGO_MANIFEST_DIR")).output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8(out.stdout).ok()?.trim().to_string())
}

/// Watches a file under the git directory. A branch kept only in packed-refs
/// has no file of its own yet: then the folder its file would be written into.
fn watch_git_path(name: &str, or_its_folder: bool) {
    // `--git-path` finds the file wherever it lives, worktrees included.
    let Some(path) = git(&["rev-parse", "--git-path", name]) else { return };
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(path);
    let watched = if path.exists() {
        Some(path.as_path())
    } else {
        path.parent().filter(|p| or_its_folder && p.exists())
    };
    if let Some(watched) = watched {
        println!("cargo:rerun-if-changed={}", watched.display());
    }
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src");
    let commit = git(&["rev-parse", "--short=12", "HEAD"])
        .filter(|c| c.len() == 12 && c.bytes().all(|b| b.is_ascii_hexdigit()));
    let id = match commit {
        Some(commit) => {
            watch_git_path("HEAD", false);
            watch_git_path("packed-refs", false);
            if let Some(branch) = git(&["symbolic-ref", "-q", "HEAD"]) {
                watch_git_path(&branch, true);
            }
            // `git diff` compares without writing the index, unlike `git status`.
            let dirty = Command::new("git")
                .args(["diff", "--quiet", "HEAD", "--"])
                .current_dir(env!("CARGO_MANIFEST_DIR"))
                .status()
                .is_ok_and(|s| s.code() == Some(1));
            if dirty { format!("{commit}-dirty") } else { commit }
        }
        None => "unknown".to_string(),
    };
    println!("cargo:rustc-env=NABIMAN_GIT_COMMIT={id}");
}
