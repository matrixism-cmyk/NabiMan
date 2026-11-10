//! Every NabiMan database, as files written before versioning existed (user_version 0,
//! tables present, rows in them), opens at the current version with its rows intact.
use super::Schema;
use rusqlite::Connection;

fn version(db: &Connection) -> i64 {
    db.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap()
}

/// `legacy` is the schema exactly as the pre-versioning code created it.
fn migrates_v0_with_rows(schema: &Schema, legacy: &str, insert: &str, count: &str) {
    let dir = std::env::temp_dir().join(format!("nabiman-schema-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(schema.name);
    {
        let db = Connection::open(&path).unwrap();
        db.execute_batch(legacy).unwrap();
        db.execute_batch(insert).unwrap();
        assert_eq!(version(&db), 0);
    }
    let db = Connection::open(&path).unwrap();
    assert_eq!(schema.migrate(&db).unwrap(), 0, "{}", schema.name);
    assert_eq!(version(&db), schema.current());
    assert_eq!(db.query_row(count, [], |r| r.get::<_, i64>(0)).unwrap(), 1, "{} lost rows", schema.name);
    // A fresh file reaches the same version.
    let fresh = Connection::open_in_memory().unwrap();
    schema.migrate(&fresh).unwrap();
    assert_eq!(version(&fresh), schema.current());
    // And a newer release's file is refused.
    db.execute_batch(&format!("PRAGMA user_version = {}", schema.current() + 1)).unwrap();
    assert!(schema.migrate(&db).unwrap_err().to_string().contains("newer NabiMan"));
    drop(db);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn server_facts() {
    migrates_v0_with_rows(
        &crate::server_facts::SCHEMA,
        "CREATE TABLE IF NOT EXISTS server_facts(
           id TEXT PRIMARY KEY, status TEXT NOT NULL, hostname TEXT NOT NULL DEFAULT '', os TEXT NOT NULL DEFAULT '',
           uptime TEXT NOT NULL DEFAULT '', cpu TEXT NOT NULL DEFAULT '', memory TEXT NOT NULL DEFAULT '',
           disk TEXT NOT NULL DEFAULT '', load TEXT NOT NULL DEFAULT '', checked_at TEXT NOT NULL DEFAULT '');",
        "INSERT INTO server_facts(id,status,hostname) VALUES('srv_1','online','web-1');",
        "SELECT count(*) FROM server_facts WHERE hostname='web-1'",
    );
}

#[test]
fn claude_events() {
    migrates_v0_with_rows(
        &crate::terminal::claude_events::SCHEMA,
        "PRAGMA journal_mode=WAL;
         CREATE TABLE IF NOT EXISTS claude_events(
           id INTEGER PRIMARY KEY AUTOINCREMENT, at INTEGER NOT NULL, from_card TEXT NOT NULL, to_card TEXT NOT NULL,
           from_name TEXT NOT NULL DEFAULT '', to_name TEXT NOT NULL DEFAULT '', text TEXT NOT NULL DEFAULT '',
           UNIQUE(at, from_card, to_card, text));
         CREATE INDEX IF NOT EXISTS claude_events_at ON claude_events(at);",
        "INSERT INTO claude_events(at,from_card,to_card,text) VALUES(1,'nb_a','nb_b','보고');",
        "SELECT count(*) FROM claude_events WHERE text='보고'",
    );
}

#[test]
fn topology() {
    migrates_v0_with_rows(
        &crate::terminal::topology::SCHEMA,
        "CREATE TABLE IF NOT EXISTS topo_devices(key TEXT PRIMARY KEY, name TEXT NOT NULL, kind TEXT NOT NULL, address TEXT NOT NULL DEFAULT '', note TEXT NOT NULL DEFAULT '', pos INTEGER NOT NULL DEFAULT 0);
         CREATE TABLE IF NOT EXISTS topo_links(from_key TEXT NOT NULL, to_key TEXT NOT NULL, label TEXT NOT NULL DEFAULT '', pos INTEGER NOT NULL DEFAULT 0, PRIMARY KEY(from_key, to_key));
         CREATE TABLE IF NOT EXISTS topo_hosts(server_host TEXT PRIMARY KEY, device_key TEXT NOT NULL);",
        "INSERT INTO topo_devices(key,name,kind) VALUES('fw','FW','firewall');",
        "SELECT count(*) FROM topo_devices WHERE key='fw'",
    );
}

#[test]
fn audit_forwarding() {
    migrates_v0_with_rows(
        &crate::audit_forwarding::store::SCHEMA,
        "CREATE TABLE IF NOT EXISTS forwarding_schema(version INTEGER NOT NULL);
         INSERT INTO forwarding_schema SELECT 1 WHERE NOT EXISTS(SELECT 1 FROM forwarding_schema);
         CREATE TABLE IF NOT EXISTS destinations(id TEXT PRIMARY KEY,revision TEXT NOT NULL,name TEXT NOT NULL,url TEXT NOT NULL,enabled INTEGER NOT NULL CHECK(enabled IN(0,1)),auth_enc TEXT NOT NULL,acknowledged INTEGER NOT NULL DEFAULT 0,last_ack_at TEXT,last_error TEXT);
         CREATE TABLE IF NOT EXISTS deliveries(id TEXT PRIMARY KEY,destination TEXT NOT NULL,revision TEXT NOT NULL,payload TEXT NOT NULL,probe INTEGER NOT NULL CHECK(probe IN(0,1)),state TEXT NOT NULL CHECK(state IN('pending','sending','blocked')),attempts INTEGER NOT NULL DEFAULT 0 CHECK(attempts>=0),next_attempt INTEGER NOT NULL DEFAULT 0,last_error TEXT,FOREIGN KEY(destination) REFERENCES destinations(id) ON DELETE CASCADE);
         CREATE INDEX IF NOT EXISTS forwarding_due ON deliveries(state,next_attempt);
         CREATE TABLE IF NOT EXISTS forwarding_counters(name TEXT PRIMARY KEY,value INTEGER NOT NULL);
         INSERT OR IGNORE INTO forwarding_counters VALUES('acknowledged',0),('capture_failed',0),('cancelled',0);",
        "INSERT INTO destinations(id,revision,name,url,enabled,auth_enc) VALUES('d1','r1','siem','https://siem.example/in',1,'');",
        "SELECT count(*) FROM destinations WHERE name='siem' AND (SELECT count(*) FROM forwarding_schema)=1",
    );
}

#[test]
fn audit_forwarding_store_opens_a_pre_versioning_file() {
    let dir = std::env::temp_dir().join(format!("nabiman-schema-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("audit-forwarding.sqlite3");
    drop(crate::audit_forwarding::store::Store::open(&path).unwrap());
    Connection::open(&path).unwrap().execute_batch("PRAGMA user_version = 0").unwrap();
    let store = crate::audit_forwarding::store::Store::open(&path).unwrap();
    assert_eq!(version(&store.db), 1);
    drop(store);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn mec() {
    // The pre-versioning schema is the v1 baseline itself (all CREATE ... IF NOT EXISTS).
    let legacy = crate::mec::db::schema::LEGACY_SQL;
    migrates_v0_with_rows(
        &crate::mec::db::schema::SCHEMA,
        legacy,
        "INSERT INTO mec_config(key,value,updated_at) VALUES('k','v','t');",
        "SELECT count(*) FROM mec_config WHERE key='k'",
    );
}

#[test]
fn mec_open_refuses_a_newer_database_with_a_clear_message() {
    let dir = std::env::temp_dir().join(format!("nabiman-schema-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("mec.db");
    drop(crate::mec::db::open_persistent(path.clone()).unwrap());
    Connection::open(&path).unwrap().execute_batch("PRAGMA user_version = 99").unwrap();
    let error = crate::mec::db::open_persistent(path).unwrap_err().to_string();
    assert!(error.contains("mec.db has schema version 99"), "{error}");
    std::fs::remove_dir_all(dir).unwrap();
}
