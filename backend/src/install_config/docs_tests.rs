//! The documents agree with the code: every setting is described, and the
//! sample files read as settings NabiMan knows.

use super::*;

const DOCS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../docs/operations/");

fn read(name: &str) -> String {
    std::fs::read_to_string(format!("{DOCS}{name}")).unwrap()
}

fn all_known(settings: &std::collections::BTreeMap<String, String>) {
    for name in settings.keys() {
        assert!(KNOWN.contains(&name.as_str()), "{name} is not a setting");
    }
}

#[test]
fn every_setting_is_described() {
    let doc = read("configuration.md");
    let missing: Vec<&&str> = KNOWN.iter().filter(|name| !doc.contains(&format!("`{name}`"))).collect();
    assert!(missing.is_empty(), "describe in docs/operations/configuration.md: {missing:?}");
}

#[test]
fn the_example_file_reads_with_every_line_switched_on() {
    let text = read("nabiman.example.toml");
    assert!(file::parse(&text).is_ok());
    // Uncomment `# key = value` lines (not prose) and read it again.
    let on: String = text.lines().map(|line| {
        let rest = line.strip_prefix("# ").unwrap_or(line);
        let key = rest.split(" = ").next().unwrap_or("");
        let setting = rest.contains(" = ") && !key.is_empty() && key.bytes().all(|b| b.is_ascii_lowercase() || b == b'_');
        format!("{}\n", if setting && line.starts_with("# ") { rest.split("  #").next().unwrap() } else { line })
    }).collect();
    let settings = file::parse(&on).unwrap();
    assert!(settings.len() >= 20, "{settings:?}");
    all_known(&settings);
}

#[test]
fn this_box_config_is_valid_and_has_its_pools() {
    let doc = read("this-box-config.md");
    let block = doc.split("```toml\n").nth(1).and_then(|b| b.split("```").next()).unwrap();
    let settings = file::parse(block).unwrap();
    all_known(&settings);
    let get = |name: &str| settings.get(name).cloned();
    assert_eq!(settings::mec_lb_pools_in(&get).len(), 2);
    assert!(settings::letsencrypt_email_in(&get).is_ok());
    assert!(crate::branding::feedback_in(&get).is_some());
}
