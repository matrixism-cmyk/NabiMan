use super::*;
fn fail(s: &Shared, source: &str, account: &str, now: Instant) -> Result<(), u64> {
    let mut p = Permit::begin(s, source, now)?;
    p.account("login", account, now)
}
#[test]
fn failures_lock_one_source_out_of_one_account_and_expire() {
    let s = Shared::default();
    let now = Instant::now();
    for _ in 0..5 {
        fail(&s, "attacker", "a", now).unwrap();
    }
    assert_eq!(fail(&s, "attacker", "a", now).unwrap_err(), 300);
    // The same source may still try another account, and the owner elsewhere may sign in.
    fail(&s, "attacker", "b", now).unwrap();
    fail(&s, "owner", "a", now).unwrap();
    fail(&s, "attacker", "a", now + Duration::from_secs(300)).unwrap();
}
#[test]
fn many_sources_meet_a_backoff_that_never_becomes_a_lockout() {
    let s = Shared::default();
    let now = Instant::now();
    for i in 0..ACCOUNT_FREE {
        fail(&s, &format!("bot{i}"), "a", now).unwrap();
    }
    // The eleventh failure in the window waits a second, then two, then four …
    assert_eq!(fail(&s, "bot-x", "a", now).unwrap_err(), 1);
    let mut t = now;
    for wait in [1u64, 2, 4, 8, 16, 32, 60, 60, 60] {
        t += Duration::from_secs(wait);
        fail(&s, &format!("bot-{wait}-{t:?}"), "a", t).unwrap();
    }
    assert_eq!(fail(&s, "new-place", "a", t).unwrap_err(), BACKOFF_CAP, "capped");
    fail(&s, "new-place", "a", t + Duration::from_secs(BACKOFF_CAP)).unwrap();
    // Other accounts are untouched.
    fail(&s, "bot0", "b", t).unwrap();
}
#[test]
fn a_known_good_source_skips_the_account_backoff_and_the_global_cap() {
    let s = Shared::default();
    let now = Instant::now();
    let mut p = Permit::begin(&s, "home", now).unwrap();
    p.account("login", "a", now).unwrap();
    p.success();
    drop(p);
    for i in 0..ACCOUNT_FREE + 3 {
        let _ = fail(&s, &format!("bot{i}"), "a", now);
    }
    assert!(fail(&s, "elsewhere", "a", now).is_err());
    fail(&s, "home", "a", now).unwrap();
    // The global budget runs out for strangers, not for the known source.
    for i in 0..GLOBAL.0 {
        let _ = Permit::begin(&s, &format!("flood{}", i % 50), now);
    }
    assert!(Permit::begin(&s, "stranger", now).is_err());
    assert!(Permit::begin(&s, "home", now).is_ok());
}
#[test]
fn simultaneous_reservations_and_cancellation_never_bypass_limits() {
    let s = Shared::default();
    let now = Instant::now();
    let mut active = vec![];
    for _ in 0..MAX_IN_FLIGHT {
        active.push(Permit::begin(&s, "peer", now).unwrap());
    }
    assert!(Permit::begin(&s, "other", now).is_err());
    active.clear();
    assert_eq!(s.lock().unwrap().in_flight, 0);
    for _ in MAX_IN_FLIGHT..SOURCE.0 {
        drop(Permit::begin(&s, "peer", now).unwrap());
    }
    assert!(Permit::begin(&s, "peer", now).is_err());
}
#[test]
fn successful_attempt_only_removes_its_own_account_reservation() {
    let s = Shared::default();
    let now = Instant::now();
    fail(&s, "p", "u", now).unwrap();
    let mut good = Permit::begin(&s, "p", now).unwrap();
    good.account("login", "u", now).unwrap();
    good.success();
    drop(good);
    let st = s.lock().unwrap();
    assert_eq!(st.buckets[&key("login", "u")].entries.len(), 1);
    assert_eq!(st.buckets[&key("login\0pair", "u\0p")].entries.len(), 1);
    assert_eq!(st.buckets[&key("source", "p")].entries.len(), 2);
}
#[test]
fn full_storage_does_not_evict_unexpired_failures() {
    let s = Shared::default();
    let now = Instant::now();
    for i in 0..CAPACITY {
        s.lock().unwrap().record(key("fixture", &i.to_string()), 300, i as u64, now);
    }
    assert!(Permit::begin(&s, "p", now).is_err());
    assert_eq!(s.lock().unwrap().buckets.len(), CAPACITY);
    assert!(Permit::begin(&s, "p", now + Duration::from_secs(300)).is_ok());
}
