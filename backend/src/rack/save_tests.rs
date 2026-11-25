use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};

use super::json::Json;
use super::layout_tests::SAMPLE;
use super::moves::Move;
use super::reach_tests::quiet;
use super::save::{save, versioned, Failure, AFTER_LISTING, BEFORE_REPOINT, FAIL_REPOINT};
use super::Service;

const V5: &str = "rack_layout_20261125_v5.json";

/// A folder as the live one is: `rack_layout_current.json` linking to v5.
fn folder() -> (PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!("nabiman-rack-save-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(V5), SAMPLE).unwrap();
    symlink(V5, dir.join("rack_layout_current.json")).unwrap();
    let link = dir.join("rack_layout_current.json");
    (dir, link)
}

fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    names
}

fn mv(id: &str, rack: &str, u: u32) -> Move {
    Move { id: id.into(), rack: rack.into(), u }
}

/// 14:00 UTC on the 25th: 23:00 the same day in Korea.
fn evening() -> DateTime<Utc> {
    "2026-11-25T14:00:07Z".parse().unwrap()
}

#[test]
fn versioned_names_are_told_from_others() {
    assert_eq!(versioned("rack_layout_20261125_v5.json"), Some(("20261125", 5)));
    assert_eq!(versioned("rack_layout_20261125_v12.json"), Some(("20261125", 12)));
    for other in ["rack_layout_current.json", "rack_layout_2026112_v5.json", "rack_layout_20261125_v.json", "rack_layout_20261125_v05.json", "rack_layout_20261125_v5.json.bak", "rack_layout_20261125_v-1.json", "x_rack_layout_20261125_v5.json", "rack_layout_20261125_v99999999999.json"] {
        assert_eq!(versioned(other), None, "{other}");
    }
}

#[test]
fn a_save_writes_a_new_file_and_points_the_link_at_it() {
    let (dir, link) = folder();
    let saved = save(&link, "t1", &[mv("sw", "R1", 40), mv("lenovo-1", "R3", 20)], "admin", evening()).unwrap();
    assert_eq!((saved.version.as_str(), saved.file.as_str(), saved.moved.len()), ("20261125_v6", "rack_layout_20261125_v6.json", 2));
    assert_eq!(names(&dir), ["rack_layout_20261125_v5.json", "rack_layout_20261125_v6.json", "rack_layout_current.json"], "no temporary file is left");
    assert_eq!(std::fs::read_link(&link).unwrap(), Path::new("rack_layout_20261125_v6.json"), "a link to a name in its own folder");
    assert_eq!(std::fs::read_to_string(dir.join(V5)).unwrap(), SAMPLE, "the old file is as it was");
    let new = dir.join(&saved.file);
    assert_eq!(std::fs::metadata(&new).unwrap().permissions().mode() & 0o777, 0o644);
    let text = std::fs::read_to_string(&new).unwrap();
    assert!(text.ends_with("}\n") && text.contains("\n  \"version\": \"20261125_v6\",\n"), "{text}");
    // What was written is what was answered, and it is the unmoved file but for the moves.
    assert_eq!(super::layout::parse(text.as_bytes()).unwrap(), saved.layout);
    assert_eq!((saved.layout.racks[0].devices[0].u, saved.layout.racks[0].devices[0].u_confirmed), (Some(40), true));
    assert_eq!(saved.layout.racks[1].devices[1].id, "lenovo-1");
    let Some(Json::Array(history)) = Json::parse(text.as_bytes()).unwrap().get("history").cloned() else { panic!("history") };
    assert_eq!((history.len(), history[0].get("at").unwrap().as_str(), history[1].get("by").unwrap().as_str()), (2, Some("2026-11-25T14:00:07Z"), Some("admin")));

    // The next save starts from the new version; the old one is refused, and writes nothing.
    let stale = save(&link, "t1", &[mv("sw", "R1", 10)], "admin", evening()).unwrap_err();
    assert!(matches!(&stale, Failure::Conflict { current } if current == "20261125_v6"), "{stale:?}");
    assert_eq!(names(&dir).len(), 3);
    // After midnight in Korea the date moves on and the count starts again.
    let next = save(&link, "20261125_v6", &[mv("sw", "R1", 10)], "admin", "2026-11-25T15:00:00Z".parse().unwrap()).unwrap();
    assert_eq!(next.file, "rack_layout_20261126_v1.json");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_taken_name_is_never_reused() {
    let (dir, link) = folder();
    // Higher numbers of the day, and names that only look alike.
    for other in ["rack_layout_20261125_v9.json", "rack_layout_20261124_v30.json", "rack_layout_20261125_v40.json.bak"] {
        std::fs::write(dir.join(other), "other").unwrap();
    }
    let saved = save(&link, "t1", &[mv("sw", "R1", 40)], "admin", evening()).unwrap();
    assert_eq!(saved.file, "rack_layout_20261125_v10.json");
    // Other writers take v11 and v12 between the listing and the create: v13 it is, and theirs are theirs still.
    let theirs = dir.clone();
    AFTER_LISTING.with(|hook| {
        *hook.borrow_mut() = Some(Box::new(move || {
            std::fs::write(theirs.join("rack_layout_20261125_v11.json"), "theirs").unwrap();
            symlink("nowhere", theirs.join("rack_layout_20261125_v12.json")).unwrap();
        }))
    });
    let raced = save(&link, "20261125_v10", &[mv("sw", "R1", 41)], "admin", evening());
    AFTER_LISTING.with(|hook| *hook.borrow_mut() = None);
    assert_eq!(raced.unwrap().file, "rack_layout_20261125_v13.json");
    assert_eq!(std::fs::read_to_string(dir.join("rack_layout_20261125_v11.json")).unwrap(), "theirs");
    assert_eq!(std::fs::read_link(dir.join("rack_layout_20261125_v12.json")).unwrap(), Path::new("nowhere"), "a name held by anything at all is taken");
    assert_eq!(std::fs::read_to_string(dir.join("rack_layout_20261125_v9.json")).unwrap(), "other");
    assert_eq!(std::fs::read_link(&link).unwrap(), Path::new("rack_layout_20261125_v13.json"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn nothing_is_left_when_a_step_fails() {
    let (dir, link) = folder();
    let before = names(&dir);
    // A refused request.
    let refused = save(&link, "t1", &[mv("sw", "R1", 27)], "admin", evening()).unwrap_err();
    assert!(matches!(&refused, Failure::Refused(r) if r.reason == "overlap"), "{refused:?}");
    // The link cannot be pointed at the new file: the new file goes again.
    FAIL_REPOINT.with(|f| f.set(true));
    let failed = save(&link, "t1", &[mv("sw", "R1", 40)], "admin", evening()).unwrap_err();
    FAIL_REPOINT.with(|f| f.set(false));
    assert!(matches!(&failed, Failure::Storage(why) if why.contains("made to fail")), "{failed:?}");
    // Somebody else points the link elsewhere while this save writes: theirs stands.
    std::fs::write(dir.join("rack_layout_20261125_v77.json"), SAMPLE.replace("\"t1\"", "\"theirs\"")).unwrap();
    let hook_link = link.clone();
    BEFORE_REPOINT.with(|hook| {
        *hook.borrow_mut() = Some(Box::new(move || {
            std::fs::remove_file(&hook_link).unwrap();
            symlink("rack_layout_20261125_v77.json", &hook_link).unwrap();
        }))
    });
    let overtaken = save(&link, "t1", &[mv("sw", "R1", 40)], "admin", evening()).unwrap_err();
    BEFORE_REPOINT.with(|hook| *hook.borrow_mut() = None);
    assert!(matches!(&overtaken, Failure::Conflict { current } if current == "theirs"), "{overtaken:?}");
    assert_eq!(std::fs::read_link(&link).unwrap(), Path::new("rack_layout_20261125_v77.json"));
    std::fs::remove_file(dir.join("rack_layout_20261125_v77.json")).unwrap();
    std::fs::remove_file(&link).unwrap();
    symlink(V5, &link).unwrap();
    // A folder that cannot be written to (not so for root, who may write anywhere).
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();
    if std::fs::write(dir.join("probe"), "").is_err() {
        let closed = save(&link, "t1", &[mv("sw", "R1", 40)], "admin", evening()).unwrap_err();
        assert!(matches!(&closed, Failure::Storage(why) if why.contains("cannot create a new layout file")), "{closed:?}");
    }
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    let _ = std::fs::remove_file(dir.join("probe"));
    assert_eq!(names(&dir), before);
    assert_eq!(std::fs::read_link(&link).unwrap(), Path::new(V5));
    assert_eq!(std::fs::read_to_string(dir.join(V5)).unwrap(), SAMPLE);
    // A file that is not a layout, and one that is missing.
    std::fs::write(dir.join("broken.json"), "{").unwrap();
    assert!(matches!(save(&dir.join("broken.json"), "t1", &[mv("sw", "R1", 40)], "admin", evening()), Err(Failure::Unusable { code: "invalid", .. })));
    assert!(matches!(save(&dir.join("missing.json"), "t1", &[mv("sw", "R1", 40)], "admin", evening()), Err(Failure::Unusable { code: "unreadable", .. })));
    assert!(matches!(save(&dir, "t1", &[mv("sw", "R1", 40)], "admin", evening()), Err(Failure::Unusable { code: "unreadable", .. })));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_plain_file_is_replaced_by_a_copy_and_the_versions_stay_beside_it() {
    let dir = std::env::temp_dir().join(format!("nabiman-rack-save-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("layout.json");
    std::fs::write(&file, SAMPLE).unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o640)).unwrap();
    let saved = save(&file, "t1", &[mv("sw", "R1", 40)], "admin", evening()).unwrap();
    assert_eq!(saved.file, "rack_layout_20261125_v1.json");
    assert_eq!(names(&dir), ["layout.json", "rack_layout_20261125_v1.json"]);
    assert_eq!(std::fs::read(&file).unwrap(), std::fs::read(dir.join(&saved.file)).unwrap(), "the same text in both");
    assert!(!std::fs::symlink_metadata(&file).unwrap().file_type().is_symlink());
    assert_eq!(std::fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o640, "the file keeps who may read it");
    assert_eq!(std::fs::metadata(dir.join(&saved.file)).unwrap().permissions().mode() & 0o777, 0o644);
    let again = save(&file, "20261125_v1", &[mv("sw", "R1", 41)], "admin", evening()).unwrap();
    assert_eq!((again.file.as_str(), names(&dir).len()), ("rack_layout_20261125_v2.json", 3));
    // The setting names a versioned file itself: saving would change it, so it is refused.
    let direct = save(&dir.join("rack_layout_20261125_v2.json"), "20261125_v2", &[mv("sw", "R1", 5)], "admin", evening()).unwrap_err();
    assert!(matches!(&direct, Failure::Storage(why) if why.contains("versioned")), "{direct:?}");
    assert_eq!(names(&dir).len(), 3);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn of_two_saves_at_once_from_one_version_only_one_wins() {
    let (dir, link) = folder();
    let service = Arc::new(Service::new(Some(link.clone()), None, Duration::from_secs(60), quiet()));
    assert_eq!(service.layout().unwrap().version, "t1");
    let saves: Vec<_> = (0..8u32)
        .map(|i| {
            let service = service.clone();
            std::thread::spawn(move || service.save_moves("t1", &[mv("sw", "R1", 2 + i)], &format!("admin{i}")))
        })
        .collect();
    let results: Vec<_> = saves.into_iter().map(|s| s.join().unwrap()).collect();
    let won: Vec<_> = results.iter().filter_map(|r| r.as_ref().ok()).collect();
    assert_eq!(won.len(), 1, "{results:?}");
    assert!(results.iter().filter_map(|r| r.as_ref().err()).all(|e| matches!(e, Failure::Conflict { current } if current == &won[0].version)));
    assert_eq!(names(&dir).len(), 3, "the losers wrote nothing");
    assert_eq!(service.layout().unwrap().version, won[0].version, "the page reads the new file");
    // Off, or with no file named, there is nothing to save to.
    assert!(matches!(Service::new(None, None, Duration::from_secs(60), quiet()).save_moves("t1", &[], "admin"), Err(Failure::Off)));
    assert!(matches!(Service::new(None, Some("http://127.0.0.1:9".into()), Duration::from_secs(60), quiet()).save_moves("t1", &[], "admin"), Err(Failure::Unusable { code: "no_file", .. })));
    let _ = std::fs::remove_dir_all(dir);
}
