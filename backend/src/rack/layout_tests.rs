use super::layout::*;

/// The shape of the real file, cut down: a confirmed node, a machine with
/// two candidates, a switch, and a device nobody has placed yet.
pub(super) const SAMPLE: &str = r#"{
 "version": "t1", "title": "Test room", "sources": ["a survey"], "notes": "n",
 "status_sources": {"cpu_util": "up{instance=\"<ip>\"}"},
 "racks": [
  {"id": "R1", "name": "RACK #1", "role": "core", "units": 42, "devices": [
   {"id": "sw", "label": "Switch", "type": "switch", "u": 38, "h": 1, "u_confirmed": true, "ip": "10.0.0.1", "future_field": 7},
   {"id": "lenovo-1", "label": "SR650 #1", "type": "server", "u": 27, "h": 2, "u_confirmed": false, "node": null, "node_candidates": ["mec-cp01", "mec-wn02"]},
   {"id": "lenovo-2", "label": "SR650 #2", "type": "server", "u": 24, "h": 2, "node": null, "node_candidates": ["mec-cp01", "mec-wn02"]},
   {"id": "gh-wn03", "label": "GH200", "type": "server", "u": null, "h": 2, "node": "mec-wn03", "ip": "10.0.0.204", "bmc": "10.0.0.206", "gpu": "GH200 x1", "cpu": 72, "arch": "arm64", "order_hint": 2},
   {"id": "odd", "label": "Thing", "type": "ups", "u": 1}
  ],
  "node_info": {
   "mec-cp01": {"ip": "10.0.0.201", "role": "control-plane", "model": "SR650 V3", "cpu": 32, "mem_gi": 125, "gpu": "-"},
   "mec-wn02": {"ip": "10.0.0.202", "role": "worker", "model": "SR650 V3", "cpu": 64, "mem_gi": 503, "gpu": "A40 x2"}
  }},
  {"id": "R3", "name": "RACK #3", "units": 42, "devices": [
   {"id": "t4", "label": "ESC4000", "type": "server", "u": 36, "h": 2, "node": "mec-cp02", "ip": "10.0.0.199"}
  ]}
 ],
 "power": {"UPS": "50A", "nested": {"x": 1}}
}"#;

fn with_devices(devices: &str) -> String {
    format!(r#"{{"racks":[{{"id":"R1","units":42,"devices":[{devices}]}}]}}"#)
}

fn fault(text: &str) -> String {
    parse(text.as_bytes()).expect_err("the file should be refused")
}

#[test]
fn a_good_file_is_read_and_cut_down_to_what_the_page_needs() {
    let layout = parse(SAMPLE.as_bytes()).unwrap();
    assert_eq!((layout.version.as_str(), layout.title.as_str(), layout.racks.len()), ("t1", "Test room", 2));
    assert_eq!(layout.racks[0].devices.len(), 5);
    assert_eq!(layout.power, [Pair { label: "UPS".into(), value: "50A".into() }], "only text entries");
    let json = serde_json::to_string(&layout).unwrap();
    assert!(!json.contains("status_sources") && !json.contains("cpu_util") && !json.contains("future_field"), "{json}");
    let sw = &layout.racks[0].devices[0];
    assert_eq!((sw.kind.as_str(), sw.u, sw.h, sw.u_confirmed, sw.ip.as_deref()), ("switch", Some(38), 1, true, Some("10.0.0.1")));
    assert!(layout.overlaps.is_empty());
}

#[test]
fn nodes_are_listed_once_each_with_what_the_file_knows() {
    let layout = parse(SAMPLE.as_bytes()).unwrap();
    let names: Vec<&str> = layout.nodes.iter().map(|n| n.name.as_str()).collect();
    assert_eq!(names, ["mec-cp01", "mec-wn02", "mec-wn03", "mec-cp02"], "candidates of two machines count once");
    let cp01 = &layout.nodes[0];
    assert_eq!((cp01.ip.as_deref(), cp01.role.as_deref(), cp01.cpu, cp01.mem_gi, cp01.gpu.as_deref()), (Some("10.0.0.201"), Some("control-plane"), Some(32), Some(125), None));
    let wn03 = &layout.nodes[2];
    assert_eq!((wn03.ip.as_deref(), wn03.model.as_deref(), wn03.arch.as_deref(), wn03.cpu), (Some("10.0.0.204"), Some("GH200"), Some("arm64"), Some(72)));
    let lenovo = &layout.racks[0].devices[1];
    assert_eq!((lenovo.node.as_deref(), lenovo.nodes()), (None, vec!["mec-cp01", "mec-wn02"]));
    assert_eq!(layout.racks[0].devices[3].nodes(), ["mec-wn03"]);
    assert!(layout.racks[0].devices[0].nodes().is_empty());
}

#[test]
fn a_file_without_the_newer_fields_still_reads() {
    let v1 = with_devices(r#"{"id":"a","label":"A","type":"server","u":27,"h":2,"u_confirmed":false,"node":null}"#);
    let layout = parse(v1.as_bytes()).unwrap();
    let a = &layout.racks[0].devices[0];
    assert!(a.node_candidates.is_empty() && a.node.is_none() && layout.nodes.is_empty());
    assert_eq!((layout.version.as_str(), layout.sources.len(), layout.power.len()), ("", 0, 0));
}

#[test]
fn an_unplaced_device_and_an_unknown_type_are_accepted() {
    let layout = parse(SAMPLE.as_bytes()).unwrap();
    let gh = &layout.racks[0].devices[3];
    assert_eq!((gh.u, gh.h, gh.order_hint, gh.bmc.as_deref()), (None, 2, Some(2), Some("10.0.0.206")));
    let odd = &layout.racks[0].devices[4];
    assert_eq!((odd.kind.as_str(), odd.h, odd.u), ("other", 1, Some(1)), "an unknown type is drawn as 'other'; h defaults to 1");
}

#[test]
fn u_is_the_top_slot_and_the_device_must_fit_below_it() {
    // h=2 at u=27 fills U26–U27; at u=1 it would reach U0.
    assert!(parse(with_devices(r#"{"id":"a","label":"A","u":2,"h":2}"#).as_bytes()).is_ok());
    assert!(fault(&with_devices(r#"{"id":"a","label":"A","u":1,"h":2}"#)).contains("devices[0].u"));
    assert!(fault(&with_devices(r#"{"id":"a","label":"A","u":43,"h":1}"#)).contains("does not fit U1–U42"));
    assert!(fault(&with_devices(r#"{"id":"a","label":"A","u":0}"#)).contains("devices[0].u"));
    assert!(fault(&with_devices(r#"{"id":"a","label":"A","u":5,"h":0}"#)).contains("devices[0].h"));
    // U26–U27 and U25–U26 share U26; U24–U25 and U26–U27 do not touch.
    let layout = parse(with_devices(r#"{"id":"a","label":"A","u":27,"h":2},{"id":"b","label":"B","u":26,"h":2},{"id":"c","label":"C","u":24,"h":1}"#).as_bytes()).unwrap();
    assert_eq!(layout.overlaps, [Overlap { rack: "R1".into(), first: "a".into(), second: "b".into() }]);
}

#[test]
fn garbage_and_missing_parts_are_refused_with_a_reason() {
    assert!(fault("").contains("not a rack layout"));
    assert!(fault("<html>").contains("not a rack layout"));
    assert!(fault("[]").contains("not a rack layout"));
    assert!(fault("{}").contains("racks"), "racks is required");
    assert!(fault(r#"{"racks":[]}"#).contains("racks: 0 given"));
    assert!(fault(r#"{"racks":[{"id":"R1","devices":[]}]}"#).contains("units"));
    assert!(fault(r#"{"racks":[{"id":"R1","units":"tall"}]}"#).contains("not a rack layout"));
    assert!(fault(&with_devices(r#"{"id":"a"}"#)).contains("label"));
    assert!(fault(&with_devices(r#"{"id":"a","label":"A","u":-3}"#)).contains("not a rack layout"));
    assert!(fault(&with_devices(r#"{"id":"a b","label":"A"}"#)).contains("devices[0].id"));
    assert!(fault(&with_devices(r#"{"id":"a","label":"A"},{"id":"a","label":"B"}"#)).contains("used twice"));
}

#[test]
fn limits_are_kept() {
    let rack = |i: usize, units: u32, devices: &str| format!(r#"{{"id":"R{i}","units":{units},"devices":[{devices}]}}"#);
    let racks = |n: usize| format!(r#"{{"racks":[{}]}}"#, (0..n).map(|i| rack(i, 42, "")).collect::<Vec<_>>().join(","));
    assert!(parse(racks(MAX_RACKS).as_bytes()).is_ok());
    assert!(fault(&racks(MAX_RACKS + 1)).contains("1–16 allowed"));
    assert!(fault(&format!(r#"{{"racks":[{}]}}"#, rack(0, MAX_UNITS + 1, ""))).contains("1–60 allowed"));
    assert!(fault(&format!(r#"{{"racks":[{}]}}"#, rack(0, 0, ""))).contains("units"));
    let many = |n: usize| (0..n).map(|i| format!(r#"{{"id":"d{i}","label":"D"}}"#)).collect::<Vec<_>>().join(",");
    assert!(parse(with_devices(&many(MAX_DEVICES)).as_bytes()).is_ok());
    assert!(fault(&with_devices(&many(MAX_DEVICES + 1))).contains("at most 400"));
    let long = "x".repeat(201);
    assert!(fault(&with_devices(&format!(r#"{{"id":"a","label":"{long}"}}"#))).contains("longer than 200"));
    assert!(fault(&with_devices(r#"{"id":"a","label":"A\u0007"}"#)).contains("control character"));
    let huge = format!(r#"{{"racks":[],"pad":"{}"}}"#, "x".repeat(MAX_FILE_BYTES as usize));
    assert!(fault(&huge).contains("larger than"));
}

#[test]
fn node_names_and_addresses_must_look_like_what_they_are() {
    for bad in [r#""node":"mec wn01""#, r#""node":"a\"}or{b=\"""#, r#""node":"-x""#, r#""ip":"10.0.0""#, r#""ip":"10.0.0.1:9796""#, r#""ip":"010.0.0.1""#, r#""bmc":"host""#, r#""node_candidates":["ok","no good"]"#] {
        let text = with_devices(&format!(r#"{{"id":"a","label":"A",{bad}}}"#));
        assert!(parse(text.as_bytes()).is_err(), "{bad}");
    }
    assert!(fault(&with_devices(r#"{"id":"a","label":"A","node_candidates":["a","b","c","d","e"]}"#)).contains("more than 4"));
    let four = parse(with_devices(r#"{"id":"a","label":"A","node_candidates":["a","b","c","d","a"]}"#).as_bytes());
    assert!(four.is_err(), "five entries are refused even when one repeats");
    let both = parse(with_devices(r#"{"id":"a","label":"A","node":"n1","node_candidates":["n2"]}"#).as_bytes()).unwrap();
    assert_eq!(both.racks[0].devices[0].nodes(), ["n1"], "a confirmed node leaves no candidates");
    assert!(is_node_name("mec-wn03") && is_node_name("node1.example.com") && !is_node_name("") && !is_node_name("a{b}"));
    let bad_info = r#"{"racks":[{"id":"R1","units":42,"node_info":{"n1":{"ip":"nope"}}}]}"#;
    assert!(fault(bad_info).contains("node_info.n1.ip"));
}

#[test]
fn power_notes_keep_the_order_of_the_file() {
    let text = r#"{"version":"v","title":"t","racks":[{"id":"R1","name":"RACK #1","units":42,"devices":[]}],
      "power":{"상전":"메인 100A","UPS":"메인 50A","skip":{"x":1},"note":"2025.9 기준"}}"#;
    let layout = parse(text.as_bytes()).expect("valid");
    let labels: Vec<&str> = layout.power.iter().map(|p| p.label.as_str()).collect();
    assert_eq!(labels, ["상전", "UPS", "note"]);
    // A power block that is not an object is a broken file, said plainly.
    assert!(parse(text.replace(r#""power":{"#, r#""power":[{"#).replace(r#"기준"}}"#, r#"기준"}]}"#).as_bytes()).is_err());
}

