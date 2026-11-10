use super::*;
use std::sync::{Arc, Mutex};

fn peer(name: &str, reference: &str, busy: bool) -> ClaudePeer {
    ClaudePeer {
        name: name.into(),
        reference: reference.into(),
        address: if reference.is_empty() { name.into() } else { format!("{name} [{reference}]") },
        cwd: "/w".into(),
        busy,
        since: Some(100),
        model: Some("claude-opus-5-5".into()),
        version: Some("3.1.0".into()),
        server: None,
    }
}

fn tile(id: &str, title: &str, owner_id: &str, claude: Vec<ClaudePeer>) -> Tile {
    Tile {
        id: id.into(),
        tmux_name: format!("nb_x_{id}"),
        title: title.into(),
        label: "local".into(),
        owner: owner_id.trim_end_matches("-id").into(),
        owner_id: owner_id.into(),
        server: None,
        claude,
        lead: false,
    }
}

fn fleet() -> Vec<Tile> {
    vec![
        tile("pty_01", "nabiman", "admin-id", vec![peer("nabiman-5d", "1eb0c4", false)]),
        tile("pty_02", "review", "admin-id", vec![peer("review-3a", "946528", true)]),
        tile("pty_03", "review", "admin-id", vec![peer("review-9f", "df126b", false)]),
        tile("pty_04", "docs", "admin-id", vec![]),
    ]
}

#[test]
fn a_tile_is_named_by_id_address_ref_title_or_claude_name() {
    let all = fleet();
    assert_eq!(resolve("pty_04", &all).unwrap().id, "pty_04");
    assert_eq!(resolve("review-3a [946528]", &all).unwrap().id, "pty_02");
    assert_eq!(resolve("946528", &all).unwrap().id, "pty_02");
    assert_eq!(resolve("[df126b]", &all).unwrap().id, "pty_03");
    // A stale name with the right ref still finds it by the ref.
    assert_eq!(resolve("renamed [1eb0c4]", &all).unwrap().id, "pty_01");
    assert_eq!(resolve("docs", &all).unwrap().id, "pty_04");
    assert_eq!(resolve("review-9f", &all).unwrap().id, "pty_03");
    assert_eq!(resolve("  DOCS ", &all).unwrap().id, "pty_04", "any case, as a last resort");
    assert!(resolve("", &all).is_err());
    assert!(resolve("nothing", &all).unwrap_err().contains("no session"));
}

#[test]
fn a_name_that_fits_several_tiles_lists_them_and_picks_none() {
    let all = fleet();
    let e = resolve("review", &all).unwrap_err();
    assert!(e.contains("2 sessions") && e.contains("pty_02") && e.contains("pty_03"), "{e}");
    assert!(e.contains("review-3a [946528]"), "candidates carry their Claude address: {e}");
    // An exact id beats a title that happens to be someone's id.
    let mut odd = fleet();
    odd[3].title = "pty_01".into();
    assert_eq!(resolve("pty_01", &odd).unwrap().id, "pty_01");
}

#[test]
fn only_the_tiles_an_account_sees_are_candidates() {
    let users: crate::users::UserStore = Arc::new(Mutex::new(vec![
        serde_json::from_value(serde_json::json!({"id":"admin-id","username":"admin","password_hash":"x","role":"admin","created_at":"now"})).unwrap(),
        serde_json::from_value(serde_json::json!({"id":"op-id","username":"op","password_hash":"x","role":"admin","created_at":"now"})).unwrap(),
    ]));
    let session = |name: &str, owner_id: &str, shared: bool, title: &str| -> TmuxSession {
        serde_json::from_value(serde_json::json!({"tmux_name":name,"owner":owner_id,"owner_id":owner_id,"label":"local","shared":shared,"title":title})).unwrap()
    };
    let mut map = HashMap::new();
    map.insert("pty_a".to_string(), session("nb_a", "admin-id", false, "mine"));
    map.insert("pty_b".to_string(), session("nb_b", "op-id", false, "theirs"));
    map.insert("pty_c".to_string(), session("nb_c", "op-id", true, "shared"));
    let seen: HashMap<String, Seen> = HashMap::new();
    let mine = tiles_for(&map, "admin-id", |s| super::super::session::visible_to(s, "admin-id", &users), &seen, Some("nb_a"), |_| None);
    let ids: Vec<&str> = mine.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(ids, vec!["pty_a", "pty_c"], "another account's own tile is not visible");
    assert!(mine[0].lead && !mine[1].lead);
    assert!(resolve("theirs", &mine).is_err());
    assert!(resolve("pty_b", &mine).is_err());
    assert!(tiles_for(&map, "", |_| true, &seen, None, |_| None).is_empty(), "no account, no tiles");
    // Visible because shared, but closed only by its owner.
    let shared = resolve("shared", &mine).unwrap();
    assert_eq!(may_close(shared, "admin-id", None, true).unwrap_err().0, 403);
}

#[test]
fn a_session_does_not_close_itself_nor_a_busy_or_lead_one_unforced() {
    let all = fleet();
    let me = &all[0];
    assert_eq!(may_close(me, "admin-id", Some("nb_x_pty_01"), true).unwrap_err().0, 403, "not even forced");
    let busy = &all[1];
    let (code, why) = may_close(busy, "admin-id", Some("nb_x_pty_01"), false).unwrap_err();
    assert_eq!(code, 409);
    assert!(why.contains("review-3a [946528]") && why.contains("--force"), "{why}");
    assert!(may_close(busy, "admin-id", Some("nb_x_pty_01"), true).is_ok());
    let mut lead = all[2].clone();
    lead.lead = true;
    assert_eq!(may_close(&lead, "admin-id", None, false).unwrap_err().0, 409);
    assert!(may_close(&lead, "admin-id", None, true).is_ok());
    assert!(may_close(&all[3], "admin-id", Some("nb_x_pty_01"), false).is_ok());
    assert_eq!(may_close(&all[3], "", None, false).unwrap_err().0, 403);
}

#[test]
fn a_close_drops_the_record_only_once_tmux_confirms() {
    let s: TmuxSession = serde_json::from_value(serde_json::json!({"tmux_name":"nb_x","owner":"a","owner_id":"a-id","label":"local"})).unwrap();
    let mut map = HashMap::from([("pty_x".to_string(), s)]);
    let failed = close_in(&mut map, "pty_x", "nb_x", |_| Err(std::io::Error::other("stuck")));
    assert_eq!(failed.unwrap_err().0, 503);
    assert!(map.contains_key("pty_x"), "kept when the end is not confirmed");
    assert_eq!(close_in(&mut map, "pty_x", "nb_other", |_| Ok(())).unwrap_err().0, 404, "a reused id is not closed");
    let mut shut = Vec::new();
    close_in(&mut map, "pty_x", "nb_x", |n| {
        shut.push(n.to_string());
        Ok(())
    })
    .unwrap();
    assert_eq!(shut, vec!["nb_x"]);
    assert!(map.is_empty());
    assert_eq!(close_in(&mut map, "pty_x", "nb_x", |_| Ok(())).unwrap_err().0, 404);
}

#[test]
fn every_close_is_written_to_the_audit_journal_with_who_asked() {
    let dir = std::env::temp_dir().join(format!("nabiman-control-audit-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let log = crate::audit::log_at(dir.join("audit.jsonl"));
    assert!(audit_close(&log, None, "admin (claude.sock: nabi pid 42 in pty_01)", "pty_02", true, 200));
    assert!(audit_close(&log, None, "admin (claude.sock: nabi pid 42 in pty_01)", "pty_03\n../x", false, 409));
    let entries = crate::audit::entries_of(&log);
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].user, "admin (claude.sock: nabi pid 42 in pty_01)");
    assert_eq!((entries[0].method.as_str(), entries[0].path.as_str(), entries[0].status), ("CLOSE", "claude.sock/sessions/pty_02/close?force=1", 200));
    assert_eq!((entries[1].path.as_str(), entries[1].status), ("claude.sock/sessions/pty_03x/close", 409));
    assert!(entries.iter().all(|e| e.hash.as_deref().is_some_and(|h| h.starts_with("v2:"))), "chained like every record");
    let on_disk = std::fs::read_to_string(dir.join("audit.jsonl")).unwrap();
    assert_eq!(on_disk.lines().count(), 2);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn status_speaks_for_the_busy_claude_and_carries_the_screen() {
    let mut t = tile("pty_09", "pair", "admin-id", vec![peer("a-1", "aaaaaa", false), peer("b-2", "bbbbbb", true)]);
    t.server = Some("gpu-01".into());
    let s = status_of(&t, "alive", "line 1\nline 2".into(), 2);
    assert!(s.alive && s.busy && s.server.as_deref() == Some("gpu-01"));
    assert_eq!(s.claude.as_deref(), Some("b-2 [bbbbbb]"));
    assert_eq!((s.model.as_deref(), s.version.as_deref(), s.cwd.as_deref(), s.since), (Some("claude-opus-5-5"), Some("3.1.0"), Some("/w"), Some(100)));
    assert_eq!(s.peers.len(), 2);
    let quiet = status_of(&tile("pty_10", "t", "x", vec![]), "ended", String::new(), 20);
    assert!(!quiet.alive && !quiet.busy && quiet.claude.is_none());
    assert_eq!((lines(None), lines(Some(0)), lines(Some(5000)), lines(Some(40))), (20, 1, 200, 40));
}

#[test]
fn the_guide_is_there_short_and_dated() {
    assert!(!GUIDE.trim().is_empty());
    let n = GUIDE.lines().count();
    assert!(n <= GUIDE_MAX_LINES, "the guide has {n} lines; keep it to {GUIDE_MAX_LINES}");
    for must in ["nabiman-claude list", "→ nabiman]", "진흥원", "SendMessage", "status", "close", "--force"] {
        assert!(GUIDE.contains(must), "the guide should mention {must}");
    }
    assert_eq!(guide_updated(GUIDE).map(|d| d.len()), Some(10));
    assert_eq!(guide_updated("x\n_갱신: 2026-01-02_\n\n"), Some("2026-01-02"));
    assert_eq!(guide_updated("no date"), None);
}

#[test]
fn a_pasted_guide_says_whom_it_is_for() {
    let lead = super::super::claude_control_http::Lead { address: "nabiman-5d [1eb0c4]".into(), tile: "nabiman".into() };
    let h = super::super::claude_control_http::header("review", "pty_02", Some("review-3a [946528]"), Some(&lead));
    assert_eq!(h.lines().filter(|l| l.starts_with('>')).count(), 3);
    assert!(h.contains("«review» (pty_02)") && h.contains("`review-3a [946528]`") && h.contains("`nabiman-5d [1eb0c4]`"));
    let h = super::super::claude_control_http::header("docs", "pty_04", None, None);
    assert_eq!(h.lines().filter(|l| l.starts_with('>')).count(), 2);
    assert!(h.contains("nabiman-claude list"));
}

#[test]
fn the_guide_is_for_every_staff_account_and_status_and_close_for_administrators() {
    use crate::models::UserRole;
    use crate::route_access::{member_reachable, staff_allowed};
    for role in [UserRole::Operator, UserRole::Viewer] {
        assert!(staff_allowed(&role, "GET", "/api/orchestration/guide"), "{role:?}");
        assert!(!staff_allowed(&role, "GET", "/api/terminal/sessions/pty_1/status"), "{role:?}");
        assert!(!staff_allowed(&role, "POST", "/api/terminal/sessions/pty_1/close"), "{role:?}");
    }
    assert!(!member_reachable("GET", "/api/orchestration/guide"), "remote-only members get 403");
    // Reachable under the member rules for `/sessions/{id}/…`; the handlers refuse members.
    let route = crate::route_access::lookup("/api/terminal/sessions/pty_1/close").unwrap();
    assert_eq!((route.class, route.member), (crate::route_access::Class::Admin, crate::route_access::Member::Handler));
}
