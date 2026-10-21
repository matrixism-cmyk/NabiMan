use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use super::*;

/// A fresh folder for one test, gone with it.
struct Dir(std::path::PathBuf);
impl Dir {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!("nabiman-copy-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&p).unwrap();
        Dir(p)
    }
    fn at(&self, rel: &str) -> String {
        self.0.join(rel).to_string_lossy().into_owned()
    }
}
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn write(path: &str, body: &str) {
    std::fs::create_dir_all(Path::new(path).parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
}

fn leftovers(dir: &str) -> Vec<String> {
    walk(Path::new(dir)).into_iter().filter(|p| p.contains(".nbup-")).collect()
}

fn walk(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() { out.extend(walk(&p)) } else { out.push(p.to_string_lossy().into_owned()) }
    }
    out
}

#[actix_web::test]
async fn a_folder_arrives_whole_with_modes_and_links_left_out() {
    let d = Dir::new();
    write(&d.at("src/top.txt"), "top");
    write(&d.at("src/tree/a/b.txt"), "deep");
    std::fs::set_permissions(d.at("src/tree/a/b.txt"), std::fs::Permissions::from_mode(0o640)).unwrap();
    std::os::unix::fs::symlink(d.at("src/top.txt"), d.at("src/tree/link")).unwrap();
    std::fs::create_dir_all(d.at("dst")).unwrap();
    let job = Job::for_test();
    run(job.clone(), Fs::Local, Fs::Local, vec![d.at("src/top.txt"), d.at("src/tree")], d.at("dst")).await;
    let p = job.test_progress();
    assert_eq!(p["state"], "done");
    assert_eq!((p["files_done"].as_u64(), p["bytes_done"].as_u64()), (Some(2), Some(7)));
    assert_eq!(std::fs::read_to_string(d.at("dst/tree/a/b.txt")).unwrap(), "deep");
    assert_eq!(std::fs::metadata(d.at("dst/tree/a/b.txt")).unwrap().permissions().mode() & 0o777, 0o640);
    assert!(!Path::new(&d.at("dst/tree/link")).exists());
    assert!(p["skipped"][0].as_str().unwrap().contains("link, not followed"));
    assert!(leftovers(&d.at("dst")).is_empty());
}

#[actix_web::test]
async fn a_file_already_there_is_replaced_only_once_the_new_one_is_whole() {
    let d = Dir::new();
    write(&d.at("src/f.txt"), "new contents");
    write(&d.at("dst/f.txt"), "old");
    let job = Job::for_test();
    run(job.clone(), Fs::Local, Fs::Local, vec![d.at("src/f.txt")], d.at("dst")).await;
    assert_eq!(job.test_progress()["state"], "done");
    assert_eq!(std::fs::read_to_string(d.at("dst/f.txt")).unwrap(), "new contents");
    assert!(leftovers(&d.at("dst")).is_empty());
}

#[actix_web::test]
async fn a_cancelled_copy_writes_nothing_and_a_missing_file_is_noted() {
    let d = Dir::new();
    write(&d.at("src/f.txt"), "x");
    std::fs::create_dir_all(d.at("dst")).unwrap();
    let job = Job::for_test();
    job.test_cancel();
    run(job.clone(), Fs::Local, Fs::Local, vec![d.at("src/f.txt")], d.at("dst")).await;
    assert_eq!(job.test_progress()["state"], "cancelled");
    assert!(walk(Path::new(&d.at("dst"))).is_empty());
    let job = Job::for_test();
    run(job.clone(), Fs::Local, Fs::Local, vec![d.at("src/gone.txt"), d.at("src/f.txt")], d.at("dst")).await;
    let p = job.test_progress();
    assert_eq!((p["state"].as_str(), p["files_done"].as_u64(), p["skipped_count"].as_u64()), (Some("done"), Some(1), Some(1)));
}
