//! The passkeys kept with each account in the user database.

use sha2::{Digest, Sha256};
use webauthn_rs::prelude::{AuthenticationResult, Passkey, Uuid};

use crate::lock_safe::LockExt;
use crate::models::{NabimanUser, StoredPasskey};
use crate::users::UserStore;

/// How a passkey is named in lists, requests and the audit log: a digest of
/// its credential id (which can be very long and is the browser's to keep).
pub(super) fn key(credential_id: &[u8]) -> String {
    hex::encode(Sha256::digest(credential_id))
}

/// A credential id as browsers write it (`PublicKeyCredential.id`):
/// base64url without padding.
pub(super) fn browser_id(credential_id: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut text = String::with_capacity(credential_id.len().div_ceil(3) * 4);
    for chunk in credential_id.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |n, (i, byte)| n | u32::from(*byte) << (16 - 8 * i));
        for i in 0..=chunk.len() {
            text.push(ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char);
        }
    }
    text
}

/// The user handle an authenticator stores with an account's passkeys and
/// returns when one signs in: fixed for the account, and says nothing about it.
pub(super) fn handle(user_id: &str) -> Uuid {
    let digest = Sha256::digest(format!("nabiman-passkey-user\0{user_id}"));
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    Uuid::from_bytes(bytes)
}

pub(super) fn now() -> String {
    chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

/// A name a person gave: trimmed, 1 to 64 characters, nothing unprintable.
pub(super) fn name(text: &str) -> Option<String> {
    let name = text.trim();
    let length = name.chars().count();
    ((1..=64).contains(&length) && !name.chars().any(char::is_control)).then(|| name.to_string())
}

fn holds<'a>(user: &'a NabimanUser, credential_id: &[u8]) -> Option<&'a StoredPasskey> {
    user.mfa.passkeys.iter().find(|p| p.credential.cred_id().as_slice() == credential_id)
}

/// The account that has this credential, and the passkey as stored.
pub(super) fn owner(store: &UserStore, credential_id: &[u8]) -> Option<(NabimanUser, StoredPasskey)> {
    let users = store.lock_safe();
    users.iter().find_map(|user| holds(user, credential_id).map(|p| (user.clone(), p.clone())))
}

/// Adds `passkey` to the account, if the account is still as it was when it
/// gave its password and nobody has this credential already.
pub(super) fn add(store: &UserStore, user_id: &str, password_hash: &str, passkey: StoredPasskey) -> Result<(), String> {
    let mut users = store.lock_safe();
    if users.iter().any(|u| holds(u, passkey.credential.cred_id()).is_some()) {
        return Err("This passkey is already registered".into());
    }
    let previous = users.clone();
    let user = users
        .iter_mut()
        .find(|u| u.id == user_id && u.active() && u.password_hash == password_hash)
        .ok_or_else(|| "Account changed; try again".to_string())?;
    if user.mfa.passkeys.len() >= super::MAX_PER_ACCOUNT {
        return Err("Too many passkeys".into());
    }
    user.mfa.passkeys.push(passkey);
    crate::users::commit_users(&mut users, previous)
}

/// Records a verified sign-in on the passkey (counter, backup flags, when)
/// and on the account, durably, and gives the account as it now is. Fails
/// when the passkey or the account went away meanwhile.
pub(super) fn used(store: &UserStore, user_id: &str, result: &AuthenticationResult) -> Result<NabimanUser, String> {
    crate::users::mutate_user(store, user_id, |user| {
        if !user.active() {
            return Err("Invalid credentials".into());
        }
        let stamp = now();
        let passkey = user
            .mfa
            .passkeys
            .iter_mut()
            .find(|p| p.credential.cred_id() == result.cred_id())
            .ok_or_else(|| "Invalid credentials".to_string())?;
        passkey.credential.update_credential(result);
        passkey.last_used = Some(stamp.clone());
        user.last_login = Some(stamp);
        Ok(user.clone())
    })
}

pub(super) fn stored(name: String, credential: Passkey) -> StoredPasskey {
    StoredPasskey { id: key(credential.cred_id()), name, created_at: now(), last_used: None, credential }
}
