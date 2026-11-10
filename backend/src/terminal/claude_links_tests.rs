use super::*;

fn m(from: &str, to: &str, at: u64) -> (String, String, u64) {
    (from.into(), to.into(), at)
}

const NOW: u64 = 100_000;

#[test]
fn the_first_to_write_leads_and_answers_do_not_turn_it_round() {
    // art handed out work hours ago; the answers since are the latest.
    let l = links(&[m("art", "mpcf", NOW - 9000), m("mpcf", "art", NOW - 50), m("art", "nabi", NOW - 8000), m("nabi", "art", NOW - 20)], NOW, None);
    assert_eq!(l.leads["art"], vec!["mpcf", "nabi"]);
    assert_eq!(l.leads.len(), 1);
    assert_eq!(l.orchestrators, HashSet::from(["art".to_string()]));
}

#[test]
fn reports_back_to_the_lead_do_not_make_the_reporters_lead() {
    // nabiman tells everyone; each reports back, and one asks another for data.
    let l = links(&[
        m("nabi", "a", NOW - 900), m("nabi", "b", NOW - 900), m("a", "nabi", NOW - 600), m("b", "nabi", NOW - 500),
        m("a", "b", NOW - 300), m("b", "a", NOW - 200), m("a", "nabi", NOW - 100),
    ], NOW, None);
    assert_eq!(l.leads["nabi"], vec!["a", "b"]);
    assert_eq!(l.leads["a"], vec!["b"]);
    assert_eq!(l.orchestrators, HashSet::from(["nabi".to_string()]));
}

#[test]
fn the_lead_named_by_the_operator_leads_every_pair_it_is_in() {
    // Reports reach the lead first (an old exchange): still drawn from the lead.
    let l = links(&[m("a", "lead", NOW - 600), m("lead", "a", NOW - 500), m("b", "lead", NOW - 100), m("a", "b", NOW - 50)], NOW, Some("lead"));
    assert_eq!(l.leads["lead"], vec!["a", "b"]);
    assert_eq!(l.leads["a"], vec!["b"]);
    assert_eq!(l.orchestrators, HashSet::from(["lead".to_string()]));
}

#[test]
fn yesterdays_exchange_does_not_decide_todays() {
    // b led a yesterday; today a starts afresh after a long quiet spell.
    let l = links(&[m("b", "a", NOW - 86_400), m("a", "b", NOW - 86_300), m("a", "b", NOW - 600), m("b", "a", NOW - 300)], NOW, None);
    assert_eq!(l.leads["a"], vec!["b"]);
    assert!(!l.leads.contains_key("b"));
}

#[test]
fn a_pair_quiet_for_an_hour_is_no_longer_linked() {
    let l = links(&[m("a", "b", NOW - WINDOW_SECS - 1), m("b", "a", NOW - WINDOW_SECS - 1)], NOW, None);
    assert_eq!(l, Links::default());
}

#[test]
fn one_led_by_another_is_not_the_orchestrator_though_it_leads_too() {
    let l = links(&[m("a", "b", NOW - 3), m("a", "c", NOW - 3), m("a", "d", NOW - 3), m("b", "e", NOW - 2)], NOW, None);
    assert_eq!(l.orchestrators, HashSet::from(["a".to_string()]));
    assert_eq!(l.leads["b"], vec!["e"]);
}

#[test]
fn only_a_verified_peer_message_counts() {
    let real = br#"{"type":"user","timestamp":"2026-11-05T12:35:08.552Z","origin":{"kind":"peer","from":"uds:/tmp/cc-socks/1673538.sock","verifiedPeerPid":1673538,"body":"..."}}"#;
    assert_eq!(peer_message(real), Some(("uds:/tmp/cc-socks/1673538.sock".into(), 1_793_882_108, String::new(), String::new())));
    // Arrived mid-turn: an attachment, with the same checked origin.
    let queued = br#"{"type":"attachment","timestamp":"2026-11-05T12:53:54.219Z","attachment":{"type":"queued_command","origin":{"kind":"peer","from":"uds:/s","verifiedPeerPid":1,"msg_id":"m1"}}}"#;
    assert_eq!(peer_message(queued), Some(("uds:/s".into(), 1_793_883_234, "m1".into(), String::new())));
    // From another machine, over Remote Control.
    let bridged = br#"{"type":"user","timestamp":"2026-11-05T14:56:18.111Z","origin":{"kind":"peer","from":"bridge:session_01WD","msg_id":"m2"}}"#;
    assert_eq!(peer_message(bridged), Some(("bridge:session_01WD".into(), 1_793_890_578, "m2".into(), String::new())));
    // What it said, from either form, without the envelope.
    let said = br#"{"type":"attachment","timestamp":"2026-11-05T12:53:54.219Z","attachment":{"type":"queued_command","prompt":"<cross-session-message from=\"uds:/s\" from-name=\"a\">\n\n  Review the PR, then report.\nMore lines.\n</cross-session-message>","origin":{"kind":"peer","from":"uds:/s","verifiedPeerPid":1}}}"#;
    assert_eq!(peer_message(said).unwrap().3, "Review the PR, then report.");
    let parts = r#"{"type":"user","timestamp":"2026-11-05T12:53:54.219Z","message":{"content":[{"type":"text","text":"Another Claude session sent a message:\n<cross-session-message from=\"bridge:x\">\n작업 요청: 배포 \u0007확인\n</cross-session-message>"}]},"origin":{"kind":"peer","from":"bridge:x"}}"#;
    assert_eq!(peer_message(parts.as_bytes()).unwrap().3, "작업 요청: 배포 확인");
    // Quoted in a tool result, or claimed without the check: no.
    let quoted = br#"{"type":"user","timestamp":"2026-11-05T12:35:08.552Z","message":{"content":"\"kind\":\"peer\""}}"#;
    assert_eq!(peer_message(quoted), None);
    let unverified = br#"{"type":"user","timestamp":"2026-11-05T12:35:08.552Z","origin":{"kind":"peer","from":"uds:/s"}}"#;
    assert_eq!(peer_message(unverified), None);
}

#[test]
fn a_transcript_is_read_once_and_then_only_what_was_added() {
    let dir = std::env::temp_dir().join(format!("nabiman-links-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("t.jsonl");
    let line = |sock: &str, ts: &str| format!(
        "{{\"type\":\"user\",\"timestamp\":\"{ts}\",\"origin\":{{\"kind\":\"peer\",\"from\":\"uds:{sock}\",\"verifiedPeerPid\":1}}}}\n");
    fs::write(&path, line("/a", "2026-11-05T12:00:00Z")).unwrap();
    let mut tail = Tail::default();
    catch_up(&path, &mut tail).unwrap();
    assert_eq!(tail.got.len(), 1);
    let mut f = fs::OpenOptions::new().append(true).open(&path).unwrap();
    std::io::Write::write_all(&mut f, line("/b", "2026-11-05T12:00:30Z").as_bytes()).unwrap();
    std::io::Write::write_all(&mut f, b"{\"half a line").unwrap();
    catch_up(&path, &mut tail).unwrap();
    assert_eq!(tail.got.iter().map(|g| g.0.as_str()).collect::<Vec<_>>(), vec!["uds:/a", "uds:/b"]);
    // In the background: the first look may come back before the reading is done.
    let _ = received(std::slice::from_ref(&path));
    let mut got = Vec::new();
    for _ in 0..200 {
        std::thread::sleep(std::time::Duration::from_millis(10));
        got = received(std::slice::from_ref(&path)).remove(&path).unwrap_or_default();
        if got.len() == 2 {
            break;
        }
    }
    assert_eq!(got.len(), 2);
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn project_folders_are_named_the_way_claude_code_names_them() {
    assert_eq!(folder_name("/workspace/server/mpcf.or.kr"), "-workspace-server-mpcf-or-kr");
}
