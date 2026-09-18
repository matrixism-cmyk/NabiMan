//! Proof that a visitor typed the share link's password.
//!
//! The ticket is `<expiry>.<digest>`, signed with the server secret, so the
//! server keeps no state between the password check and the WebSocket that
//! follows — and a restart does not lock a visitor out.

use sha2::{Digest, Sha256};

use super::session::now_unix;

pub(crate) fn sign(secret: &str, token: &str, expires: u64) -> String {
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    hasher.update(b"share-ticket");
    hasher.update(token.as_bytes());
    hasher.update(expires.to_string().as_bytes());
    // Closing with the secret as well keeps the digest from being extended.
    hasher.update(secret.as_bytes());
    format!("{}.{}", expires, hex::encode(hasher.finalize()))
}

pub(crate) fn valid(secret: &str, token: &str, ticket: &str) -> bool {
    let expires: u64 = match ticket.split_once('.').map(|(exp, _)| exp.parse()) {
        Some(Ok(v)) => v,
        _ => return false,
    };
    now_unix() <= expires && sign(secret, token, expires) == ticket
}
