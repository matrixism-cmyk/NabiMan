//! Looking at and closing tiles from a program — `nabiman-claude status` and
//! `close`, and their admin HTTP twins — by the same rules as the dashboard:
//! an account sees the tiles its session list shows it (its own, and ones
//! shared with it) and closes only its own.
//!
//! A tile is named the way people and Claude Code name it: its title, its
//! `pty_…` id, the address of the Claude in it (`name [ref]`, `bridge:…`), or
//! just the ref. A name that fits several tiles is refused with the list, so
//! nothing is closed on a guess.

use std::collections::HashMap;

use serde::Serialize;

use super::claude_peers::{ClaudePeer, Seen};
use super::session::TmuxSession;
use crate::audit::{append_entry, AuditEntry, AuditLog};

/// The orchestration guide, one text for the CLI, the socket and the pages.
pub(crate) const GUIDE: &str = include_str!("../../../docs/orchestration/guide-ko.md");
/// The most lines the guide may have (a test holds it to that).
#[cfg(test)]
pub(crate) const GUIDE_MAX_LINES: usize = 60;
pub(crate) const DEFAULT_LINES: usize = 20;
pub(crate) const MAX_LINES: usize = 200;

/// The date on the guide's last line (`_갱신: 2026-11-10_`).
pub(crate) fn guide_updated(text: &str) -> Option<&str> {
    let last = text.lines().rev().find(|l| !l.trim().is_empty())?;
    let date = last.trim().trim_matches('_').strip_prefix("갱신:")?.trim();
    (!date.is_empty()).then_some(date)
}

/// The screen lines asked for, kept between 1 and `MAX_LINES`.
pub(crate) fn lines(asked: Option<usize>) -> usize {
    asked.unwrap_or(DEFAULT_LINES).clamp(1, MAX_LINES)
}

/// One tile the asking account sees, with the Claude sessions in it.
#[derive(Clone, Debug, Default)]
pub(crate) struct Tile {
    pub id: String,
    pub tmux_name: String,
    /// Its title, or its label while it has none.
    pub title: String,
    pub label: String,
    pub owner: String,
    pub owner_id: String,
    /// The saved server an SSH tile is connected to, by name.
    pub server: Option<String>,
    pub claude: Vec<ClaudePeer>,
    /// The operator made it the lead of all Claude work (총괄).
    pub lead: bool,
}

impl Tile {
    fn busy(&self) -> Option<&ClaudePeer> {
        self.claude.iter().find(|p| p.busy)
    }
    /// The Claude session that speaks for the tile: a busy one, else the first.
    fn main_claude(&self) -> Option<&ClaudePeer> {
        self.busy().or_else(|| self.claude.first())
    }
    fn describe(&self) -> String {
        match self.main_claude() {
            Some(p) => format!("{} ({}) claude {}", self.title, self.id, p.address),
            None => format!("{} ({})", self.title, self.id),
        }
    }
}

/// The tiles an account sees, as its dashboard list does.
pub(crate) fn tiles_for(
    map: &HashMap<String, TmuxSession>,
    account_id: &str,
    visible: impl Fn(&TmuxSession) -> bool,
    seen: &HashMap<String, Seen>,
    lead: Option<&str>,
    server_name: impl Fn(&TmuxSession) -> Option<String>,
) -> Vec<Tile> {
    let mut out: Vec<Tile> = map
        .iter()
        .filter(|(_, s)| !account_id.is_empty() && visible(s))
        .map(|(id, s)| Tile {
            id: id.clone(),
            tmux_name: s.tmux_name.clone(),
            title: if s.title.is_empty() { s.label.clone() } else { s.title.clone() },
            label: s.label.clone(),
            owner: s.owner.clone(),
            owner_id: s.owner_id.clone(),
            server: server_name(s),
            claude: seen.get(&s.tmux_name).map(|h| h.claude.clone()).unwrap_or_default(),
            lead: lead == Some(s.tmux_name.as_str()),
        })
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// The ref in `who`: `ab12cd`, `[ab12cd]` or the end of `name [ab12cd]`.
fn ref_in(who: &str) -> Option<&str> {
    let inner = match who.rfind(" [") {
        Some(i) if who.ends_with(']') => &who[i + 2..who.len() - 1],
        _ => who.strip_prefix('[').and_then(|w| w.strip_suffix(']')).unwrap_or(who),
    };
    (inner.len() >= 6 && inner.len() <= 64 && inner.chars().all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))).then_some(inner)
}

/// The one tile `who` names among `tiles`. Tried in order — id, Claude
/// address, ref, title, Claude name, title in any case — and the first way
/// that fits anything decides; fitting several is an error naming them.
pub(crate) fn resolve<'a>(who: &str, tiles: &'a [Tile]) -> Result<&'a Tile, String> {
    let who = who.trim();
    if who.is_empty() {
        return Err("say which session: its title, its pty_… id, or its Claude address".into());
    }
    let reference = ref_in(who);
    let same_ref = |p: &ClaudePeer| {
        reference.is_some_and(|r| !p.reference.is_empty() && (p.reference.starts_with(r) || r.starts_with(p.reference.as_str())))
    };
    let lower = who.to_lowercase();
    let ways: [&dyn Fn(&Tile) -> bool; 6] = [
        &|t| t.id == who,
        &|t| t.claude.iter().any(|p| p.address == who),
        &|t| t.claude.iter().any(same_ref),
        &|t| t.title == who,
        &|t| t.claude.iter().any(|p| p.name == who),
        &|t| t.title.to_lowercase() == lower,
    ];
    for fits in ways {
        let found: Vec<&Tile> = tiles.iter().filter(|t| fits(t)).collect();
        match found.as_slice() {
            [] => continue,
            [one] => return Ok(one),
            many => {
                let names: Vec<String> = many.iter().map(|t| t.describe()).collect();
                return Err(format!("{who:?} fits {} sessions: {}; name one by its pty_… id", many.len(), names.join("; ")));
            }
        }
    }
    Err(format!("no session this account can see is called {who:?}"))
}

/// What `status` answers about a tile.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub(crate) struct Status {
    pub session_id: String,
    pub title: String,
    pub label: String,
    /// The saved server an SSH tile runs on; `null` for this machine.
    pub server: Option<String>,
    pub owner: String,
    pub alive: bool,
    /// `alive`, `ended`, `blocked` (its server approval changed) or `unknown`.
    pub state: &'static str,
    pub lead: bool,
    /// The address of its Claude (a busy one when several), for SendMessage.
    pub claude: Option<String>,
    pub name: Option<String>,
    pub busy: bool,
    /// When that Claude last became busy or idle (seconds since 1970).
    pub since: Option<u64>,
    pub model: Option<String>,
    pub version: Option<String>,
    pub cwd: Option<String>,
    /// Every Claude session in the tile.
    pub peers: Vec<ClaudePeer>,
    /// The last `lines` non-empty lines on its screen.
    pub lines: usize,
    pub screen: String,
}

pub(crate) fn status_of(tile: &Tile, state: &'static str, screen: String, lines: usize) -> Status {
    let main = tile.main_claude();
    Status {
        session_id: tile.id.clone(),
        title: tile.title.clone(),
        label: tile.label.clone(),
        server: tile.server.clone(),
        owner: tile.owner.clone(),
        alive: state == "alive",
        state,
        lead: tile.lead,
        claude: main.map(|p| p.address.clone()),
        name: main.map(|p| p.name.clone()),
        busy: tile.busy().is_some(),
        since: main.and_then(|p| p.since),
        model: main.and_then(|p| p.model.clone()),
        version: main.and_then(|p| p.version.clone()),
        cwd: main.map(|p| p.cwd.clone()),
        peers: tile.claude.clone(),
        lines,
        screen,
    }
}

/// Why a close is refused: an HTTP-like status and the reason.
pub(crate) type Refusal = (u16, String);

/// Whether `account_id` may close `tile` now. `caller_tmux` is the tmux
/// session the asking program runs in, when it is one of ours.
pub(crate) fn may_close(tile: &Tile, account_id: &str, caller_tmux: Option<&str>, force: bool) -> Result<(), Refusal> {
    if account_id.is_empty() || tile.owner_id != account_id {
        return Err((403, format!("{} is {}'s session, shared with this account: only its owner closes it", tile.title, tile.owner)));
    }
    if caller_tmux == Some(tile.tmux_name.as_str()) {
        return Err((403, "that is the session this runs in: a session does not close itself".into()));
    }
    if let (Some(p), false) = (tile.busy(), force) {
        return Err((409, format!("its Claude {} is working on a turn; wait until it is idle, or pass --force", p.address)));
    }
    if tile.lead && !force {
        return Err((409, "that is the lead session (총괄); pass --force only if the operator asked for it".into()));
    }
    Ok(())
}

/// Closes the tile in `map` as the dashboard does: tmux first (`shut`
/// hangs up what ran under it), then the record. A tile whose close cannot
/// be confirmed is kept, so it can be tried again.
pub(crate) fn close_in(
    map: &mut HashMap<String, TmuxSession>,
    id: &str,
    tmux_name: &str,
    shut: impl FnOnce(&str) -> std::io::Result<()>,
) -> Result<(), Refusal> {
    if map.get(id).is_none_or(|s| s.tmux_name != tmux_name) {
        return Err((404, "that session is already gone".into()));
    }
    shut(tmux_name).map_err(|_| (503, "could not confirm the session ended; it is kept".to_string()))?;
    map.remove(id);
    Ok(())
}

/// Writes one close (done or refused) to the audit journal; `false` when the
/// journal would not take it.
pub(crate) fn audit_close(log: &AuditLog, forwarding: Option<&crate::audit_forwarding::Forwarder>, actor: &str, id: &str, force: bool, status: u16) -> bool {
    let entry = AuditEntry {
        timestamp: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        user: actor.chars().filter(|c| !c.is_control()).take(300).collect(),
        method: "CLOSE".into(),
        path: format!("claude.sock/sessions/{}/close{}", id.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_').take(64).collect::<String>(), if force { "?force=1" } else { "" }),
        status,
        ip: "local".into(),
        hash: None,
        change: (status < 400).then(|| {
            let id: String = id.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_').take(64).collect();
            crate::audit_change::Change::new("terminal_session", &id, "close")
                .field("state", Some("open"), Some("closed"))
                .field("force", None, force.then_some("true"))
        }),
    };
    match append_entry(log, entry) {
        Some(saved) => {
            if let Some(f) = forwarding {
                f.enqueue(&saved);
            }
            true
        }
        None => false,
    }
}

#[cfg(test)]
#[path = "claude_control_tests.rs"]
mod tests;
