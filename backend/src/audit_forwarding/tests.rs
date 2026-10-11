use super::*;
struct Fixture(std::path::PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("nb-forward-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn path(&self) -> std::path::PathBuf {
        self.0.join("queue.db")
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn entry() -> AuditEntry {
    AuditEntry {
        timestamp: "fixture".into(),
        user: "fixture".into(),
        method: "GET".into(),
        path: "/api/fixture".into(),
        status: 200,
        ip: "127.0.0.1".into(),
        hash: None,
    }
}
fn enabled(s: &mut Store) -> String {
    let id = s
        .add("receiver", "https://example.test/events", "")
        .unwrap();
    let rev = s.list().unwrap()[0].revision.clone();
    s.change(&id, &rev, "enable").unwrap();
    id
}
#[test]
fn urls_refuse_plaintext_credentials_queries_fragments_and_control_characters() {
    for url in [
        "http://example.test",
        "https://user:secret@example.test",
        "https://example.test/?token=secret",
        "https://example.test/#secret",
        "https://example.test/\n",
    ] {
        assert!(validate_url(url).is_err());
    }
    assert_eq!(
        validate_url("https://example.test/events").unwrap(),
        "https://example.test/events"
    );
}
#[test]
fn queue_survives_restart_and_acknowledgement_uses_same_delivery_identity() {
    let f = Fixture::new();
    let mut s = Store::open(&f.path()).unwrap();
    enabled(&mut s);
    s.enqueue(&entry(), None).unwrap();
    let row = s.claim(1).unwrap().unwrap();
    let id = row.id.clone();
    drop(s);
    let mut s = Store::open(&f.path()).unwrap();
    let row = s.claim(2).unwrap().unwrap();
    assert_eq!(row.id, id);
    assert_eq!(row.attempts, 2);
    s.finish(&row, None, 2).unwrap();
    assert!(s.claim(3).unwrap().is_none());
    assert_eq!(s.counters().unwrap()["acknowledged"], 1);
    assert_eq!(s.list().unwrap()[0].acknowledged, 1);
}
#[test]
fn stale_configuration_cannot_enable_or_deliver_cancelled_events() {
    let f = Fixture::new();
    let mut s = Store::open(&f.path()).unwrap();
    let id = enabled(&mut s);
    s.enqueue(&entry(), None).unwrap();
    let row = s.claim(0).unwrap().unwrap();
    let rev = s.list().unwrap()[0].revision.clone();
    s.change(&id, &rev, "disable").unwrap();
    assert!(matches!(
        s.change(&id, &rev, "enable"),
        Err(Error::Conflict)
    ));
    s.finish(&row, None, 1).unwrap();
    assert_eq!(s.counters().unwrap()["acknowledged"], 0);
    assert!(s.claim(1).unwrap().is_none());
    assert_eq!(s.counters().unwrap()["cancelled"], 1);
}
#[test]
fn failures_back_off_then_block_until_explicit_retry() {
    let f = Fixture::new();
    let mut s = Store::open(&f.path()).unwrap();
    let id = enabled(&mut s);
    s.enqueue(&entry(), None).unwrap();
    let mut now = 0;
    let mut original = String::new();
    for attempt in 1..=8 {
        let row = s.claim(now).unwrap().unwrap();
        if attempt == 1 {
            original = row.id.clone();
        }
        assert_eq!(row.id, original);
        s.finish(&row, Some("http_rejected_503"), now).unwrap();
        assert!(s.claim(now).unwrap().is_none());
        now += 4000;
    }
    assert_eq!(s.list().unwrap()[0].blocked, 1);
    let rev = s.list().unwrap()[0].revision.clone();
    s.change(&id, &rev, "retry").unwrap();
    assert_eq!(s.claim(now).unwrap().unwrap().id, original);
}
#[test]
fn failed_queue_transaction_does_not_publish_partial_delivery() {
    let f = Fixture::new();
    let mut s = Store::open(&f.path()).unwrap();
    enabled(&mut s);
    s.db.execute_batch("CREATE TRIGGER reject_queue BEFORE INSERT ON deliveries BEGIN SELECT RAISE(ABORT,'fixture'); END;").unwrap();
    assert!(s.enqueue(&entry(), None).is_err());
    assert!(s.claim(0).unwrap().is_none());
}
#[test]
fn size_limit_is_visible_and_corrupt_or_unknown_database_is_preserved() {
    let f = Fixture::new();
    let mut s = Store::open(&f.path()).unwrap();
    enabled(&mut s);
    let mut large = entry();
    large.path = "x".repeat(17000);
    assert!(matches!(s.enqueue(&large, None), Err(Error::Limit)));
    assert_eq!(s.counters().unwrap()["capture_failed"], 1);
    drop(s);
    let bytes = b"not a database";
    std::fs::write(f.path(), bytes).unwrap();
    assert!(Store::open(&f.path()).is_err());
    assert_eq!(std::fs::read(f.path()).unwrap(), bytes);
}
