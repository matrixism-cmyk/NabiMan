use super::*;
use nabiman_server::state_snapshot::restore;
use std::path::PathBuf;

const PHASE: &str = "NABIMAN_RESTORE_TEST_PHASE";
const CHILD: &str = "backup::state::tests::restore_child";

struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!("nabiman-restore-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&p).unwrap();
        Self(p)
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The real stores read NABIMAN_DATA_DIR, so each phase runs in a child test process
/// with its own environment; the child leaves a marker so a filter typo cannot pass.
fn child(phase: &str, dir: &Path) {
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([CHILD, "--exact", "--ignored", "--test-threads=1", "--nocapture"])
        .env("NABIMAN_DATA_DIR", dir)
        .env(PHASE, phase)
        // Explicit, so no legacy ./data/mec.db of the working directory is ever adopted.
        .env("NABIMAN_MEC_DB", dir.join("mec.db"))
        .current_dir(std::env::temp_dir())
        .env_remove("NABIMAN_PASSWORD")
        .output()
        .unwrap();
    let marker = dir.join(format!(".restore-test-{phase}"));
    assert!(
        output.status.success() && marker.exists(),
        "{phase} failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::remove_file(marker).unwrap();
}

#[test]
fn a_backup_restores_into_an_empty_directory_with_every_store_readable() {
    let root = Root::new();
    let live = root.0.join("live");
    child("populate", &live);
    let backups = live.join("backups");
    let row = default_schedule(None, None).unwrap().unwrap();
    let options = Options { include_secret_key: row.include_secret_key, mec_db: None };
    let name = create(&live, &backups, &prefix(&row), &options).unwrap();
    // The key stays out of the archive by default; the operator keeps it separately.
    let kept_key = root.0.join("secret.key.offline");
    std::fs::copy(live.join("secret.key"), &kept_key).unwrap();
    let restored = root.0.join("restored");
    let summary = restore(&backups.join(&name), restored.to_str().unwrap(), Some(&kept_key)).unwrap();
    assert_eq!(summary.secret_key, "supplied");
    assert_eq!(summary.encrypted_credentials, 2, "server password and forwarding token");
    for (db, schema) in [
        ("mec.db", &crate::mec::db::schema::SCHEMA),
        ("topology.sqlite3", &crate::terminal::topology::SCHEMA),
        ("claude-events.sqlite3", &crate::terminal::claude_events::SCHEMA),
        ("server-facts.sqlite3", &crate::server_facts::SCHEMA),
        ("audit-forwarding.sqlite3", &crate::audit_forwarding::store::SCHEMA),
    ] {
        assert_eq!(summary.databases.get(db), Some(&schema.current()), "{db}");
    }
    child("verify", &restored);
}

#[test]
#[ignore = "a phase of a_backup_restores_into_an_empty_directory_with_every_store_readable"]
fn restore_child() {
    let dir = nabiman_server::data_dir();
    match std::env::var(PHASE).as_deref() {
        Ok("populate") => populate(),
        Ok("verify") => verify(),
        _ => return,
    }
    std::fs::write(dir.join(format!(".restore-test-{}", std::env::var(PHASE).unwrap())), b"ok").unwrap();
}

fn populate() {
    let dir = nabiman_server::data_dir();
    let storage = crate::data_dir::prepare().unwrap();
    let mec = crate::mec::db::open_owned(crate::mec::db::default_path()).unwrap();
    mec.handle.lock().unwrap().execute("INSERT INTO mec_config(key,value,updated_at) VALUES('restore','kept','t')", []).unwrap();
    storage.seed_defaults();
    let users = crate::users::new_user_store(&bcrypt::hash("restore-test-password", 4).unwrap());
    assert_eq!(users.lock().unwrap().len(), 1);
    crate::auth::new_jwt_secret();
    let mut server: crate::models::RemoteServer = serde_json::from_value(serde_json::json!({
        "id": "srv_restore", "name": "db-1", "host": "10.0.0.5", "port": 22, "user": "root",
        "auth_method": "password", "tags": [], "memo": "", "created_at": "2026-11-10T00:00:00Z"
    }))
    .unwrap();
    server.password_enc = crate::secret_store::encrypt("s3cret-pw").unwrap();
    crate::remote_servers::save_servers(&mut [server]).unwrap();
    let mut forwarding = crate::audit_forwarding::store::Store::open(&dir.join("audit-forwarding.sqlite3")).unwrap();
    forwarding.add("siem", "https://siem.example/in", &crate::secret_store::encrypt("Bearer t").unwrap()).unwrap();
    let doc = crate::terminal::topology::checked(serde_json::from_value(serde_json::json!({
        "devices": [{"key": "fw", "name": "FW", "kind": "firewall"}, {"key": "n1", "name": "N1", "kind": "node"}],
        "links": [{"from": "fw", "to": "n1"}],
        "hosts": [{"device": "n1", "server_host": "10.0.0.5"}]
    })).unwrap())
    .unwrap();
    crate::terminal::topology::replace(&doc).unwrap();
    let now = crate::terminal::session::now_unix();
    let seen = vec![("nb_a".to_string(), "nb_b".to_string(), now - 10, "백업 시험".to_string())];
    crate::terminal::claude_events::record(&seen, &Default::default());
    assert_eq!(crate::terminal::claude_events::texts_since(now - 60), ["백업 시험"]);
    let checked: crate::models::RemoteServerStatus = serde_json::from_value(serde_json::json!({
        "id": "srv_restore", "name": "db-1", "host": "10.0.0.5", "status": "online", "hostname": "db-1",
        "os": "Linux", "uptime": "1d", "cpu_usage": "5%", "memory": "1G", "disk": "10G", "load": "0.1", "checked_at": "t"
    }))
    .unwrap();
    crate::server_facts::record(&[checked]);
    assert!(dir.join("backup_schedules.json").exists(), "a fresh data directory gets the default schedule");
}

fn verify() {
    let dir = nabiman_server::data_dir();
    let storage = crate::data_dir::prepare().unwrap();
    let mec = crate::mec::db::open_owned(crate::mec::db::default_path()).unwrap();
    let kept: String = mec.handle.lock().unwrap().query_row("SELECT value FROM mec_config WHERE key='restore'", [], |r| r.get(0)).unwrap();
    assert_eq!(kept, "kept");
    storage.seed_defaults();
    let users = crate::users::new_user_store("");
    assert_eq!(users.lock().unwrap()[0].username, "admin");
    assert!(dir.join("jwt_secret").exists(), "sign-ins stay valid: the JWT key came back");
    let servers = crate::remote_servers::load_servers().unwrap();
    assert_eq!(servers.len(), 1);
    assert_eq!(crate::remote_servers::server_password(&servers[0]).as_deref(), Some("s3cret-pw"));
    let forwarding = crate::audit_forwarding::store::Store::open(&dir.join("audit-forwarding.sqlite3")).unwrap();
    let destinations = forwarding.list().unwrap();
    assert!(destinations.len() == 1 && destinations[0].has_auth);
    let topology = crate::terminal::topology::current().unwrap();
    assert_eq!(topology.devices.len(), 2);
    assert_eq!(topology.hosts[0].server_host, "10.0.0.5");
    let now = crate::terminal::session::now_unix();
    assert_eq!(crate::terminal::claude_events::texts_since(now - 3600), ["백업 시험"]);
    let rows: Vec<Schedule> = serde_json::from_slice(&std::fs::read(dir.join("backup_schedules.json")).unwrap()).unwrap();
    assert!(rows.iter().any(|r| r.kind == "state"), "the schedule came back and was not seeded twice");
    assert_eq!(rows.len(), 1);
    let facts = rusqlite::Connection::open(dir.join("server-facts.sqlite3")).unwrap();
    let host: String = facts.query_row("SELECT hostname FROM server_facts WHERE id='srv_restore'", [], |r| r.get(0)).unwrap();
    assert_eq!(host, "db-1");
}

#[test]
fn old_snapshots_beyond_keep_are_removed_newest_kept() {
    let root = Root::new();
    for name in ["nabiman-state-aaaa-20261101T031700Z.tar.gz", "nabiman-state-aaaa-20261102T031700Z.tar.gz", "nabiman-state-aaaa-20261103T031700Z.tar.gz", "nabiman-state-bbbb-20261101T031700Z.tar.gz", "other.tar.gz"] {
        std::fs::write(root.0.join(name), b"x").unwrap();
    }
    prune(&root.0, "nabiman-state-aaaa", 2);
    let mut left: Vec<_> = std::fs::read_dir(&root.0).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).collect();
    left.sort();
    assert_eq!(left, ["nabiman-state-aaaa-20261102T031700Z.tar.gz", "nabiman-state-aaaa-20261103T031700Z.tar.gz", "nabiman-state-bbbb-20261101T031700Z.tar.gz", "other.tar.gz"]);
}

#[test]
fn the_default_schedule_is_daily_keeps_a_week_and_can_be_changed_or_switched_off() {
    let row = default_schedule(None, None).unwrap().unwrap();
    assert_eq!((row.cron_expr.as_str(), row.keep, row.kind.as_str(), row.include_secret_key), (DEFAULT_CRON, Some(7), "state", false));
    assert!(default_schedule(Some("off"), None).unwrap().is_none());
    assert_eq!(default_schedule(Some("0 */6 * * *"), Some("28")).unwrap().unwrap().keep, Some(28));
    assert!(default_schedule(Some("every night"), None).is_err());
    assert!(default_schedule(None, Some("0")).is_err());
}

#[test]
fn only_a_fresh_data_directory_gets_the_default_schedule() {
    let root = Root::new();
    assert!(seed_default_schedule(&root.0, false).is_none());
    assert!(!root.0.join("backup_schedules.json").exists());
    assert!(seed_default_schedule(&root.0, true).unwrap().contains("17 3 * * *"));
    let rows: Vec<Schedule> = serde_json::from_slice(&std::fs::read(root.0.join("backup_schedules.json")).unwrap()).unwrap();
    assert_eq!(rows.len(), 1);
    // An existing schedules file (even an empty list someone left) is never replaced.
    std::fs::write(root.0.join("backup_schedules.json"), b"[]").unwrap();
    assert!(seed_default_schedule(&root.0, true).is_none());
    assert_eq!(std::fs::read(root.0.join("backup_schedules.json")).unwrap(), b"[]");
}
