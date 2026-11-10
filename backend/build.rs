//! Records the git commit the server is built from, for `/api/diagnostics/runtime`.
//!
//! `NABIMAN_GIT_COMMIT` is the 12-character commit id, with `-dirty` when
//! tracked files differ from it, or `unknown` when nothing says which commit
//! this is.
//!
//! Where the id comes from, first match wins:
//! 1. The `NABIMAN_GIT_COMMIT` environment variable at build time (7-40 hex
//!    digits, optionally `-dirty`). Builds without a `.git` set it: the
//!    Dockerfile (`--build-arg NABIMAN_GIT_COMMIT=$(git rev-parse HEAD)`),
//!    the release workflow (the tag's commit) and source tarballs.
//! 2. `git` in the repository.
//! 3. `unknown`.
//!
//! The script runs again only when what it read can change: the variable,
//! the checked-out commit (HEAD, the branch it points to, packed refs) and the
//! backend's own sources, which rebuild the server anyway. Not the index,
//! which `git status` rewrites in passing: that would rebuild for nothing. So
//! an edit outside `backend/` shows as `-dirty` only from the next build that
//! has to run this script again.
//!
//! A build that found no git must not keep `unknown` once git can answer: it
//! then also watches the repository's `.git` (HEAD and the branch file, read
//! without git) and `PATH`, so installing git, checking out, committing or
//! cloning into place runs the script again.

use std::path::{Path, PathBuf};
use std::process::Command;

const ENV: &str = "NABIMAN_GIT_COMMIT";

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).current_dir(env!("CARGO_MANIFEST_DIR")).output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8(out.stdout).ok()?.trim().to_string())
}

fn watch(path: &Path) {
    println!("cargo:rerun-if-changed={}", path.display());
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
        watch(watched);
    }
}

/// The commit given by the environment, cut to 12 digits; a malformed value
/// stops the build rather than being recorded.
fn from_env() -> Option<String> {
    let raw = std::env::var(ENV).ok()?;
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    let (hex, dirty) = match raw.strip_suffix("-dirty") {
        Some(hex) => (hex, "-dirty"),
        None => (raw, ""),
    };
    if !(7..=40).contains(&hex.len()) || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        panic!("{ENV} must be a git commit id (7-40 hex digits, optional -dirty), got {raw:?}");
    }
    let short: String = hex.to_ascii_lowercase().chars().take(12).collect();
    Some(format!("{short}{dirty}"))
}

/// The nearest `.git` (a folder, or the file a worktree or submodule has).
fn find_dot_git() -> Option<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().map(|dir| dir.join(".git")).find(|p| p.exists())
}

/// Without git: watches HEAD and the branch it names, read by hand, so a
/// checkout or a commit runs the script again. A `.git` folder itself is not
/// watched: cargo would scan every object in it on each build.
fn watch_without_git(dot_git: &Path) {
    let git_dir = if dot_git.is_file() {
        watch(dot_git);
        // A worktree: `gitdir: <path>`, and the shared refs in `commondir`.
        let Some(dir) = std::fs::read_to_string(dot_git)
            .ok()
            .and_then(|s| s.strip_prefix("gitdir:").map(|d| d.trim().to_string()))
        else {
            return;
        };
        dot_git.parent().unwrap_or(Path::new("/")).join(dir)
    } else {
        dot_git.to_path_buf()
    };
    let head = git_dir.join("HEAD");
    if !head.exists() {
        return;
    }
    watch(&head);
    let common = std::fs::read_to_string(git_dir.join("commondir"))
        .map(|c| git_dir.join(c.trim()))
        .unwrap_or_else(|_| git_dir.clone());
    let packed = common.join("packed-refs");
    if packed.exists() {
        watch(&packed);
    }
    if let Some(branch) = std::fs::read_to_string(&head).ok().and_then(|h| h.strip_prefix("ref:").map(|r| r.trim().to_string())) {
        let file = common.join(&branch);
        if file.exists() {
            watch(&file);
        } else if let Some(folder) = file.parent().filter(|p| p.exists()) {
            watch(folder);
        }
    }
}

fn from_git() -> Option<String> {
    let commit = git(&["rev-parse", "--short=12", "HEAD"])
        .filter(|c| c.len() == 12 && c.bytes().all(|b| b.is_ascii_hexdigit()))?;
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
    Some(if dirty { format!("{commit}-dirty") } else { commit })
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-env-changed={ENV}");
    let id = from_env().or_else(from_git).unwrap_or_else(|| {
        // Git may be installed, or the checkout appear, later: look again then.
        println!("cargo:rerun-if-env-changed=PATH");
        if let Some(dot_git) = find_dot_git() {
            watch_without_git(&dot_git);
        }
        "unknown".to_string()
    });
    println!("cargo:rustc-env={ENV}={id}");
}
