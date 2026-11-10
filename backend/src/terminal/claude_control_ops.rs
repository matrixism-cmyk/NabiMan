//! `status`, `close` and `guide` on the Claude socket, and the lookups they
//! share with the admin HTTP routes. Who is asking comes from the kernel
//! (the caller's pid leads to the tile it runs in); that tile's NabiMan
//! account is whose dashboard the answers follow.

use crate::lock_safe::LockExt;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::claude_control::{self as control, Refusal, Status, Tile};
use super::session::{PtyStore, TmuxSession};
use crate::users::UserStore;

/// How long a look waits for servers no probe has reached yet.
const REMOTE_WAIT: Duration = Duration::from_secs(10);

/// What the socket's requests reach besides the sessions.
#[derive(Clone)]
pub(crate) struct Ctx {
    pub store: PtyStore,
    pub users: UserStore,
    pub audit: crate::audit::AuditLog,
    pub forwarding: Option<crate::audit_forwarding::Forwarder>,
}

/// The program asking, as the kernel and tmux tell it.
pub(crate) struct Caller {
    pub os_user: String,
    pub pid: u32,
    /// The tmux session it runs in, and that tile's id.
    pub tmux: String,
    pub tile_id: String,
    /// The NabiMan account owning that tile: the one it acts for.
    pub account_id: String,
    pub account: String,
}

impl Caller {
    pub(crate) fn find(os_user: &str, pid: u32, store: &PtyStore) -> Result<Caller, String> {
        let tmux = super::claude_peers::tmux_session_of_pid(pid).ok_or("only from inside a NabiMan session")?;
        let map = store.lock_safe();
        let (id, s) = map.iter().find(|(_, s)| s.tmux_name == tmux).ok_or("the calling session is not one of NabiMan's")?;
        Ok(Caller {
            os_user: os_user.to_string(),
            pid,
            tmux: tmux.clone(),
            tile_id: id.clone(),
            account_id: s.owner_id.clone(),
            account: s.owner.clone(),
        })
    }

    /// Who did it, for the audit journal.
    pub(crate) fn actor(&self) -> String {
        format!("{} (claude.sock: {} pid {} in {})", self.account, self.os_user, self.pid, self.tile_id)
    }
}

fn refuse(why: impl Into<String>) -> Value {
    json!({"ok": false, "error": why.into()})
}

fn server_name(s: &TmuxSession) -> Option<String> {
    let b = s.remote_binding.as_ref()?;
    Some(crate::remote_servers::find_server(b.server_id()).ok().flatten().map_or_else(|| b.server_id().to_string(), |srv| srv.name))
}

/// The tiles `account_id` sees, with what is known of their Claude sessions.
pub(crate) fn tiles(store: &PtyStore, users: &UserStore, account_id: &str) -> Vec<Tile> {
    if store.lock_safe().values().any(|s| s.remote_binding.is_some()) {
        super::claude_remote::want(store);
        let until = Instant::now() + REMOTE_WAIT;
        while super::claude_remote::pending() && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(500));
        }
    }
    // tmux and /proc are asked with the store unlocked.
    let seen = super::claude_peers::look();
    let lead = super::claude_lead::lead_tmux();
    let map = store.lock_safe().clone();
    control::tiles_for(&map, account_id, |s| super::session::visible_to(s, account_id, users), &seen, lead.as_deref(), server_name)
}

/// What `status` tells of the tile `who` names, among those the account sees.
pub(crate) fn status_for(store: &PtyStore, users: &UserStore, account_id: &str, who: &str, lines: Option<usize>) -> Result<Status, String> {
    let tiles = tiles(store, users, account_id);
    let tile = control::resolve(who, &tiles)?;
    let n = control::lines(lines);
    let allowed = store.lock_safe().get(&tile.id).is_some_and(super::remote_access::allowed);
    let state = match (allowed, super::session::tmux_session_state(&tile.tmux_name)) {
        (false, _) => "blocked",
        (_, Ok(true)) => "alive",
        (_, Ok(false)) => "ended",
        (_, Err(_)) => "unknown",
    };
    let screen = if state == "alive" { super::session::pane_preview(&tile.tmux_name, n) } else { String::new() };
    Ok(control::status_of(tile, state, screen, n))
}

/// Closes the tile `who` names, if the account may; the tile it named (when
/// it named one) comes back either way, for the record.
pub(crate) fn close_for(
    store: &PtyStore,
    users: &UserStore,
    account_id: &str,
    who: &str,
    caller_tmux: Option<&str>,
    force: bool,
) -> Result<Tile, (Option<Tile>, Refusal)> {
    let tiles = tiles(store, users, account_id);
    let tile = control::resolve(who, &tiles).map_err(|e| (None, (404, e)))?.clone();
    if let Err(r) = control::may_close(&tile, account_id, caller_tmux, force) {
        return Err((Some(tile), r));
    }
    let mut map = store.lock_safe();
    if let Err(r) = control::close_in(&mut map, &tile.id, &tile.tmux_name, super::session::close_tmux_session) {
        return Err((Some(tile), r));
    }
    super::session::persist(&map);
    Ok(tile)
}

pub(crate) fn status(who: &str, lines: Option<usize>, caller: &Caller, ctx: &Ctx) -> Value {
    match status_for(&ctx.store, &ctx.users, &caller.account_id, who, lines) {
        Ok(s) => json!({"ok": true, "status": s}),
        Err(e) => refuse(e),
    }
}

pub(crate) fn close(who: &str, force: bool, caller: &Caller, ctx: &Ctx) -> Value {
    let done = close_for(&ctx.store, &ctx.users, &caller.account_id, who, Some(&caller.tmux), force);
    let (tile, code, mut answer) = match done {
        Ok(t) => {
            let claude = t.claude.first().map(|p| p.address.clone());
            let answer = json!({"ok": true, "closed": {"session_id": t.id, "title": t.title, "claude": claude}});
            (Some(t), 200, answer)
        }
        Err((t, (code, why))) => (t, code, json!({"ok": false, "error": why, "code": code})),
    };
    let id = tile.as_ref().map_or("unresolved", |t| t.id.as_str());
    let audited = control::audit_close(&ctx.audit, ctx.forwarding.as_ref(), &caller.actor(), id, force, code);
    answer["audited"] = json!(audited);
    answer
}

pub(crate) fn guide() -> Value {
    json!({"ok": true, "guide": control::GUIDE})
}
