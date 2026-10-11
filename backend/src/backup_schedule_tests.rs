use super::*;
use chrono::TimeZone;
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!("nb-backup-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&p).unwrap();
        Self(p)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn row() -> Schedule {
    Schedule {
        id: "s1".into(),
        name: "fixture".into(),
        paths: vec!["/missing-fixture".into()],
        cron_expr: "* * * * *".into(),
        compression: true,
        enabled: true,
        last_run: None,
        next_run: None,
        last_result: None,
        last_slot: None,
    }
}
#[test]
fn cron_ranges_steps_sunday_and_day_or_semantics() {
    let sunday = chrono::Utc.with_ymd_and_hms(2026, 10, 4, 2, 30, 0).unwrap();
    for value in ["*/15 1-3 * * 0", "0,30 2 * * 7", "30 2 31 * 0"] {
        assert!(Cron::parse(value).unwrap().matches(sunday));
    }
    assert!(!Cron::parse("30 2 31 * *").unwrap().matches(sunday));
    for value in [
        "60 * * * *",
        "* 24 * * *",
        "* * 0 * *",
        "*/0 * * * *",
        "1/2 * * * *",
        "* * * * 8",
        "* * * * * command",
        "-1 * * * *",
        "* * * * MON",
    ] {
        assert!(Cron::parse(value).is_err(), "{value}");
    }
}
#[test]
fn malformed_store_preserved_and_write_failure_disables_scheduler() {
    let f = Fixture::new();
    let path = f.0.join("schedules.json");
    std::fs::write(&path, b"broken").unwrap();
    let mut state = Store::load(path.clone());
    assert!(state.unavailable);
    assert!(state.commit(vec![row()]).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"broken");
    let bad = f.0.join("missing/rows.json");
    let mut state = Store::load(bad);
    assert!(state.commit(vec![row()]).is_err());
    assert!(state.rows.is_empty() && state.unavailable);
}
#[test]
fn durable_claim_single_worker_slot_dedupe_and_restart_interrupted() {
    let f = Fixture::new();
    let path = f.0.join("rows.json");
    save(&path, &[row()]).unwrap();
    let store = Arc::new(Mutex::new(Store::load(path.clone())));
    claim(&store, "s1", Some(42)).unwrap();
    assert!(claim(&store, "s1", Some(42)).is_err());
    assert!(claim(&store, "s1", None).is_err());
    let recovered = Arc::new(Mutex::new(Store::load(path.clone())));
    assert_eq!(
        recovered.lock().unwrap().rows[0]
            .last_result
            .as_ref()
            .unwrap()
            .state,
        "interrupted"
    );
    assert!(claim(&recovered, "s1", Some(42)).is_err());
    assert!(claim(&recovered, "s1", Some(43)).is_ok());
    assert!(
        serde_json::from_slice::<Vec<Schedule>>(&std::fs::read(path).unwrap()).unwrap()[0]
            .last_run
            .is_none()
    );
}
#[test]
fn archive_roundtrip_private_permissions_and_failure_no_artifact() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    let source = f.0.join("source");
    std::fs::create_dir(&source).unwrap();
    std::fs::write(
        source.join("--checkpoint-action=exec=invalid"),
        b"fixture contents",
    )
    .unwrap();
    let dir = f.0.join("backups");
    let paths = vec![source.to_str().unwrap().into()];
    for compressed in [false, true] {
        let filename = archive::create(&dir, "fixture", &paths, compressed).unwrap();
        assert_eq!(
            std::fs::metadata(dir.join(&filename))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert!(filename.ends_with(if compressed { ".tar.gz" } else { ".tar" }));
        let result = std::process::Command::new("tar")
            .arg(if compressed { "-xOzf" } else { "-xOf" })
            .arg(dir.join(filename))
            .output()
            .unwrap();
        assert!(result.status.success());
        assert_eq!(result.stdout, b"fixture contents");
    }
    let before = std::fs::read_dir(&dir).unwrap().count();
    assert!(archive::create(
        &dir,
        "fixture",
        &["/nb-does-not-exist-fixture".into()],
        true
    )
    .is_err());
    assert!(archive::create(&dir, "fixture", &[f.0.to_str().unwrap().into()], true).is_err());
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), before);
    for paths in [
        vec!["--checkpoint=1".into()],
        vec!["/tmp/../etc".into()],
        vec![],
    ] {
        assert!(archive::validate("fixture", &paths).is_err());
    }
    assert!(archive::validate("../escape", &paths).is_err());
}
#[actix_web::test]
async fn schedule_request_defaults_and_forged_run_state_rejected() {
    use actix_web::{test, App};
    let f = Fixture::new();
    let store = Arc::new(Mutex::new(Store::load(f.0.join("rows.json"))));
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(store.clone()))
            .route("/", web::post().to(create)),
    )
    .await;
    let body = serde_json::json!({"name":"fixture","paths":["/tmp/fixture"],"cron_expr":"0 2 * * *","compression":true});
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/")
            .set_json(&body)
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), 201);
    let state = store.lock().unwrap();
    assert!(state.rows[0].enabled);
    assert!(state.rows[0].last_result.is_none());
    drop(state);
    let mut forged = body;
    forged["last_result"] = serde_json::json!({"state":"succeeded"});
    assert_eq!(
        test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/")
                .set_json(forged)
                .to_request()
        )
        .await
        .status(),
        400
    );
}
