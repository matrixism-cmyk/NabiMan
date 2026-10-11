use super::*;
#[test]
fn concurrent_records_have_individual_lines_one_chain_and_matching_memory_order() {
    let dir = std::env::temp_dir().join(format!("nabiman-audit-append-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("audit.jsonl");
    let store = Arc::new(Mutex::new(storage::State::load(path.clone())));
    let barrier = Arc::new(std::sync::Barrier::new(16));
    let threads: Vec<_> = (0..16)
        .map(|i| {
            let store = store.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                for j in 0..8 {
                    store
                        .lock()
                        .unwrap()
                        .append(AuditEntry {
                            timestamp: format!("{i}:{j}"),
                            user: format!("fixture{i}"),
                            method: "GET".into(),
                            path: "/api/fixture".into(),
                            status: 200,
                            ip: "127.0.0.1".into(),
                            hash: None,
                        })
                        .unwrap();
                }
            })
        })
        .collect();
    for t in threads {
        t.join().unwrap();
    }
    let bytes = std::fs::read_to_string(&path).unwrap();
    let mut previous = "0".to_owned();
    let log = store.lock().unwrap();
    let mut seen = std::collections::HashSet::new();
    for (i, line) in bytes.lines().enumerate() {
        let entry: AuditEntry = serde_json::from_str(line).unwrap();
        let expected = compute_hash(&previous, &entry);
        assert_eq!(entry.hash.as_deref(), Some(expected.as_str()));
        assert_eq!(log.entries[i].hash, entry.hash);
        assert!(seen.insert(entry.timestamp));
        previous = expected;
    }
    assert_eq!(seen.len(), 128);
    assert_eq!(log.entries.len(), 128);
    drop(log);
    std::fs::remove_dir_all(dir).unwrap();
}

fn entry() -> AuditEntry {
    AuditEntry {
        timestamp: "fixture".into(),
        user: "operator".into(),
        method: "GET".into(),
        path: "/api/status".into(),
        status: 200,
        ip: "127.0.0.1".into(),
        hash: None,
    }
}
#[test]
fn failed_persistence_freezes_writer_and_does_not_claim_memory_success() {
    let dir = Temp::new();
    let path = dir.path().join("audit.jsonl");
    let mut state = storage::State::load(path.clone());
    state.append(entry()).unwrap();
    let original = std::fs::read(&path).unwrap();
    std::fs::write(&path, b"external change\n").unwrap();
    assert_eq!(state.append(entry()).err().unwrap(), "file_changed");
    assert_eq!(state.entries.len(), 1);
    std::fs::write(&path, original).unwrap();
    assert!(state.append(entry()).is_err()); // Explicit restart/review, not silent retry.
    let mut restarted = storage::State::load(path);
    restarted.append(entry()).unwrap();
    assert_eq!(restarted.entries.len(), 2);
}
#[test]
fn unreadable_special_and_partial_records_are_not_healthy_empty_logs() {
    let dir = Temp::new();
    let path = dir.path().join("audit.jsonl");
    std::fs::create_dir(&path).unwrap();
    assert!(storage::State::load(path.clone()).error.is_some());
    std::fs::remove_dir(&path).unwrap();
    std::fs::write(&path, b"{}").unwrap();
    assert_eq!(
        storage::State::load(path.clone()).error,
        Some("incomplete_record")
    );
    std::fs::remove_file(&path).unwrap();
    std::os::unix::fs::symlink("/dev/null", &path).unwrap();
    assert!(storage::read(&path).is_err());
}
#[test]
fn inspection_reports_legacy_gaps_tamper_and_truncation_without_repair() {
    let dir = Temp::new();
    let path = dir.path().join("audit.jsonl");
    let mut state = storage::State::load(path.clone());
    state.append(entry()).unwrap();
    let bytes = std::fs::read(path).unwrap();
    assert!(inspection::inspect("audit.jsonl", &bytes).valid);
    let mut changed = String::from_utf8(bytes.clone())
        .unwrap()
        .replace("operator", "attacker")
        .into_bytes();
    let result = inspection::inspect("audit.jsonl", &changed);
    assert_eq!(result.hash_mismatches, 1);
    assert!(!result.valid);
    changed.extend_from_slice(b"not-json\n");
    assert_eq!(
        inspection::inspect("audit.jsonl", &changed).malformed_lines,
        1
    );
    assert!(inspection::inspect("audit.jsonl", &bytes[..bytes.len() - 1]).truncated);
    let legacy = serde_json::to_string(&entry()).unwrap() + "\n";
    assert_eq!(
        inspection::inspect("audit.jsonl", legacy.as_bytes()).unhashed_entries,
        1
    );
}
#[test]
fn global_audit_is_denied_to_viewers_operators_and_path_lookalikes_do_not_override_rules() {
    for role in [
        crate::models::UserRole::Viewer,
        crate::models::UserRole::Operator,
    ] {
        for endpoint in [
            "/api/audit",
            "/api/audit/health",
            "/api/audit/verify",
            "/api/audit/export",
            "/api/audit/forwarding",
            "/api/audit/clear",
        ] {
            for method in ["GET", "POST", "DELETE"] {
                assert!(!crate::rbac::check_permission(&role, method, endpoint));
            }
        }
    }
}

struct Temp(std::path::PathBuf);
impl Temp {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("nabiman-audit-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn path(&self) -> &std::path::Path {
        &self.0
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[test]
fn historical_malformed_records_are_preserved_and_reported_while_new_records_persist() {
    let dir = Temp::new();
    let path = dir.path().join("audit.jsonl");
    std::fs::write(&path, b"historical malformed line\n").unwrap();
    let mut state = storage::State::load(path.clone());
    assert_eq!(state.unparsed_lines, 1);
    state.append(entry()).unwrap();
    let bytes = std::fs::read(path).unwrap();
    assert!(bytes.starts_with(b"historical malformed line\n"));
    let result = inspection::inspect("audit.jsonl", &bytes);
    assert!(!result.valid);
    assert_eq!(result.malformed_lines, 1);
    assert_eq!(result.hash_mismatches, 0);
}
#[test]
fn rotation_is_persisted_and_each_retained_legacy_segment_is_verified_separately() {
    use std::os::unix::fs::PermissionsExt;
    let dir = Temp::new();
    let path = dir.path().join("audit.jsonl");
    let mut state = storage::State::load(path.clone());
    let mut large = entry();
    large.path = "x".repeat(10_000_001);
    state.append(large).unwrap();
    assert!(!path.exists());
    let backup = path.with_extension("jsonl.1");
    assert!(inspection::inspect("audit.jsonl.1", &std::fs::read(&backup).unwrap()).valid);
    state.append(entry()).unwrap();
    assert!(inspection::inspect("audit.jsonl", &std::fs::read(&path).unwrap()).valid);
    assert_eq!(
        std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

#[test]
fn versioned_hashes_frame_fields_and_can_follow_legacy_records() {
    let mut a = entry();
    let mut b = entry();
    a.user = "ab".into();
    a.method = "c".into();
    b.user = "a".into();
    b.method = "bc".into();
    assert_eq!(compute_hash("0", &a), compute_hash("0", &b));
    a.hash = Some("v2:".into());
    b.hash = Some("v2:".into());
    assert_ne!(compute_hash("0", &a), compute_hash("0", &b));
    let dir = Temp::new();
    let path = dir.path().join("audit.jsonl");
    let mut legacy = entry();
    legacy.hash = Some(compute_hash("0", &legacy));
    std::fs::write(&path, serde_json::to_string(&legacy).unwrap() + "\n").unwrap();
    let mut state = storage::State::load(path.clone());
    assert!(state
        .append(entry())
        .unwrap()
        .hash
        .unwrap()
        .starts_with("v2:"));
    assert!(inspection::inspect("audit.jsonl", &std::fs::read(path).unwrap()).valid);
}
