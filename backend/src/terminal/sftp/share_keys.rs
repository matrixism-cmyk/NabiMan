//! File access for someone who opened a share link.
//!
//! A visitor has no account and no JWT. When a link grants files, the same
//! admission that hands out a terminal ticket (password, rate limits and all)
//! also hands out a file key: random, kept here only as a digest, tied to
//! that link exactly as it was authorized. Every file request re-checks the
//! live link against that snapshot, so revoking or changing the link ends
//! file access at once, and a key never outlives its link or one day.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use rand::{rngs::OsRng, RngCore};
use sha2::{Digest, Sha256};

use super::super::share::{ShareFiles, ShareLink};
use super::super::session::now_unix;

const MAX_TTL: Duration = Duration::from_secs(24 * 3600);
const CAPACITY: usize = 4096;
/// Keys a single link may hold at once; the oldest goes first.
const PER_LINK: usize = 16;

struct Key {
    link: ShareLink,
    issued: Instant,
    expires: Instant,
}

fn store() -> &'static Mutex<HashMap<String, Key>> {
    static STORE: OnceLock<Mutex<HashMap<String, Key>>> = OnceLock::new();
    STORE.get_or_init(Default::default)
}

fn digest(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}

/// A key for `link`, or `None` when the link grants no files.
pub(crate) fn issue(link: &ShareLink) -> Option<String> {
    if link.files == ShareFiles::None {
        return None;
    }
    let left = Duration::from_secs(link.expires_unix.saturating_sub(now_unix()));
    let ttl = left.min(MAX_TTL);
    if ttl.is_zero() {
        return None;
    }
    let mut bytes = [0u8; 32];
    OsRng.try_fill_bytes(&mut bytes).ok()?;
    let key = hex::encode(bytes);
    let now = Instant::now();
    let mut map = store().lock().unwrap_or_else(|e| e.into_inner());
    map.retain(|_, k| k.expires > now);
    let mine: Vec<(String, Instant)> = map
        .iter()
        .filter(|(_, k)| k.link.token == link.token)
        .map(|(d, k)| (d.clone(), k.issued))
        .collect();
    if mine.len() >= PER_LINK {
        if let Some((oldest, _)) = mine.iter().min_by_key(|(_, at)| *at) {
            map.remove(oldest);
        }
    }
    if map.len() >= CAPACITY {
        return None;
    }
    map.insert(digest(&key), Key { link: link.clone(), issued: now, expires: now + ttl });
    Some(key)
}

/// The link snapshot `key` was issued for, if it belongs to `token` and has
/// not expired.
pub(crate) fn check(key: &str, token: &str) -> Option<ShareLink> {
    if key.len() != 64 {
        return None;
    }
    let map = store().lock().unwrap_or_else(|e| e.into_inner());
    map.get(&digest(key))
        .filter(|k| k.expires > Instant::now() && k.link.token == token)
        .map(|k| k.link.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn link(token: &str, files: ShareFiles) -> ShareLink {
        serde_json::from_value(serde_json::json!({
            "token": token, "session_id": "pty_1", "owner": "admin", "owner_id": "u1",
            "owner_authorization_version": "v1", "label": "local",
            "created_unix": now_unix(), "expires_unix": now_unix() + 3600,
            "files": files,
        }))
        .unwrap()
    }

    #[test]
    fn keys_belong_to_one_link() {
        let a = link(&"a".repeat(64), ShareFiles::Read);
        let key = issue(&a).unwrap();
        assert!(check(&key, &a.token).is_some_and(|l| l.files == ShareFiles::Read));
        assert!(check(&key, &"b".repeat(64)).is_none());
        assert!(check(&"0".repeat(64), &a.token).is_none());
    }

    #[test]
    fn links_without_files_get_no_key() {
        assert!(issue(&link(&"c".repeat(64), ShareFiles::None)).is_none());
    }

    #[test]
    fn a_link_holds_a_bounded_number_of_keys() {
        let l = link(&"d".repeat(64), ShareFiles::Write);
        let first = issue(&l).unwrap();
        for _ in 0..PER_LINK {
            issue(&l).unwrap();
        }
        assert!(check(&first, &l.token).is_none(), "the oldest key gives way");
    }
}
