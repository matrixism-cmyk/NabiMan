use super::*;
use rusqlite::Connection;

fn day(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
}
fn ask(from: Option<&str>, to: Option<&str>) -> Ask {
    Ask { from: from.map(Into::into), to: to.map(Into::into) }
}

#[test]
fn a_period_is_the_last_thirty_days_unless_asked_and_both_ends_count() {
    let today = day("2026-11-11");
    assert_eq!(period(&ask(None, None), today).unwrap(), Period { from: "2026-10-13".into(), to: "2026-11-11".into(), days: 30 });
    let october = period(&ask(Some("2026-10-01"), Some("2026-10-31")), today).unwrap();
    assert_eq!((october.days, october.from.as_str()), (31, "2026-10-01"));
    assert_eq!(period(&ask(Some("2026-11-11"), Some("2026-11-11")), today).unwrap().days, 1);
    for (from, to) in [(Some("2026-11-12"), Some("2026-11-11")), (Some("2025-01-01"), None), (Some("11/01/2026"), None), (None, Some("2026-13-01"))] {
        assert!(period(&ask(from, to), today).is_err(), "{from:?} {to:?}");
    }
}

#[test]
fn users_are_counted_by_state_and_active_ones_by_role() {
    let user = |role: &str, status: &str| -> crate::models::NabimanUser {
        serde_json::from_value(serde_json::json!({
            "id": uuid::Uuid::new_v4().to_string(), "username": "u", "password_hash": "", "role": role,
            "created_at": "now", "status": status
        }))
        .unwrap()
    };
    let all = [user("admin", "active"), user("operator", "active"), user("viewer", "pending"), user("remote", "active"), user("viewer", "suspended")];
    let u = users(&all);
    assert_eq!((u.total, u.active, u.pending, u.suspended, u.rejected), (5, 3, 1, 1, 0));
    assert_eq!((u.admins, u.operators, u.viewers, u.remote_members), (1, 1, 0, 1));
    assert_eq!(crate::license::plan::seats(&all), 3);
}

fn memory() -> Connection {
    let db = Connection::open_in_memory().unwrap();
    ledger::SCHEMA.migrate(&db).unwrap();
    db
}

#[test]
fn the_ledger_adds_up_session_seconds_and_keeps_peaks_per_day() {
    let db = memory();
    let s = |seconds, sessions, attached, users, servers| ledger::Sample { seconds, sessions, attached, users, servers };
    ledger::record(&db, "2026-11-01", &s(60, 3, 1, 4, 10)).unwrap();
    ledger::record(&db, "2026-11-01", &s(60, 1, 0, 6, 9)).unwrap();
    // A long stall counts as two minutes, not as hours of use.
    ledger::record(&db, "2026-11-02", &s(36_000, 2, 2, 5, 11)).unwrap();
    ledger::record(&db, "2026-12-01", &s(60, 9, 9, 9, 99)).unwrap();
    let t = ledger::totals(&db, "2026-11-01", "2026-11-30").unwrap();
    assert_eq!(t.sampled_days, 2);
    assert_eq!(t.up_seconds, 60 + 60 + 120);
    assert_eq!(t.session_seconds, 180 + 60 + 240);
    assert_eq!(t.attached_seconds, 60 + 240);
    assert_eq!((t.peak_sessions, t.peak_users, t.peak_servers), (3, 6, 11));
    assert_eq!(t.first_day.as_deref(), Some("2026-11-01"));
    assert_eq!(hours(t.session_seconds), 0.13);
    let empty = ledger::totals(&memory(), "2026-11-01", "2026-11-30").unwrap();
    assert_eq!(empty, ledger::Totals::default());
}

#[test]
fn the_ledger_opens_at_its_current_version_and_refuses_a_newer_one() {
    let dir = std::env::temp_dir().join(format!("nabiman-usage-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(ledger::FILE);
    drop(ledger::open(&path).unwrap());
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
    Connection::open(&path).unwrap().execute_batch("PRAGMA user_version = 99").unwrap();
    assert!(ledger::open(&path).is_err());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn storage_adds_up_files_without_following_links() {
    let dir = std::env::temp_dir().join(format!("nabiman-usage-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(dir.join("backups/2026")).unwrap();
    std::fs::write(dir.join("users.json"), [0u8; 100]).unwrap();
    std::fs::write(dir.join("backups/2026/a.tar"), [0u8; 1000]).unwrap();
    std::fs::write(dir.join("backups/b.tar"), [0u8; 500]).unwrap();
    std::os::unix::fs::symlink("/", dir.join("root-link")).unwrap();
    let s = storage::measure(&dir);
    assert_eq!((s.bytes, s.files, s.complete), (1600, 3, true));
    assert_eq!(s.largest[0], storage::Part { name: "backups".into(), bytes: 1500 });
    assert_eq!(s.largest[1], storage::Part { name: "users.json".into(), bytes: 100 });
    assert!(!storage::measure(&dir.join("missing")).complete);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn the_collector_token_opens_usage_only_when_set_and_exact() {
    use crate::observability::metrics::bearer_setting_in;
    let mut good = actix_web::http::header::HeaderMap::new();
    good.insert(actix_web::http::header::AUTHORIZATION, "Bearer usage-0123456789abcdef".parse().unwrap());
    let set = |name: &str| (name == "NABIMAN_USAGE_TOKEN").then(|| "usage-0123456789abcdef".to_string());
    let metrics_only = |name: &str| (name == "NABIMAN_METRICS_TOKEN").then(|| "usage-0123456789abcdef".to_string());
    assert!(bearer_setting_in(&good, &set, "NABIMAN_USAGE_TOKEN"));
    assert!(!bearer_setting_in(&good, &metrics_only, "NABIMAN_USAGE_TOKEN"), "the metrics token is not the usage token");
    assert!(!bearer_setting_in(&actix_web::http::header::HeaderMap::new(), &set, "NABIMAN_USAGE_TOKEN"));
}
