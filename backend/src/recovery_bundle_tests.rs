use super::*;
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use age::secrecy::SecretString;
struct Fixture(std::path::PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn fixture() -> Fixture {
    let f = Fixture(std::env::temp_dir().join(format!("nb-recovery-{}", uuid::Uuid::new_v4())));
    fs::create_dir_all(f.0.join("auth")).unwrap();
    fs::create_dir_all(f.0.join("runtime/static")).unwrap();
    fs::create_dir(f.0.join("database")).unwrap();
    fs::write(
        f.0.join("auth/users.json"),
        br#"[{"id":"immutable-account","username":"fixture","role":"admin"}]"#,
    )
    .unwrap();
    fs::write(
        f.0.join("auth/jwt_secret"),
        b"fixture-stable-jwt-key-at-least-32-bytes",
    )
    .unwrap();
    for name in [
        "terminal_sessions.json",
        "terminal_settings.json",
        "terminal_shares.json",
    ] {
        fs::write(f.0.join("auth").join(name), b"{}").unwrap();
    }
    fs::write(f.0.join("auth/secret.key"), hex::encode([7u8; 32])).unwrap();
    let cipher = Aes256Gcm::new_from_slice(&[7u8; 32]).unwrap();
    let ciphertext = cipher
        .encrypt(
            Nonce::from_slice(&[9u8; 12]),
            b"fixture remote secret".as_slice(),
        )
        .unwrap();
    fs::write(f.0.join("auth/remote_servers.json"),serde_json::to_vec(&serde_json::json!([{"id":"server","password_enc":format!("v1:{}:{}",hex::encode([9u8;12]),hex::encode(ciphertext))}])).unwrap()).unwrap();
    fs::write(f.0.join("runtime/nabiman-server"), b"runtime fixture").unwrap();
    fs::write(f.0.join("runtime/static/index.html"), b"fixture UI").unwrap();
    let db = rusqlite::Connection::open(f.0.join("database/mec.db")).unwrap();
    db.execute_batch("CREATE TABLE tenant_profiles(id TEXT);CREATE TABLE jobs(id TEXT);INSERT INTO tenant_profiles VALUES('fixture');").unwrap();
    drop(db);
    manifest(&f.0);
    f
}
fn manifest(root: &Path) {
    let mut found = BTreeSet::new();
    inventory(root, root, &mut found).unwrap();
    let files:BTreeMap<_,_>=found.into_iter().map(|name|{let path=root.join(&name);let mode=if name=="runtime/nabiman-server"{0o700}else{0o600};(name,serde_json::json!({"sha256":digest(&path).unwrap(),"size":fs::metadata(path).unwrap().len(),"mode":mode}))}).collect();
    fs::write(root.join("manifest.json"),serde_json::to_vec(&serde_json::json!({"schema_version":1,"kind":"nabiman-management-state","snapshot_id":uuid::Uuid::new_v4(),"created_at":chrono::Utc::now().to_rfc3339(),"files":files,"excluded":["remote target data"]})).unwrap()).unwrap();
}
#[test]
fn verifies_real_secret_key_and_rejects_missing_or_mismatched_keys() {
    let f = fixture();
    let proof = verify(&f.0).unwrap();
    assert_eq!(proof.accounts, 1);
    assert_eq!(proof.encrypted_credentials_verified, 1);
    fs::write(f.0.join("auth/secret.key"), hex::encode([8u8; 32])).unwrap();
    manifest(&f.0);
    assert!(verify(&f.0).is_err());
    fs::remove_file(f.0.join("auth/secret.key")).unwrap();
    manifest(&f.0);
    assert!(verify(&f.0).is_err());
}
#[test]
fn rejects_hash_change_extras_missing_runtime_and_invalid_database() {
    let f = fixture();
    fs::write(f.0.join("runtime/static/index.html"), b"modified").unwrap();
    assert!(verify(&f.0).is_err());
    manifest(&f.0);
    assert!(verify(&f.0).is_ok());
    fs::write(f.0.join("extra"), b"not manifested").unwrap();
    assert!(verify(&f.0).is_err());
    fs::remove_file(f.0.join("extra")).unwrap();
    fs::write(f.0.join("database/mec.db"), b"corrupt sqlite").unwrap();
    manifest(&f.0);
    assert!(verify(&f.0).is_err());
    fs::remove_file(f.0.join("runtime/nabiman-server")).unwrap();
    manifest(&f.0);
    assert!(verify(&f.0).is_err());
}
#[test]
fn portable_encrypted_bundle_verified_before_atomic_publication() {
    use std::os::unix::fs::PermissionsExt;
    let f = fixture();
    let outside =
        Fixture(std::env::temp_dir().join(format!("nb-recovery-out-{}", uuid::Uuid::new_v4())));
    fs::create_dir(&outside.0).unwrap();
    let secret = || SecretString::from("long fixture recovery passphrase".to_string());
    let name = crate::backup_crypto::encrypt_tree(&outside.0, &f.0, secret()).unwrap();
    let target = outside.0.join("restored");
    crate::backup_crypto::decrypt_validated(
        &outside.0,
        &name,
        target.to_str().unwrap(),
        secret(),
        prepare,
    )
    .unwrap();
    assert_eq!(verify(&target).unwrap().encrypted_credentials_verified, 1);
    assert_eq!(
        fs::metadata(target.join("runtime/nabiman-server"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    fs::write(f.0.join("auth/users.json"), b"modified").unwrap();
    let bad = crate::backup_crypto::encrypt_tree(&outside.0, &f.0, secret()).unwrap();
    let rejected = outside.0.join("rejected");
    assert!(crate::backup_crypto::decrypt_validated(
        &outside.0,
        &bad,
        rejected.to_str().unwrap(),
        secret(),
        prepare
    )
    .is_err());
    assert!(!rejected.exists());
}

#[test]
fn forwarding_only_credentials_also_require_the_restored_key() {
    let f = fixture();
    let servers: serde_json::Value =
        serde_json::from_slice(&fs::read(f.0.join("auth/remote_servers.json")).unwrap()).unwrap();
    let ciphertext = servers[0]["password_enc"].as_str().unwrap();
    let db = rusqlite::Connection::open(f.0.join("auth/audit-forwarding.sqlite3")).unwrap();
    db.execute_batch("CREATE TABLE destinations(auth_enc TEXT NOT NULL);")
        .unwrap();
    db.execute("INSERT INTO destinations VALUES(?1)", [ciphertext])
        .unwrap();
    drop(db);
    fs::write(f.0.join("auth/remote_servers.json"), "[]").unwrap();
    manifest(&f.0);
    assert_eq!(verify(&f.0).unwrap().encrypted_credentials_verified, 1);
    fs::remove_file(f.0.join("auth/secret.key")).unwrap();
    manifest(&f.0);
    assert!(verify(&f.0).is_err());
}
