//! Who has been handing work to whom among the Claude sessions found: the
//! arrows between cards, and the messages behind them.

use std::collections::HashMap;
use std::path::PathBuf;

use super::records::Record;
use super::{base_name, ghost_id, Seen};

/// `%XX` escapes in a peer address (Claude Code escapes unusual characters).
fn unescape(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Some(b) = s.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}

/// Who has been handing work to whom among the sessions found, here and on
/// the servers the tiles are connected to. A sender is named by its socket on
/// the receiver's machine (`uds:…`) or, across machines, by its Remote Control
/// session (`bridge:…`).
pub(super) fn link(found: &[(Record, String)], remote: &super::super::claude_remote::Snapshot, seen: &mut HashMap<String, Seen>) {
    // Each session's card (tmux name, or a remote one's own id), by address.
    let mut card: HashMap<String, String> = HashMap::new();
    for (r, t) in found {
        card.insert(format!("local|uds:{}", r.sock), t.clone());
        if let Some(b) = &r.bridge {
            card.insert(format!("bridge:{b}"), t.clone());
        }
    }
    let mut remote_card: HashMap<(&str, &str), String> = HashMap::new();
    for p in &remote.peers {
        if p.sock.is_empty() {
            continue; // no address (another user's Claude): nothing reaches or leaves it here
        }
        let c = p.tile.clone().unwrap_or_else(|| ghost_id(&p.server_id, p));
        card.insert(format!("{}|uds:{}", p.server_id, p.sock), c.clone());
        if let Some(b) = &p.bridge {
            card.insert(format!("bridge:{b}"), c.clone());
        }
        // A conversation open in several Claudes writes one transcript: its
        // messages go to the one in a tile, if any.
        if p.tile.is_some() || !remote_card.contains_key(&(p.server_id.as_str(), p.session_id.as_str())) {
            remote_card.insert((p.server_id.as_str(), p.session_id.as_str()), c);
        }
    }
    let sender = |machine: &str, from: &str| -> Option<String> {
        let from = unescape(from);
        if from.starts_with("bridge:") {
            card.get(&from).cloned()
        } else {
            card.get(&format!("{machine}|{from}")).cloned()
        }
    };
    let transcripts: Vec<(PathBuf, &str)> = found
        .iter()
        .filter_map(|(r, t)| {
            let dir = r.file.parent()?.parent()?;
            Some((super::super::claude_links::transcript_of(dir, &r.cwd, &r.session_id)?, t.as_str()))
        })
        .collect();
    let paths: Vec<PathBuf> = transcripts.iter().map(|(p, _)| p.clone()).collect();
    let now = super::super::session::now_unix();
    let got = super::super::claude_links::received(&paths);
    let mut messages = Vec::new();
    // The latest message along each pair, for the tooltip on its arrow.
    let mut latest: HashMap<(String, String), (u64, String)> = HashMap::new();
    let mut note = |f: &str, t: &str, at: u64, said: &str| {
        let slot = latest.entry((f.to_string(), t.to_string())).or_insert((0, String::new()));
        if at >= slot.0 && !said.is_empty() {
            *slot = (at, said.to_string());
        }
    };
    let mut events: Vec<super::super::claude_events::Seen> = Vec::new();
    for (path, to) in &transcripts {
        for (from, at, said) in got.get(path).into_iter().flatten() {
            if let Some(f) = sender("local", from) {
                note(&f, to, *at, said);
                events.push((f.clone(), to.to_string(), *at, said.clone()));
                messages.push((f, to.to_string(), *at));
            }
        }
    }
    for (server, receiver, from, at, said) in &remote.messages {
        let (Some(to), Some(f)) = (remote_card.get(&(server.as_str(), receiver.as_str())), sender(server, from)) else { continue };
        note(&f, to, *at, said);
        events.push((f.clone(), to.clone(), *at, said.clone()));
        messages.push((f, to.clone(), *at));
    }
    // The record behind the arrows: each message as an event, with its ends' names now.
    let mut names: HashMap<String, String> = found.iter().map(|(r, t)| (t.clone(), if r.name.is_empty() { base_name(&r.cwd) } else { r.name.clone() })).collect();
    for p in &remote.peers {
        if let Some(t) = &p.tile {
            names.entry(t.clone()).or_insert_with(|| p.name.clone());
        }
    }
    super::super::claude_events::record(&events, &names);
    let links = super::super::claude_links::links(&messages, now, super::super::claude_lead::lead_tmux().as_deref());
    for (from, to) in links.leads {
        let entry = seen.entry(from.clone()).or_default();
        entry.orchestrator = links.orchestrators.contains(&from);
        entry.notes = to.iter().filter_map(|t| Some((t.clone(), latest.get(&(from.clone(), t.clone()))?.clone()))).collect();
        entry.leads = to;
    }
}
