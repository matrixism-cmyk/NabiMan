use super::*;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!("nabiman-snapshot-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(p.join("data")).unwrap();
        fs::create_dir_all(p.join("out")).unwrap();
        Self(p)
    }
    fn data(&self) -> PathBuf {
        self.0.join("data")
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

const KEY: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

fn ciphertext(key: &str, plain: &str) -> String {
    use aes_gcm::aead::{Aead, KeyInit};
    let cipher = aes_gcm::Aes256Gcm::new_from_slice(&hex::decode(key).unwrap()).unwrap();
    let nonce = [7u8; 12];
    let data = cipher.encrypt(aes_gcm::Nonce::from_slice(&nonce), plain.as_bytes()).unwrap();
    format!("v1:{}:{}", hex::encode(nonce), hex::encode(data))
}

/// A data directory like a running server's: a WAL database with a writer still
/// open and rows not yet checkpointed, credentials, keys, caches, locks, old backups.
fn populate(dir: &Path) -> rusqlite::Connection {
    fs::write(dir.join("users.json"), br#"[{"id":"u1","username":"admin"}]"#).unwrap();
    fs::write(dir.join("remote_servers.json"), format!(r#"[{{"id":"srv_1","password_enc":"{}"}}]"#, ciphertext(KEY, "pw"))).unwrap();
    fs::write(dir.join("secret.key"), KEY).unwrap();
    fs::write(dir.join(".lock"), b"123\n").unwrap();
    fs::write(dir.join("terminal_sessions.lock"), b"").unwrap();
    fs::write(dir.join("tmux.conf"), b"set -g x").unwrap();
    for sub in ["ssh/pw", "ssh-ca", "backups", "preview-cache", "claude-voice"] {
        fs::create_dir_all(dir.join(sub)).unwrap();
    }
    fs::write(dir.join("ssh/id_ed25519"), b"PRIVATE").unwrap();
    fs::write(dir.join("ssh/pw/tmp-pass"), b"plain").unwrap();
    fs::write(dir.join("ssh-ca/ca"), b"CA PRIVATE").unwrap();
    fs::write(dir.join("backups/old.tar.gz"), b"old").unwrap();
    fs::write(dir.join("preview-cache/x.pdf"), b"pdf").unwrap();
    let live = rusqlite::Connection::open(dir.join("topology.sqlite3")).unwrap();
    live.execute_batch("PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0; CREATE TABLE t(v TEXT); PRAGMA user_version=1; INSERT INTO t VALUES('in-wal');").unwrap();
    assert!(dir.join("topology.sqlite3-wal").exists());
    live
}

#[test]
fn a_snapshot_restores_whole_without_the_key_unless_asked() {
    let root = Root::new();
    let _writer = populate(&root.data());
    let name = create(&root.data(), &root.0.join("out"), "nabiman-state", &Options::default()).unwrap();
    let archive = root.0.join("out").join(&name);
    assert_eq!(fs::metadata(&archive).unwrap().permissions().mode() & 0o777, 0o600);

    // Without the key: everything else, the credentials reported as not checkable.
    let bare = root.0.join("bare");
    let restored = restore(&archive, bare.to_str().unwrap(), None).unwrap();
    assert_eq!(restored.secret_key, "missing");
    assert_eq!(restored.encrypted_credentials, 1);
    assert_eq!(restored.databases.get("topology.sqlite3"), Some(&1));
    assert!(!bare.join("secret.key").exists());
    let value: String = rusqlite::Connection::open(bare.join("topology.sqlite3"))
        .unwrap()
        .query_row("SELECT v FROM t", [], |r| r.get(0))
        .unwrap();
    assert_eq!(value, "in-wal", "committed rows still in the WAL are captured");
    assert_eq!(fs::read(bare.join("ssh/id_ed25519")).unwrap(), b"PRIVATE");
    assert_eq!(fs::read(bare.join("ssh-ca/ca")).unwrap(), b"CA PRIVATE");
    assert!(bare.join("claude-voice").is_dir(), "empty state directories come back");
    for gone in [".lock", "terminal_sessions.lock", "tmux.conf", "ssh/pw", "backups", "preview-cache", "topology.sqlite3-wal"] {
        assert!(!bare.join(gone).exists(), "{gone} must not be in a backup");
    }
    assert_eq!(fs::metadata(&bare).unwrap().permissions().mode() & 0o777, 0o700);
    assert_eq!(fs::metadata(bare.join("users.json")).unwrap().permissions().mode() & 0o777, 0o600);

    // With the separately kept key: checked against the stored credential.
    let keyed = root.0.join("keyed");
    let restored = restore(&archive, keyed.to_str().unwrap(), Some(&root.data().join("secret.key"))).unwrap();
    assert_eq!(restored.secret_key, "supplied");
    assert_eq!(fs::read_to_string(keyed.join("secret.key")).unwrap(), KEY);

    // A wrong key is refused before anything is published.
    let wrong = root.0.join("wrong-key");
    fs::write(&wrong, "ff".repeat(32)).unwrap();
    let target = root.0.join("wrong");
    let error = restore(&archive, target.to_str().unwrap(), Some(&wrong)).unwrap_err();
    assert!(error.contains("does not decrypt"), "{error}");
    assert!(!target.exists());

    // An existing directory is never overwritten.
    assert!(restore(&archive, keyed.to_str().unwrap(), None).is_err());

    // A restored directory says where it came from and snapshots again cleanly.
    assert!(keyed.join(RESTORED_FROM).exists() && !keyed.join(MANIFEST).exists());
    let again = create(&keyed, &root.0.join("out"), "again", &Options::default()).unwrap();
    assert!(restore(&root.0.join("out").join(again), root.0.join("again").to_str().unwrap(), None).is_ok());
}

#[test]
fn the_key_is_in_the_archive_only_when_asked_for() {
    let root = Root::new();
    drop(populate(&root.data()));
    let options = Options { include_secret_key: true, mec_db: None };
    let name = create(&root.data(), &root.0.join("out"), "with-key", &options).unwrap();
    let target = root.0.join("restored");
    let restored = restore(&root.0.join("out").join(name), target.to_str().unwrap(), None).unwrap();
    assert_eq!(restored.secret_key, "included");
    assert_eq!(fs::read_to_string(target.join("secret.key")).unwrap(), KEY);
}

#[test]
fn a_mec_database_outside_the_data_directory_is_included_as_mec_db() {
    let root = Root::new();
    let mec = root.0.join("elsewhere.db");
    rusqlite::Connection::open(&mec).unwrap().execute_batch("CREATE TABLE jobs(id); PRAGMA user_version=1;").unwrap();
    fs::write(root.data().join("users.json"), b"[]").unwrap();
    let options = Options { include_secret_key: false, mec_db: Some(mec) };
    let name = create(&root.data(), &root.0.join("out"), "s", &options).unwrap();
    let target = root.0.join("restored");
    let restored = restore(&root.0.join("out").join(name), target.to_str().unwrap(), None).unwrap();
    assert_eq!(restored.databases.get("mec.db"), Some(&1));
}

#[test]
fn a_damaged_snapshot_is_refused() {
    let root = Root::new();
    drop(populate(&root.data()));
    let name = create(&root.data(), &root.0.join("out"), "s", &Options::default()).unwrap();
    // Unpack, alter one file, verify the tree.
    let tree = root.0.join("tree");
    fs::create_dir(&tree).unwrap();
    crate::backup_crypto::restore::extract(fs::File::open(root.0.join("out").join(name)).unwrap(), &tree).unwrap();
    assert!(verify(&tree).is_ok());
    fs::write(tree.join("users.json"), b"[]").unwrap();
    assert!(verify(&tree).unwrap_err().contains("users.json"));
}
