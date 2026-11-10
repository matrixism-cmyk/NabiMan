use super::values::*;
use super::*;
use std::collections::BTreeMap;

fn map(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

fn lookup(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
    move |name| pairs.iter().find(|(k, _)| *k == name).map(|(_, v)| v.to_string())
}

#[test]
fn the_environment_wins_over_the_file_which_wins_over_the_default() {
    let file = map(&[("NABIMAN_PORT", "18080"), ("NABIMAN_SOFFICE", "/file/soffice")]);
    let env = lookup(&[("NABIMAN_PORT", "9000")]);
    assert_eq!(resolve("NABIMAN_PORT", &env, &file).as_deref(), Some("9000"));
    assert_eq!(resolve("NABIMAN_SOFFICE", &env, &file).as_deref(), Some("/file/soffice"));
    assert_eq!(resolve("NABIMAN_RHWP", &env, &file), None);

    // What startup does: only what the environment lacks is taken from the file.
    let (set, report) = plan(None, false, &file, &env).unwrap();
    assert_eq!(set, vec![("NABIMAN_SOFFICE".to_string(), "/file/soffice".to_string())]);
    assert_eq!(report.overridden, ["NABIMAN_PORT"]);
    assert_eq!(report.from_file, ["NABIMAN_SOFFICE"]);

    // And the typed getters put the default under both.
    let after = |name: &str| resolve(name, &env, &file);
    assert_eq!(number_in(&after, "NABIMAN_PORT", 8080u16), 9000);
    assert_eq!(number_in(&after, "NABIMAN_UPLOAD_MAX_MB", 1024u64), 1024);
    assert_eq!(text_in(&after, "NABIMAN_SOFFICE").as_deref(), Some("/file/soffice"));
}

#[test]
fn keys_become_environment_names() {
    let file = file::parse(
        r#"
port = 18080
NABIMAN_HSTS = true
trusted-proxies = ["127.0.0.1", "10.0.0.0/8"]
[dictation]
url = "https://speech.example.com"
[mec.lb]
pools = "a=10.0.0.1-10.0.0.9"
"#,
    )
    .unwrap();
    assert_eq!(
        file,
        map(&[
            ("NABIMAN_PORT", "18080"),
            ("NABIMAN_HSTS", "true"),
            ("NABIMAN_TRUSTED_PROXIES", "127.0.0.1,10.0.0.0/8"),
            ("NABIMAN_DICTATION_URL", "https://speech.example.com"),
            ("NABIMAN_MEC_LB_POOLS", "a=10.0.0.1-10.0.0.9"),
        ])
    );
}

#[test]
fn a_bad_file_is_refused_with_the_reason() {
    for (text, why) in [
        ("port = ", "expected"),
        ("\"a b\" = 1", "not a setting name"),
        ("port = 1\nNABIMAN_PORT = 2", "set twice"),
        ("x = [[1]]", "only plain values"),
        ("x = [\"a,b\"]", "comma"),
    ] {
        let err = file::parse(text).unwrap_err();
        assert!(err.contains(why), "{text}: {err}");
    }
    assert!(plan(None, false, &map(&[("NABIMAN_CONFIG", "/x")]), &|_| None).is_err());
    assert!(plan(None, true, &map(&[("NABIMAN_DATA_DIR", "/x")]), &|_| None).is_err());
    assert!(plan(None, false, &map(&[("NABIMAN_DATA_DIR", "/x")]), &|_| None).is_ok());
    let (_, report) = plan(None, false, &map(&[("NABIMAN_PORTT", "1")]), &|_| None).unwrap();
    assert_eq!(report.unknown, ["NABIMAN_PORTT"]);
}

#[test]
fn only_root_or_this_user_may_be_able_to_change_the_file() {
    assert!(file::check_owner(true, 0, 0o100600, 0).is_ok());
    assert!(file::check_owner(true, 1000, 0o100644, 1000).is_ok());
    assert!(file::check_owner(true, 1000, 0o100644, 0).unwrap_err().contains("uid 1000"));
    assert!(file::check_owner(true, 0, 0o100664, 0).unwrap_err().contains("chmod"));
    assert!(file::check_owner(true, 0, 0o100646, 0).is_err());
    assert!(file::check_owner(false, 0, 0o40755, 0).is_err());
}

#[test]
fn the_file_is_found_where_it_should_be() {
    let dir = std::env::temp_dir().join(format!("nabiman-config-{}", std::process::id()));
    let data = dir.join("data");
    std::fs::create_dir_all(&data).unwrap();
    let system = dir.join("etc.toml");
    assert_eq!(locate(None, &system, &data).unwrap(), None);
    std::fs::write(data.join("nabiman.toml"), "port = 1\n").unwrap();
    assert_eq!(locate(None, &system, &data).unwrap(), Some(data.join("nabiman.toml")));
    std::fs::write(&system, "port = 2\n").unwrap();
    assert_eq!(locate(None, &system, &data).unwrap(), Some(system.clone()));
    assert_eq!(locate(Some("none"), &system, &data).unwrap(), None);
    assert_eq!(locate(Some(""), &system, &data).unwrap(), None);
    assert!(locate(Some("/nonexistent/nabiman.toml"), &system, &data).unwrap_err().contains("does not exist"));
    let read = file::read(&system);
    // Temp files are this user's; a umask that leaves group write is refused.
    if std::fs::metadata(&system).map(|m| std::os::unix::fs::PermissionsExt::mode(&m.permissions()) & 0o022 == 0).unwrap() {
        assert_eq!(read.unwrap(), map(&[("NABIMAN_PORT", "2")]));
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn typed_values() {
    let get = lookup(&[("A", " 42 "), ("B", "x"), ("D", " a, b ,,c "), ("E", "  ")]);
    assert_eq!((number_in(&get, "A", 1u16), number_in(&get, "B", 7u16), number_in(&get, "Z", 9u16)), (42, 7, 9));
    assert_eq!(list_in(&get, "D"), ["a", "b", "c"]);
    assert_eq!(text_in(&get, "E"), None);
    assert!(list_in(&get, "E").is_empty());
}

#[test]
fn programs_are_found_on_a_given_path() {
    use std::os::unix::fs::PermissionsExt;
    let dir = std::env::temp_dir().join(format!("nabiman-config-path-{}", std::process::id()));
    let (a, b) = (dir.join("a"), dir.join("b"));
    std::fs::create_dir_all(&a).unwrap();
    std::fs::create_dir_all(&b).unwrap();
    std::fs::write(a.join("tool"), "").unwrap(); // not runnable
    std::fs::write(b.join("tool"), "#!/bin/sh\n").unwrap();
    std::fs::set_permissions(b.join("tool"), std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!("{}:{}:relative", a.display(), b.display());
    assert_eq!(find_program(&["tool"], &path, &[]), Some(b.join("tool")));
    assert_eq!(find_program(&["other"], &path, &[b.join("tool")]), Some(b.join("tool")));
    assert_eq!(find_program(&["other"], &path, &[]), None);
    let set = |name: &str| (name == "SET").then(|| "/opt/x/tool".to_string());
    assert_eq!(program_in(&set, "SET", &["tool"], &[]), Some("/opt/x/tool".into()));
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Every `NABIMAN_*` name the code reads is a known setting (or listed as not one).
#[test]
fn the_catalogue_covers_the_code() {
    fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let p = entry.path();
            if p.is_dir() { walk(&p, out) } else if p.extension().is_some_and(|e| e == "rs") { out.push(p) }
        }
    }
    let mut files = Vec::new();
    walk(std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/src")), &mut files);
    let mut missing = std::collections::BTreeSet::new();
    for f in files {
        let name = f.file_name().unwrap().to_string_lossy().to_string();
        if name.contains("test") {
            continue;
        }
        let text = std::fs::read_to_string(&f).unwrap();
        for part in text.split("\"NABIMAN_").skip(1) {
            let id: String = part.chars().take_while(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || *c == '_').collect();
            let full = format!("NABIMAN_{id}");
            if !id.is_empty() && part[id.len()..].starts_with('"') && !id.starts_with("TEST_") && !id.starts_with("LIMITS_TEST")
                && !KNOWN.contains(&full.as_str()) && !catalogue::NOT_SETTINGS.contains(&full.as_str()) {
                missing.insert(full);
            }
        }
    }
    assert!(missing.is_empty(), "settings missing from install_config/catalogue.rs: {missing:?}");
}
