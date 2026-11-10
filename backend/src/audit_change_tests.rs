use super::*;

#[derive(Serialize)]
struct Account {
    id: String,
    username: String,
    role: String,
    password_hash: String,
    totp_secret: Option<String>,
    last_login: Option<String>,
    memberships: Vec<String>,
}

fn account(role: &str, hash: &str) -> Account {
    Account {
        id: "u1".into(),
        username: "alice".into(),
        role: role.into(),
        password_hash: hash.into(),
        totp_secret: None,
        last_login: Some("now".into()),
        memberships: vec![],
    }
}

#[test]
fn an_update_names_each_changed_field_and_hides_secrets() {
    let before = account("viewer", "$2b$old");
    let mut after = account("operator", "$2b$new");
    after.last_login = Some("later".into());
    after.memberships = vec!["tenant:t1".into()];
    let change = Change::between("user", "u1", Some(&before), Some(&after));
    assert_eq!((change.resource.as_str(), change.id.as_str(), change.action.as_str()), ("user", "u1", "update"));
    let names: Vec<&str> = change.fields.iter().map(|f| f.field.as_str()).collect();
    assert_eq!(names, ["memberships", "password_hash", "role"], "last_login is noise");
    let role = &change.fields[2];
    assert_eq!((role.old.as_deref(), role.new.as_deref()), (Some("viewer"), Some("operator")));
    let hash = &change.fields[1];
    assert!(hash.secret && hash.old.is_none() && hash.new.is_none());
    let json = serde_json::to_string(&change).unwrap();
    assert!(!json.contains("$2b$"), "{json}");
}

#[test]
fn create_and_delete_say_so_compactly() {
    let created = Change::between("user", "u1", None, Some(&account("viewer", "$2b$x")));
    assert_eq!(created.action, "create");
    let names: Vec<&str> = created.fields.iter().map(|f| f.field.as_str()).collect();
    assert_eq!(names, ["id", "password_hash", "role", "username"], "empty fields are left out");
    assert!(created.fields.iter().all(|f| f.old.is_none()));
    let deleted = Change::between("user", "u1", Some(&account("viewer", "$2b$x")), None);
    assert_eq!(deleted.action, "delete");
    let names: Vec<&str> = deleted.fields.iter().map(|f| f.field.as_str()).collect();
    assert_eq!(names, ["role", "username"], "only what names it");
    assert!(deleted.fields.iter().all(|f| f.new.is_none()));
}

#[test]
fn hand_made_changes_skip_the_unchanged_and_hide_secret_values() {
    let change = Change::new("server", "s1", "update")
        .field("port", Some("22"), Some("22"))
        .field("host", Some("a.example"), Some("b.example"))
        .field("password", None, Some("hunter2"));
    assert_eq!(change.fields.len(), 2);
    assert!(change.fields[1].secret && change.fields[1].new.is_none());
    let long = "x".repeat(500);
    let c = Change::between("t", "1", Some(&serde_json::json!({"memo": ""})), Some(&serde_json::json!({ "memo": long })));
    assert!(c.fields[0].new.as_ref().unwrap().chars().count() <= MAX_VALUE + 1);
}

#[actix_web::test]
async fn a_noted_change_reaches_the_audit_record_only_when_the_request_succeeds() {
    use actix_web::{test, web, App, HttpResponse};
    let dir = std::env::temp_dir().join(format!("nabiman-audit-change-{}", uuid::Uuid::new_v4()));
    let log = crate::audit::log_at(dir.join("audit.jsonl"));
    let forwarding = crate::audit_forwarding::Forwarder::disabled_for_test();
    let handler = |status: u16| {
        move |req: actix_web::HttpRequest| async move {
            note(&req, Change::new("firewall_rule", "22/tcp", "create").field("action", None, Some("allow")));
            HttpResponse::build(actix_web::http::StatusCode::from_u16(status).unwrap()).finish()
        }
    };
    let app = test::init_service(
        App::new()
            .wrap(crate::audit_middleware::AuditMiddleware::new(log.clone(), forwarding))
            .route("/api/ok", web::post().to(handler(200)))
            .route("/api/fail", web::post().to(handler(500))),
    )
    .await;
    test::call_service(&app, test::TestRequest::post().uri("/api/ok").to_request()).await;
    test::call_service(&app, test::TestRequest::post().uri("/api/fail").to_request()).await;
    let entries = crate::audit::entries_of(&log);
    assert_eq!(entries.len(), 2);
    let change = entries[0].change.as_ref().expect("recorded");
    assert_eq!((change.resource.as_str(), change.id.as_str()), ("firewall_rule", "22/tcp"));
    assert!(entries[0].hash.as_deref().unwrap().starts_with("v3:"));
    assert!(entries[1].change.is_none(), "a failed request changed nothing");
    let _ = std::fs::remove_dir_all(dir);
}
