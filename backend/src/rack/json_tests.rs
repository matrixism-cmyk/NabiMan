use super::json::Json;

/// In the form the file is kept in: every kind of value, keys out of
/// alphabetical order, text that needs no escape and text that does.
const KEPT: &str = r#"{
  "version": "20261125_v5",
  "title": "서버실 랙 실장도",
  "zeta": 1,
  "alpha": [
    1.50,
    -0,
    1e3,
    12345678901234567890123,
    0.1E-7
  ],
  "query": "up{instance=\"<ip>:9796\"} \\ a/b\ttab\nline",
  "empty_list": [],
  "empty_object": {},
  "nested": {
    "b": null,
    "a": true,
    "b2": false,
    "devices": [
      {
        "id": "t4",
        "u": 36,
        "desc": "",
        "gpu": "T4 ×2"
      }
    ]
  },
  "history": []
}
"#;

#[test]
fn a_file_in_the_kept_form_comes_back_byte_for_byte() {
    let doc = Json::parse(KEPT.as_bytes()).unwrap();
    assert_eq!(doc.write(), KEPT);
    let keys: Vec<&str> = doc.entries().unwrap().iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(keys, ["version", "title", "zeta", "alpha", "query", "empty_list", "empty_object", "nested", "history"]);
    assert_eq!(doc.get("alpha"), Some(&Json::Array(["1.50", "-0", "1e3", "12345678901234567890123", "0.1E-7"].iter().map(|n| Json::Number(n.to_string())).collect())));
}

#[test]
fn any_other_spacing_is_brought_to_the_kept_form_and_nothing_else_changes() {
    let compact = r#"{"b":[1,{"y":null,"x":[]}] ,  "a" :"é\u00e9\ud83d\ude00\/\u0001\u007f" , "a": 2.0}"#;
    let want = "{\n  \"b\": [\n    1,\n    {\n      \"y\": null,\n      \"x\": []\n    }\n  ],\n  \"a\": \"éé😀/\\u0001\u{7f}\",\n  \"a\": 2.0\n}\n";
    let doc = Json::parse(compact.as_bytes()).unwrap();
    assert_eq!(doc.write(), want, "non-ASCII as itself, a repeated key kept, a number as written");
    assert_eq!(Json::parse(want.as_bytes()).unwrap(), doc, "and that reads back as the same document");
    for scalar in ["null", "true", " 12 ", "\"x\"", "[]", "{}"] {
        assert_eq!(Json::parse(scalar.as_bytes()).unwrap().write(), format!("{}\n", scalar.trim()));
    }
}

#[test]
fn what_is_not_json_is_refused_with_its_line() {
    for bad in [
        "", "{", "[1,]", "{\"a\":1,}", "{a:1}", "{\"a\" 1}", "01", "1.", "-", "1e", "+1", ".5", "nul", "tru", "\"open", "\"a\nb\"", "\"\\x\"",
        "\"\\ud800\"", "\"\\ud800\\u0041\"", "\"\\udc00\"", "\"\\u12\"", "1 2", "{} x", "[1 2]", "NaN", "'a'",
    ] {
        assert!(Json::parse(bad.as_bytes()).is_err(), "{bad:?} should be refused");
    }
    assert!(Json::parse(b"{\n\"a\": [\n1,\n]\n}").unwrap_err().contains("line 4"));
    assert!(Json::parse(&[b'"', 0xff, b'"']).unwrap_err().contains("UTF-8"));
    let deep = format!("{}1{}", "[".repeat(200), "]".repeat(200));
    assert!(Json::parse(deep.as_bytes()).unwrap_err().contains("deeply"));
    assert!(Json::parse(format!("{}1{}", "[".repeat(40), "]".repeat(40)).as_bytes()).is_ok());
}

#[test]
fn an_entry_is_changed_where_it_stands_and_a_new_one_goes_after_its_neighbour() {
    let mut device = Json::parse(br#"{"id": "a", "u": 3, "h": 2, "desc": "d"}"#).unwrap();
    device.set("u", Json::number(9), &["id"]);
    device.set("u_confirmed", Json::Bool(true), &["h", "u"]);
    device.set("last", Json::Null, &["missing"]);
    let keys: Vec<&str> = device.entries().unwrap().iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(keys, ["id", "u", "h", "u_confirmed", "desc", "last"]);
    assert_eq!((device.get("u"), device.get("id").and_then(Json::as_str)), (Some(&Json::Number("9".into())), Some("a")));
    let mut list = Json::Array(vec![]);
    list.set("x", Json::Null, &[]);
    assert_eq!(list, Json::Array(vec![]), "only an object has entries");
}

/// The real file, when one is named: `NABIMAN_RACK_ROUNDTRIP_FILE=<path>
/// cargo test rack::json_tests`. Read and written with no change, it must
/// come back byte for byte (it is kept in the form `write` produces).
#[test]
fn the_named_real_file_comes_back_byte_for_byte() {
    let Some(path) = std::env::var_os("NABIMAN_RACK_ROUNDTRIP_FILE") else { return };
    let before = std::fs::read(&path).unwrap();
    let after = Json::parse(&before).unwrap().write();
    let differing = before.iter().zip(after.as_bytes()).position(|(a, b)| a != b);
    assert!(before == after.as_bytes(), "{}: {} bytes read, {} written, first difference at byte {differing:?}", std::path::Path::new(&path).display(), before.len(), after.len());
    println!("round trip identical: {} bytes, {}", before.len(), std::path::Path::new(&path).display());
}
