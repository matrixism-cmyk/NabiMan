use super::*;

fn server(id: &str) -> RemoteServer {
    RemoteServer {
        id: id.into(),
        name: "Original".into(),
        host: "example.com".into(),
        port: 22,
        user: "admin".into(),
        auth_method: "key".into(),
        tags: vec![],
        memo: String::new(),
        created_at: String::new(),
        last_checked: String::new(),
        status: "unknown".into(),
        password_enc: String::new(),
        has_password: false,
        host_trust_version: uuid::Uuid::new_v4().to_string(),
        host_trust: None,
        terminal: "ssh".into(),
        files: "sftp".into(),
        ftp_port: 21,
        tls_fingerprint: String::new(),
        ftps_implicit: false,
    }
}

#[test]
fn probe_of_previous_approval_cannot_mark_same_key_reapproval_healthy() {
    let original = server("approval");
    let snapshot = vec![original.clone()];
    let mut latest = vec![original];
    latest[0].host_trust_version = uuid::Uuid::new_v4().to_string();
    merge_probe_results(&mut latest, &snapshot, &[result("approval")]);
    assert_eq!(latest[0].status, "unknown");
    assert!(latest[0].last_checked.is_empty());
}

fn result(id: &str) -> RemoteServerStatus {
    RemoteServerStatus {
        id: id.into(),
        name: "Original".into(),
        host: "example.com".into(),
        status: "online".into(),
        hostname: String::new(),
        os: String::new(),
        uptime: String::new(),
        cpu_usage: String::new(),
        memory: String::new(),
        disk: String::new(),
        load: String::new(),
        checked_at: "2026-09-24T00:00:00Z".into(),
        checks: Vec::new(),
    }
}

#[test]
fn database_errors_are_not_empty_lists() {
    let dir = std::env::temp_dir().join(format!("nabiman-servers-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("servers.json");
    assert!(load_servers_at(&path).unwrap().is_empty());
    fs::write(&path, "{broken").unwrap();
    assert!(load_servers_at(&path).is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), "{broken");
    assert!(load_servers_at(&dir).is_err());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn probe_preserves_edits_additions_and_deletions() {
    let snapshot = vec![server("kept"), server("deleted")];
    let mut latest = vec![snapshot[0].clone(), server("new")];
    latest[0].name = "Renamed".into();
    latest[0].memo = "New memo".into();
    merge_probe_results(&mut latest, &snapshot, &[result("kept"), result("deleted")]);
    assert_eq!(latest.len(), 2);
    assert_eq!(latest[0].name, "Renamed");
    assert_eq!(latest[0].memo, "New memo");
    assert_eq!(latest[0].status, "online");
    assert_eq!(latest[1].id, "new");
    assert_eq!(latest[1].status, "unknown");
}

#[test]
fn probe_of_old_target_cannot_change_new_targets_status() {
    let snapshot = vec![server("changed")];
    let mut latest = snapshot.clone();
    latest[0].host = "new.example.com".into();
    merge_probe_results(&mut latest, &snapshot, &[result("changed")]);
    assert_eq!(latest[0].status, "unknown");
    assert!(latest[0].last_checked.is_empty());
}
