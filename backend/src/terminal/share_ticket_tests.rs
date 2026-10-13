use super::*;
fn link() -> ShareLink {
    ShareLink {
        token: "link-capability".into(),
        session_id: "terminal-one".into(),
        owner: "owner".into(),
        owner_id: "owner-id".into(),
        owner_authorization_version: "fixture-version".into(),
        label: "fixture".into(),
        created_unix: 1,
        expires_unix: 3601,
        password_hash: Some("fixture-hash".into()),
        read_only: true,
        last_used_unix: 0,
        needs_password: false,
        alive: true,
        usable: true,
        files: Default::default(),
    }
}
#[test]
fn opaque_admission_is_single_use_and_stored_only_by_digest() {
    let mut store = Tickets::default();
    let now = Instant::now();
    let value = store
        .issue(
            link(),
            "cols=80&rows=24".into(),
            Some("https://example.test".into()),
            now,
            TTL,
        )
        .unwrap();
    assert_eq!(value.len(), 64);
    assert!(!store.records.contains_key(&value));
    assert!(store
        .consume(
            &value,
            &link().token,
            "cols=80&rows=24",
            Some("https://example.test"),
            now
        )
        .is_some());
    assert!(store
        .consume(
            &value,
            &link().token,
            "cols=80&rows=24",
            Some("https://example.test"),
            now
        )
        .is_none());
    assert!(store
        .consume("12.legacy-signed-digest", &link().token, "", None, now)
        .is_none());
}
#[test]
fn changed_query_link_or_origin_consumes_admission_without_authorizing() {
    for (target, query, origin) in [
        ("other-link", "cols=80", Some("https://example.test")),
        ("link-capability", "cols=81", Some("https://example.test")),
        ("link-capability", "cols=80", Some("https://other.test")),
        ("link-capability", "cols=80", None),
    ] {
        let mut store = Tickets::default();
        let now = Instant::now();
        let value = store
            .issue(
                link(),
                "cols=80".into(),
                Some("https://example.test".into()),
                now,
                TTL,
            )
            .unwrap();
        assert!(store.consume(&value, target, query, origin, now).is_none());
        assert!(store
            .consume(
                &value,
                &link().token,
                "cols=80",
                Some("https://example.test"),
                now
            )
            .is_none());
    }
}
#[test]
fn expiry_is_strict_monotonic_and_never_more_than_thirty_seconds() {
    let mut store = Tickets::default();
    let now = Instant::now();
    let value = store
        .issue(link(), "".into(), None, now, Duration::from_secs(600))
        .unwrap();
    assert!(store
        .consume(&value, &link().token, "", None, now + TTL)
        .is_none());
    let value = store
        .issue(link(), "".into(), None, now, Duration::from_secs(1))
        .unwrap();
    assert!(store
        .consume(
            &value,
            &link().token,
            "",
            None,
            now + Duration::from_secs(1)
        )
        .is_none());
    let value = store.issue(link(), "".into(), None, now, TTL).unwrap();
    assert!(store
        .consume(
            &value,
            &link().token,
            "",
            None,
            now + TTL - Duration::from_millis(1)
        )
        .is_some());
}
#[test]
fn concurrent_redemptions_authorize_exactly_one_caller() {
    let store = Arc::new(Mutex::new(Tickets::default()));
    let now = Instant::now();
    let value = store
        .lock()
        .unwrap()
        .issue(link(), "".into(), None, now, TTL)
        .unwrap();
    let threads: Vec<_> = (0..16)
        .map(|_| {
            let store = store.clone();
            let value = value.clone();
            std::thread::spawn(move || {
                store
                    .lock()
                    .unwrap()
                    .consume(&value, &link().token, "", None, now)
                    .is_some()
            })
        })
        .collect();
    assert_eq!(
        threads
            .into_iter()
            .filter_map(|t| t.join().ok())
            .filter(|v| *v)
            .count(),
        1
    );
}
#[test]
fn pending_limits_do_not_evict_other_live_proofs_and_expired_capacity_recovers() {
    let mut store = Tickets::default();
    let now = Instant::now();
    let mut first = String::new();
    for i in 0..PER_LINK {
        let value = store.issue(link(), "".into(), None, now, TTL).unwrap();
        if i == 0 {
            first = value;
        }
    }
    assert_eq!(
        store.issue(link(), "".into(), None, now, TTL),
        Err(IssueError::Capacity)
    );
    let mut other = link();
    other.token = "other-link".into();
    assert!(store.issue(other, "".into(), None, now, TTL).is_ok());
    assert!(store
        .consume(&first, &link().token, "", None, now)
        .is_some());
    assert!(store.issue(link(), "".into(), None, now + TTL, TTL).is_ok());
    assert_eq!(store.records.len(), 1);
}
#[test]
fn query_rejects_duplicate_or_unbounded_geometry_and_any_authentication_fields() {
    assert!(valid_query(""));
    assert!(valid_query("rows=24&cols=80"));
    for invalid in [
        "cols=80&cols=81",
        "rows=0",
        "cols=513",
        "cols=-1",
        "cols=%38%30",
        "password=pw",
        "token=jwt",
        "ticket=old",
        "cols=80&",
        "rows=24\n",
        "unknown=x",
    ] {
        assert!(!valid_query(invalid), "{invalid}");
    }
}
#[test]
fn link_and_peer_limits_are_independent_and_reset_after_window() {
    let mut attempts = Attempts::default();
    let now = Instant::now();
    for _ in 0..LINK_RATE {
        assert!(attempts.allow("one", "peer", now));
    }
    assert!(!attempts.allow("one", "different-peer", now));
    for _ in 0..LINK_RATE {
        assert!(attempts.allow("two", "peer", now));
    }
    assert!(!attempts.allow("new-link", "peer", now));
    assert!(attempts.allow("new-link", "different-peer", now));
    assert!(attempts.allow("one", "peer", now + WINDOW));
}
#[test]
fn rotating_peer_and_link_cannot_bypass_process_limit() {
    let mut attempts = Attempts::default();
    let now = Instant::now();
    for i in 0..GLOBAL_RATE {
        assert!(attempts.allow(&format!("link-{i}"), &format!("peer-{i}"), now));
    }
    assert!(!attempts.allow("new-link", "new-peer", now));
    assert!(attempts.allow("new-link", "new-peer", now + WINDOW));
}
#[test]
fn duplicate_ticket_fields_and_unknown_parameters_never_redeem() {
    use actix_web::test::TestRequest;
    let store = new_store();
    let now = Instant::now();
    let value = store
        .tickets
        .lock()
        .unwrap()
        .issue(link(), "".into(), None, now, TTL)
        .unwrap();
    let req = TestRequest::get()
        .uri(&format!("/api/share/link/ws?ticket={value}&ticket={value}"))
        .to_http_request();
    assert!(redeem(&req, &link().token, &store).is_none());
    let req = TestRequest::get()
        .uri(&format!("/api/share/link/ws?token=jwt&ticket={value}"))
        .to_http_request();
    assert!(redeem(&req, &link().token, &store).is_none());
    let req = TestRequest::get()
        .uri(&format!("/api/share/link/ws?ticket={value}"))
        .to_http_request();
    assert!(redeem(&req, &link().token, &store).is_none());
}

#[test]
fn live_connection_leases_bound_each_link_and_global_use_and_release_on_drop() {
    let store = new_store();
    let mut leases = Vec::new();
    for _ in 0..PER_LINK_CONNECTIONS {
        leases.push(reserve(&store, "one").unwrap());
    }
    assert!(!store.has_room("one"));
    assert!(reserve(&store, "one").is_none());
    leases.pop();
    assert!(store.has_room("one"));
    leases.push(reserve(&store, "one").unwrap());
    for i in PER_LINK_CONNECTIONS..GLOBAL_CONNECTIONS {
        leases.push(reserve(&store, &format!("link-{i}")).unwrap());
    }
    assert!(!store.has_room("unrelated"));
    assert!(reserve(&store, "unrelated").is_none());
    leases.clear();
    assert!(store.connections.lock().unwrap().is_empty());
    assert!(reserve(&store, "one").is_some());
}

#[test]
fn parallel_reservations_cannot_exceed_link_connection_limit() {
    let store = new_store();
    let barrier = Arc::new(std::sync::Barrier::new(32));
    let threads: Vec<_> = (0..32)
        .map(|_| {
            let store = store.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                reserve(&store, "one")
            })
        })
        .collect();
    let leases: Vec<_> = threads
        .into_iter()
        .filter_map(|t| t.join().unwrap())
        .collect();
    assert_eq!(leases.len(), PER_LINK_CONNECTIONS);
    drop(leases);
    assert!(store.has_room("one"));
}
