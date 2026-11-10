//! Which tmux session a process runs in, found through its parents up to the
//! shell tmux started in a pane.

use std::collections::HashMap;
use std::fs;

/// How far up the parent processes a pane's shell is looked for.
const MAX_HOPS: usize = 32;

pub(super) struct Panes {
    pub(super) server_pid: u32,
    /// The shell tmux started in each pane, to the tmux session it is in.
    pub(super) session_of: HashMap<u32, String>,
}

pub(super) fn panes() -> Option<Panes> {
    // Plain names only: a time format (`#{session_activity}`) once crashed
    // the tmux server it was asked of, taking every session with it.
    let out = super::super::session::tmux()
        .args(["list-panes", "-a", "-F", "#{pid}\t#{pane_pid}\t#{session_name}"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    parse_panes(&String::from_utf8_lossy(&out.stdout))
}

fn parse_panes(text: &str) -> Option<Panes> {
    let mut p = Panes { server_pid: 0, session_of: HashMap::new() };
    for line in text.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() != 3 {
            continue;
        }
        p.server_pid = f[0].parse().ok()?;
        if let Ok(pane_pid) = f[1].parse() {
            p.session_of.insert(pane_pid, f[2].to_string());
        }
    }
    (p.server_pid != 0).then_some(p)
}

/// The tmux session a process runs in, found through its parents.
pub(crate) fn tmux_session_of_pid(pid: u32) -> Option<String> {
    let panes = panes()?;
    up_to_pane(pid, &panes, |p| {
        let stat = fs::read_to_string(format!("/proc/{p}/stat")).ok()?;
        stat_fields(&stat).map(|(_, ppid)| ppid)
    })
}

/// Every agent process under a pane of this tmux server: (tmux session, kind).
pub(super) fn local_agents(panes: &Panes) -> Vec<(String, &'static str)> {
    let parent = |pid: u32| fs::read_to_string(format!("/proc/{pid}/stat")).ok().and_then(|s| stat_fields(&s)).map(|(_, p)| p);
    let mut out = Vec::new();
    for entry in fs::read_dir("/proc").into_iter().flatten().flatten() {
        let Some(pid) = entry.file_name().to_str().and_then(|n| n.parse::<u32>().ok()) else { continue };
        let Ok(raw) = fs::read(entry.path().join("cmdline")) else { continue };
        let text = String::from_utf8_lossy(&raw[..raw.len().min(4096)]).to_string();
        let argv: Vec<&str> = text.split('\0').filter(|a| !a.is_empty()).take(8).collect();
        let Some(kind) = super::super::ai_agents::kind_of(&argv) else { continue };
        if let Some(tmux) = up_to_pane(pid, panes, parent) {
            out.push((tmux, kind));
        }
    }
    out
}

/// Climbs from `pid` through `parent` to a pane's shell, stopping at the
/// tmux server itself or after `MAX_HOPS`.
pub(super) fn up_to_pane(mut pid: u32, panes: &Panes, parent: impl Fn(u32) -> Option<u32>) -> Option<String> {
    for _ in 0..MAX_HOPS {
        if let Some(session) = panes.session_of.get(&pid) {
            return Some(session.clone());
        }
        if pid <= 1 || pid == panes.server_pid {
            return None;
        }
        pid = parent(pid)?;
    }
    None
}

/// The start time (field 22) and parent (field 4) in `/proc/<pid>/stat`.
pub(crate) fn stat_fields(stat: &str) -> Option<(String, u32)> {
    // The command name (field 2) may hold spaces and parentheses.
    let rest: Vec<&str> = stat[stat.rfind(')')? + 1..].split_whitespace().collect();
    Some((rest.get(19)?.to_string(), rest.get(1)?.parse().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stat_survives_odd_command_names() {
        let stat = "1406230 (my (odd) cmd) S 1406007 1406230 1 34816 1 4194560 1 2 3 4 5 6 7 8 20 0 12 0 511134282 1 2";
        assert_eq!(stat_fields(stat), Some(("511134282".to_string(), 1406007)));
    }

    #[test]
    fn a_process_belongs_to_the_pane_its_parents_lead_to() {
        // claude → bash → su → the pane's shell, as `su nabi` in a pane runs it.
        let panes = parse_panes("427843\t1292830\tnb_a\n427843\t500\tnb_b\n").unwrap();
        let parents = HashMap::from([(1406230, 1406007), (1406007, 1406005), (1406005, 1292830), (1292830, 427843), (900, 1)]);
        let parent = |pid: u32| parents.get(&pid).copied();
        assert_eq!(up_to_pane(1406230, &panes, parent).as_deref(), Some("nb_a"));
        // Not under any pane: started elsewhere, or reparented to init.
        assert_eq!(up_to_pane(900, &panes, parent), None);
        // A loop in a made-up tree ends too.
        let lp = HashMap::from([(7, 8), (8, 7)]);
        assert_eq!(up_to_pane(7, &panes, |p| lp.get(&p).copied()), None);
    }

    #[test]
    fn panes_map_to_their_sessions() {
        let p = parse_panes("427843\t1292830\tnb_a\n427843\t1672302\tnb_b\n").unwrap();
        assert_eq!(p.server_pid, 427843);
        assert_eq!(p.session_of[&1672302], "nb_b");
        assert_eq!(p.session_of[&1292830], "nb_a");
        assert!(parse_panes("").is_none());
    }
}
