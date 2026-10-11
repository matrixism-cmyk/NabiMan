use super::*;
use std::sync::{Arc, Mutex};
#[test]
fn only_numeric_proc_fields_are_exported() {
    let input = b"Name:\tsecret-host-user\nVmRSS:\t123 kB\nMemTotal: 2048 kB\n";
    assert_eq!(kib(input, "VmRSS"), Some(123));
    assert_eq!(kib(input, "Name"), None);
    assert_eq!(kib(b"VmRSS: 123 kB secret-token", "VmRSS"), None);
    assert_eq!(kib(b"VmRSS: secret-token kB", "VmRSS"), None);
}
#[test]
fn inventory_summary_omits_identifiers_credentials_and_raw_errors() {
    let path = std::env::temp_dir().join(format!("nabiman-support-{}.json", uuid::Uuid::new_v4()));
    let private = "CANARY_PRIVATE_HOST_USER_PASSWORD";
    let raw = serde_json::json!([{"id":private,"name":private,"host":private,"port":22,"user":private,"auth_method":"password","password_enc":private,"tags":[private],"memo":private,"created_at":private}]);
    std::fs::write(&path, serde_json::to_vec(&raw).unwrap()).unwrap();
    let summary = serde_json::to_string(&inventory(&path)).unwrap();
    assert!(!summary.contains(private));
    assert!(summary.contains("\"registered\":1"));
    assert!(summary.contains("\"password_configured\":1"));
    std::fs::write(&path, private).unwrap();
    assert_eq!(
        serde_json::to_value(inventory(&path)).unwrap(),
        serde_json::json!({"state":"unavailable","data":null})
    );
    std::fs::write(&path, vec![b'x'; 8 * 1024 * 1024 + 1]).unwrap();
    assert_eq!(
        serde_json::to_value(inventory(&path)).unwrap(),
        serde_json::json!({"state":"too_large","data":null})
    );
    std::fs::remove_file(path).unwrap();
}
#[test]
fn busy_stores_are_not_reported_as_empty_or_healthy() {
    let users = Arc::new(Mutex::new(vec![]));
    let _guard = users.lock().unwrap();
    assert_eq!(
        serde_json::to_value(accounts(&users)).unwrap(),
        serde_json::json!({"state":"busy","data":null})
    );
    let db = crate::mec::db::open_in_memory().unwrap();
    let _guard = db.lock().unwrap();
    assert_eq!(
        serde_json::to_value(jobs(&db)).unwrap(),
        serde_json::json!({"state":"busy","data":null})
    );
}
#[test]
fn job_errors_and_unknown_states_never_leave_the_database_and_sampling_is_explicit() {
    let db = crate::mec::db::open_in_memory().unwrap();
    {
        let connection = db.lock().unwrap();
        for (i, status) in [
            r#"{"state":"failed","error":"CANARY_JOB_TOKEN"}"#,
            r#"{"state":"CANARY_PRIVATE_STATE"}"#,
            "CANARY_CORRUPT_STATUS",
        ]
        .iter()
        .enumerate()
        {
            connection.execute("INSERT INTO jobs (id,kind,status,steps,created_by,started_at) VALUES (?1,'{}',?2,'[]','CANARY_ACTOR','CANARY_TIME')",rusqlite::params![i.to_string(),status]).unwrap();
        }
    }
    let summary = serde_json::to_value(jobs(&db)).unwrap();
    assert_eq!(summary["data"]["failed"], 1);
    assert_eq!(summary["data"]["other"], 2);
    assert!(!summary.to_string().contains("CANARY"));
    {
        let connection = db.lock().unwrap();
        for i in 3..1005 {
            connection.execute("INSERT INTO jobs (id,kind,status,steps,created_by,started_at) VALUES (?1,'{}','{\"state\":\"completed\"}','[]','private','private')",[i.to_string()]).unwrap();
        }
    }
    let summary = serde_json::to_value(jobs(&db)).unwrap();
    assert_eq!(summary["data"]["sampled"], 1000);
    assert_eq!(summary["data"]["older_jobs_excluded"], true);
}

#[test]
fn account_and_terminal_metadata_never_enter_aggregate_sections() {
    let user=serde_json::from_value(serde_json::json!({"id":"CANARY_ID","username":"CANARY_USER","password_hash":"CANARY_PASSWORD","role":"viewer","created_at":"CANARY_TIME","totp_enabled":true,"totp_secret":"CANARY_KEY"})).unwrap();
    let users = Arc::new(Mutex::new(vec![user]));
    let account = serde_json::to_string(&accounts(&users)).unwrap();
    assert!(!account.contains("CANARY"));
    assert!(account.contains("\"active_mfa_configured\":1"));
    let terminal=serde_json::from_value(serde_json::json!({"tmux_name":"CANARY_TERMINAL","owner":"CANARY_OWNER","owner_id":"CANARY_ID","label":"CANARY_LABEL","ssh_cmd":"CANARY_COMMAND","title":"CANARY_TITLE"})).unwrap();
    let terminals = Arc::new(Mutex::new(std::collections::HashMap::from([(
        "CANARY_SESSION".into(),
        terminal,
    )])));
    let summary = serde_json::to_string(&super::terminals(&terminals)).unwrap();
    assert!(!summary.contains("CANARY"));
    assert!(summary.contains("\"remote\":1"));
}
#[test]
fn report_authorization_rechecks_current_privilege_and_session() {
    use crate::{auth, jwt_sessions, models::UserRole};
    let user=serde_json::from_value(serde_json::json!({"id":"admin-id","username":"admin","password_hash":"unused","role":"admin","created_at":"now"})).unwrap();
    let users = Arc::new(Mutex::new(vec![user]));
    let sessions = jwt_sessions::new_session_store();
    let secret = Arc::new("support-fixture-secret".to_string());
    jwt_sessions::register_session(&sessions, "sid", "admin", "admin-id", "fixture");
    let (token, _) = jwt_sessions::create_token_pair(&secret, "admin", "admin", "sid").unwrap();
    let req = actix_web::test::TestRequest::post()
        .uri("/api/diagnostics/support-report")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_srv_request();
    assert!(auth::check_auth(&req, &secret, &sessions, &users) == auth::AuthDecision::Allow);
    assert!(super::super::authorize(req.request(), &users, &sessions).is_ok());
    users.lock().unwrap()[0].role = UserRole::Viewer;
    assert_eq!(
        super::super::authorize(req.request(), &users, &sessions)
            .unwrap_err()
            .status(),
        403
    );
    users.lock().unwrap()[0].role = UserRole::Admin;
    sessions.lock().unwrap().clear();
    assert_eq!(
        super::super::authorize(req.request(), &users, &sessions)
            .unwrap_err()
            .status(),
        401
    );
}
