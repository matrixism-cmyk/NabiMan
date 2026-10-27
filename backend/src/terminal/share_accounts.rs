//! An account's share links seen from outside the share routes: whether its
//! visitors may dictate, and the super administrator's view and revocation.
use super::*;

/// Whether the visitor of a link may dictate: only when its owner may.
pub(crate) fn owner_dictates(users: &UserStore, link: &ShareLink) -> bool {
    account_dictates(users, &link.owner_id)
}

/// Whether an account may dictate (and so may the visitors of its links):
/// any account but a remote member without the dictation permission.
pub(crate) fn account_dictates(users: &UserStore, owner_id: &str) -> bool {
    users.lock().is_ok_and(|all| {
        all.iter().any(|u| u.id == owner_id && (!crate::member_servers::is_member(u) || u.remote_permissions.dictation))
    })
}

/// An account's links, for the super administrator's view of it (no secrets:
/// the token is cut to a recognisable prefix).
pub(crate) fn links_of(shares: &ShareStore, users: &UserStore, owner_id: &str) -> Vec<ShareLink> {
    let all = shares.lock().ok().and_then(|r| r.all().ok()).unwrap_or_default();
    all.into_iter()
        .filter(|l| l.owner_id == owner_id)
        .map(|l| {
            let usable = live_link(shares, users, &l.token).is_some();
            let mut l = public_view(l);
            l.usable = usable;
            l.token.truncate(8);
            l
        })
        .collect()
}

/// Ends every link an account made; how many there were.
pub(crate) fn revoke_links_of(shares: &ShareStore, owner_id: &str) -> Result<usize, String> {
    let mut registry = shares.lock().map_err(|_| "Share registry unavailable".to_string())?;
    let mut ended = 0;
    registry
        .update(|map| {
            let before = map.len();
            map.retain(|_, l| l.owner_id != owner_id);
            ended = before - map.len();
            Ok(())
        })
        .map_err(|e| e.to_string())?;
    Ok(ended)
}
