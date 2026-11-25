use super::json::Json;
use super::layout::{self, Layout};
use super::layout_tests::SAMPLE;
use super::moves::*;

fn sample() -> Layout {
    layout::parse(SAMPLE.as_bytes()).unwrap()
}

fn mv(id: &str, rack: &str, u: u32) -> Move {
    Move { id: id.into(), rack: rack.into(), u }
}

fn reason(moves: &[Move]) -> &'static str {
    plan(&sample(), moves).expect_err("should be refused").reason
}

#[test]
fn a_move_names_a_device_and_a_place_that_exist_and_fit() {
    assert_eq!(reason(&[mv("nobody", "R1", 5)]), "unknown_device");
    assert_eq!(reason(&[mv("sw", "R9", 5)]), "unknown_rack");
    assert_eq!(reason(&[mv("sw", "../R1", 5)]), "unknown_rack");
    // lenovo-1 is 2U: its top slot is at least U2 and at most the rack's height.
    for u in [0, 1, 43, 4_000_000_000] {
        assert_eq!(reason(&[mv("lenovo-1", "R1", u)]), "out_of_rack", "U{u}");
    }
    assert!(plan(&sample(), &[mv("lenovo-1", "R3", 2)]).is_ok() && plan(&sample(), &[mv("lenovo-1", "R1", 42)]).is_ok());
    assert_eq!(reason(&[mv("sw", "R1", 10), mv("sw", "R1", 11)]), "duplicate");
    assert_eq!(reason(&(0..65).map(|_| mv("sw", "R1", 10)).collect::<Vec<_>>()), "too_many");
    let refused = plan(&sample(), &[mv("no\u{7}body", "R1", 5)]).unwrap_err();
    assert!(!refused.detail.contains('\u{7}'), "{}", refused.detail);
}

#[test]
fn a_request_that_moves_nothing_is_refused_and_a_move_to_where_it_is_does_not_count() {
    assert_eq!(reason(&[]), "nothing_to_move");
    assert_eq!(reason(&[mv("sw", "R1", 38), mv("t4", "R3", 36)]), "nothing_to_move");
    let moved = plan(&sample(), &[mv("sw", "R1", 38), mv("t4", "R3", 40)]).unwrap();
    assert_eq!(moved, [Moved { id: "t4".into(), from_rack: "R3".into(), from_u: Some(36), to_rack: "R3".into(), to_u: 40 }]);
    // A device nobody had placed gets its first slot.
    let placed = plan(&sample(), &[mv("gh-wn03", "R3", 10)]).unwrap();
    assert_eq!((placed[0].from_rack.as_str(), placed[0].from_u, placed[0].to_rack.as_str()), ("R1", None, "R3"));
}

#[test]
fn no_moved_device_may_share_a_slot_once_all_the_moves_are_made() {
    // lenovo-1 fills U26–27, lenovo-2 U23–24, sw U38.
    assert_eq!(reason(&[mv("sw", "R1", 27)]), "overlap");
    assert_eq!(reason(&[mv("sw", "R1", 26)]), "overlap");
    assert!(plan(&sample(), &[mv("sw", "R1", 25)]).is_ok());
    assert_eq!(reason(&[mv("lenovo-1", "R1", 25)]), "overlap", "its lower slot would be lenovo-2's top");
    assert_eq!(reason(&[mv("lenovo-1", "R3", 37)]), "overlap", "another rack's devices count there");
    assert!(plan(&sample(), &[mv("lenovo-1", "R3", 38)]).is_ok());
    // Two devices swap: fine together, refused one by one.
    assert_eq!(reason(&[mv("lenovo-1", "R1", 24)]), "overlap");
    let swap = plan(&sample(), &[mv("lenovo-1", "R1", 24), mv("lenovo-2", "R1", 27)]).unwrap();
    assert_eq!(swap.len(), 2);
    // Two moved devices onto one slot.
    let refused = plan(&sample(), &[mv("sw", "R3", 10), mv("odd", "R3", 10)]).unwrap_err();
    assert_eq!(refused.reason, "overlap");
    assert_eq!(refused.detail, "sw: U10 of R3 is taken by odd");
    assert_eq!(plan(&sample(), &[mv("lenovo-1", "R3", 37)]).unwrap_err().detail, "lenovo-1: U36–U37 of R3 is taken by t4");
    // Where a moved device was is free for another.
    assert!(plan(&sample(), &[mv("sw", "R1", 10), mv("odd", "R1", 38)]).is_ok());
}

fn rewritten(moves: &[Move]) -> Json {
    let mut doc = Json::parse(SAMPLE.as_bytes()).unwrap();
    let moved = plan(&sample(), moves).unwrap();
    rewrite(&mut doc, &moved, "2026-11-25T14:00:07Z", "admin").unwrap();
    set_version(&mut doc, "20261125_v6");
    doc
}

fn keys(value: &Json) -> Vec<&str> {
    value.entries().unwrap().iter().map(|(k, _)| k.as_str()).collect()
}

fn devices<'a>(doc: &'a Json, rack: usize) -> &'a [Json] {
    let Some(Json::Array(racks)) = doc.get("racks") else { panic!("racks") };
    let Some(Json::Array(devices)) = racks[rack].get("devices") else { panic!("devices") };
    devices
}

#[test]
fn only_the_moved_devices_the_version_and_the_history_change() {
    let before = Json::parse(SAMPLE.as_bytes()).unwrap();
    let after = rewritten(&[mv("sw", "R1", 40), mv("lenovo-2", "R1", 20)]);
    assert_eq!(keys(&after), ["version", "title", "sources", "notes", "status_sources", "racks", "power", "history"], "history is added last; the rest keeps its order");
    for key in ["title", "sources", "notes", "status_sources", "power"] {
        assert_eq!(after.get(key), before.get(key), "{key}");
    }
    assert_eq!(after.get("version"), Some(&Json::text("20261125_v6")));
    let sw = &devices(&after, 0)[0];
    assert_eq!(keys(sw), ["id", "label", "type", "u", "h", "u_confirmed", "ip", "future_field"], "an unknown field stays");
    assert_eq!((sw.get("u"), sw.get("u_confirmed"), sw.get("future_field")), (Some(&Json::Number("40".into())), Some(&Json::Bool(true)), Some(&Json::Number("7".into()))));
    // lenovo-2 had no u_confirmed: it goes right after h.
    let lenovo2 = &devices(&after, 0)[2];
    assert_eq!(keys(lenovo2), ["id", "label", "type", "u", "h", "u_confirmed", "node", "node_candidates"]);
    assert_eq!((lenovo2.get("u"), lenovo2.get("h")), (Some(&Json::Number("20".into())), Some(&Json::Number("2".into()))));
    for untouched in [1, 3, 4] {
        assert_eq!(devices(&after, 0)[untouched], devices(&before, 0)[untouched]);
    }
    assert_eq!(devices(&after, 1), devices(&before, 1));
    let want = r#"[
  {
    "at": "2026-11-25T14:00:07Z",
    "by": "admin",
    "id": "sw",
    "from": {
      "rack": "R1",
      "u": 38
    },
    "to": {
      "rack": "R1",
      "u": 40
    }
  },
  {
    "at": "2026-11-25T14:00:07Z",
    "by": "admin",
    "id": "lenovo-2",
    "from": {
      "rack": "R1",
      "u": 24
    },
    "to": {
      "rack": "R1",
      "u": 20
    }
  }
]
"#;
    assert_eq!(after.get("history").unwrap().write(), want);
    assert!(layout::parse(after.write().as_bytes()).is_ok());
}

#[test]
fn a_device_moved_to_another_rack_leaves_its_list_and_joins_the_end_of_the_other() {
    let before = Json::parse(SAMPLE.as_bytes()).unwrap();
    let after = rewritten(&[mv("lenovo-1", "R3", 20), mv("gh-wn03", "R3", 10)]);
    let left: Vec<&str> = devices(&after, 0).iter().map(|d| d.get("id").unwrap().as_str().unwrap()).collect();
    let joined: Vec<&str> = devices(&after, 1).iter().map(|d| d.get("id").unwrap().as_str().unwrap()).collect();
    assert_eq!((left, joined), (vec!["sw", "lenovo-2", "odd"], vec!["t4", "lenovo-1", "gh-wn03"]));
    let lenovo = &devices(&after, 1)[1];
    assert_eq!(keys(lenovo), keys(&devices(&before, 0)[1]));
    assert_eq!((lenovo.get("u_confirmed"), lenovo.get("node_candidates")), (Some(&Json::Bool(true)), devices(&before, 0)[1].get("node_candidates")));
    // It had `"u": null`: the first slot it gets is noted as coming from none.
    let Some(Json::Array(history)) = after.get("history") else { panic!("history") };
    assert_eq!(history[1].get("from").unwrap().get("u"), Some(&Json::Null));
    assert_eq!(history[1].get("to").unwrap().write(), "{\n  \"rack\": \"R3\",\n  \"u\": 10\n}\n");
}

#[test]
fn history_keeps_the_newest_five_hundred_and_a_version_is_given_to_a_file_without_one() {
    let mut doc = Json::parse(SAMPLE.replace(r#""power":"#, &format!(r#""history": [{}], "power":"#, (0..499).map(|i| format!(r#"{{"n": {i}}}"#)).collect::<Vec<_>>().join(","))).as_bytes()).unwrap();
    let moved = plan(&sample(), &[mv("sw", "R1", 40), mv("odd", "R1", 2), mv("t4", "R3", 30)]).unwrap();
    rewrite(&mut doc, &moved, "t", "admin").unwrap();
    let Some(Json::Array(history)) = doc.get("history") else { panic!("history") };
    assert_eq!(history.len(), MAX_HISTORY);
    assert_eq!(history[0], Json::parse(br#"{"n": 2}"#).unwrap(), "the two oldest went");
    assert_eq!(history[499].get("id"), Some(&Json::text("t4")));
    assert_eq!(keys(&doc)[6..], ["history", "power"], "an existing history stays where it is");

    let mut bare = Json::parse(br#"{"racks": []}"#).unwrap();
    set_version(&mut bare, "20261125_v1");
    assert_eq!(keys(&bare), ["version", "racks"]);
    let mut odd = Json::parse(SAMPLE.replace(r#""power":"#, r#""history": "none", "power":"#).as_bytes()).unwrap();
    assert!(rewrite(&mut odd, &moved, "t", "admin").unwrap_err().contains("history"));
}
