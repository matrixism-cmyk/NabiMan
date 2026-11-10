use super::*;
use rusqlite::Connection;

struct Dirs(PathBuf);
impl Dirs {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("nabiman-mec-location-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("cwd/data")).unwrap();
        fs::create_dir_all(root.join("var/lib/nabiman")).unwrap();
        Self(root)
    }
    fn legacy(&self) -> PathBuf {
        self.0.join("cwd").join(LEGACY_RELATIVE)
    }
    fn target(&self) -> PathBuf {
        self.0.join("var/lib/nabiman/mec.db")
    }
}
impl Drop for Dirs {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn rows(path: &Path) -> Vec<String> {
    let db = Connection::open(path).unwrap();
    let mut q = db.prepare("SELECT value FROM mec_config ORDER BY key").unwrap();
    q.query_map([], |r| r.get(0)).unwrap().collect::<Result<_, _>>().unwrap()
}

fn seed(path: &Path, values: &[&str]) -> Connection {
    let db = Connection::open(path).unwrap();
    crate::mec::db::schema::initialize(&db).unwrap();
    for (i, v) in values.iter().enumerate() {
        db.execute("INSERT INTO mec_config(key,value,updated_at) VALUES(?1,?2,'t')", rusqlite::params![format!("k{i}"), v]).unwrap();
    }
    db
}

#[test]
fn a_legacy_database_is_moved_once_with_its_data() {
    let d = Dirs::new();
    drop(seed(&d.legacy(), &["tenant-a", "tenant-b"]));
    assert_eq!(adopt(&d.legacy(), &d.target()).unwrap(), Adoption::Moved);
    assert!(!d.legacy().exists());
    assert_eq!(rows(&d.target()), ["tenant-a", "tenant-b"]);
    // A second start finds the new file and does nothing.
    assert_eq!(adopt(&d.legacy(), &d.target()).unwrap(), Adoption::NothingToDo);
}

#[test]
fn committed_rows_still_in_the_write_ahead_log_come_along() {
    let d = Dirs::new();
    // A server that died with its WAL not yet folded into the main file.
    let scratch = d.0.join("scratch.db");
    let live = seed(&scratch, &[]);
    live.execute_batch("PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0;").unwrap();
    live.execute("INSERT INTO mec_config(key,value,updated_at) VALUES('w','only-in-wal','t')", []).unwrap();
    fs::copy(&scratch, d.legacy()).unwrap();
    fs::copy(sidecar(&scratch, "-wal"), sidecar(&d.legacy(), "-wal")).unwrap();
    drop(live);
    assert_eq!(adopt(&d.legacy(), &d.target()).unwrap(), Adoption::Moved);
    assert_eq!(rows(&d.target()), ["only-in-wal"]);
    assert!(!sidecar(&d.legacy(), "-wal").exists());
}

#[test]
fn an_existing_new_database_wins_and_the_legacy_one_is_left_alone() {
    let d = Dirs::new();
    drop(seed(&d.legacy(), &["old"]));
    drop(seed(&d.target(), &["new"]));
    assert_eq!(adopt(&d.legacy(), &d.target()).unwrap(), Adoption::LegacyIgnored);
    assert_eq!(rows(&d.legacy()), ["old"]);
    assert_eq!(rows(&d.target()), ["new"]);
}

#[test]
fn nothing_happens_without_a_legacy_database() {
    let d = Dirs::new();
    assert_eq!(adopt(&d.legacy(), &d.target()).unwrap(), Adoption::NothingToDo);
    assert!(!d.target().exists());
}

#[test]
fn a_legacy_database_still_owned_by_a_running_server_is_not_taken() {
    let d = Dirs::new();
    drop(seed(&d.legacy(), &["busy"]));
    let file = fs::OpenOptions::new().read(true).write(true).open(d.legacy()).unwrap();
    let _held = nix::fcntl::Flock::lock(file, nix::fcntl::FlockArg::LockExclusiveNonblock).unwrap();
    let error = adopt(&d.legacy(), &d.target()).unwrap_err();
    assert!(error.to_string().contains("still in use"), "{error}");
    assert!(d.legacy().exists() && !d.target().exists());
}

#[test]
fn the_cross_filesystem_copy_is_a_consistent_private_copy() {
    use std::os::unix::fs::PermissionsExt;
    let d = Dirs::new();
    drop(seed(&d.legacy(), &["copied"]));
    copy_into(&d.legacy(), &d.target()).unwrap();
    assert_eq!(rows(&d.target()), ["copied"]);
    assert_eq!(fs::metadata(d.target()).unwrap().permissions().mode() & 0o777, 0o600);
}
