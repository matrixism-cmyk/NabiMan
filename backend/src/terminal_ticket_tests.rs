use super::*;
use crate::models::{NabimanUser, UserRole};
use crate::{jwt_sessions::SessionStore, stream_access::ConnectionGrant, AuthCheck};
use actix_web::{test as awtest, App};

fn claims() -> auth::Claims {
    auth::Claims {
        sub: "root-test".into(),
        role: "admin".into(),
        sid: "test-session".into(),
        iat: chrono::Utc::now().timestamp() as usize,
        exp: (chrono::Utc::now().timestamp() + 300) as usize,
    }
}
fn user() -> NabimanUser {
    NabimanUser {
        share_authorization_version: uuid::Uuid::new_v4().to_string(),
        id: "root-test-id".into(),
        username: "root-test".into(),
        password_hash: "unused-fixture".into(),
        role: UserRole::Admin,
        status: Default::default(),
        memberships: vec![],
        requested_memberships: vec![],
        scope_restricted: false,
        created_at: "now".into(),
        last_login: None,
        totp_enabled: false,
        totp_secret: None,
        mfa: Default::default(),
    }
}
#[test]
fn ticket_is_opaque_single_use_and_target_bound() {
    let mut store = Tickets::default();
    let now = Instant::now();
    let token = store
        .issue(claims(), "server_id=one&history=1".into(), None, now, TTL)
        .unwrap();
    assert_eq!(token.len(), 64);
    assert!(!store.records.contains_key(&token));
    assert!(store
        .consume(&token, "server_id=one&history=1", None, now)
        .is_some());
    assert!(store
        .consume(&token, "server_id=one&history=1", None, now)
        .is_none());
    let token = store
        .issue(claims(), "server_id=one".into(), None, now, TTL)
        .unwrap();
    assert!(store.consume(&token, "server_id=two", None, now).is_none());
    assert!(store.consume(&token, "server_id=one", None, now).is_none());
}
#[test]
fn expired_ticket_and_changed_origin_are_rejected() {
    let mut store = Tickets::default();
    let now = Instant::now();
    let token = store
        .issue(
            claims(),
            "".into(),
            Some("https://test.example".into()),
            now,
            TTL,
        )
        .unwrap();
    assert!(store
        .consume(&token, "", Some("https://test.example"), now + TTL)
        .is_none());
    for origin in [None, Some("https://other.example")] {
        let token = store
            .issue(
                claims(),
                "".into(),
                Some("https://test.example".into()),
                now,
                TTL,
            )
            .unwrap();
        assert!(store.consume(&token, "", origin, now).is_none());
    }
}
#[test]
fn query_validation_rejects_credentials_duplicates_and_unknown_parameters() {
    for q in [
        "token=x",
        "password=x",
        "ticket=x",
        "server_id=one&server_id=two",
        "server_id=one&&history=1",
        "unknown=1",
        "cols=20\n",
        "ssh_host=나비",
    ] {
        assert!(!valid_query(q), "{q}");
    }
    assert!(valid_query(
        "ssh_host=127.0.0.1&ssh_port=22&ssh_user=root&history=1"
    ));
}
#[test]
fn session_limit_expires_without_eviction_of_valid_tickets() {
    let mut s = Tickets::default();
    let now = Instant::now();
    for _ in 0..PER_SESSION {
        s.issue(claims(), "".into(), None, now, TTL).unwrap();
    }
    assert_eq!(
        s.issue(claims(), "".into(), None, now, TTL).unwrap_err(),
        IssueError::Capacity
    );
    assert_eq!(s.records.len(), PER_SESSION);
    assert!(s.issue(claims(), "".into(), None, now + TTL, TTL).is_ok());
    assert_eq!(s.records.len(), 1);
}
#[test]
fn global_capacity_refuses_without_replacing_existing_admissions() {
    let mut s = Tickets::default();
    let now = Instant::now();
    for n in 0..CAPACITY {
        let mut c = claims();
        c.sid = n.to_string();
        s.issue(c, "".into(), None, now, TTL).unwrap();
    }
    assert_eq!(
        s.issue(claims(), "".into(), None, now, TTL).unwrap_err(),
        IssueError::Capacity
    );
    assert_eq!(s.records.len(), CAPACITY);
}
#[test]
fn concurrent_redemption_has_one_winner() {
    let s = new_store();
    let now = Instant::now();
    let token = s
        .lock()
        .unwrap()
        .issue(claims(), "".into(), None, now, TTL)
        .unwrap();
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let s = s.clone();
            let token = token.clone();
            std::thread::spawn(move || s.lock().unwrap().consume(&token, "", None, now).is_some())
        })
        .collect();
    assert_eq!(
        handles
            .into_iter()
            .map(|h| h.join().unwrap() as usize)
            .sum::<usize>(),
        1
    );
}
#[test]
fn live_grant_tracks_logout_role_suspension_and_absolute_session_age() {
    let sessions = crate::jwt_sessions::new_session_store();
    register(&sessions);
    let users = Arc::new(Mutex::new(vec![user()]));
    let g = ConnectionGrant {
        user_id: "root-test-id".into(),
        username: "root-test".into(),
        sid: "test-session".into(),
        sessions: sessions.clone(),
        users: users.clone(),
        path: "/api/terminal".into(),
    };
    assert!(g.valid());
    let mut missing_identity = g.clone();
    missing_identity.user_id.clear();
    assert!(!missing_identity.valid());
    users.lock().unwrap()[0].role = UserRole::Operator;
    assert!(!g.valid());
    users.lock().unwrap()[0].role = UserRole::Admin;
    users.lock().unwrap()[0].status = crate::models::AccountStatus::Suspended;
    assert!(!g.valid());
    users.lock().unwrap()[0].status = Default::default();
    sessions
        .lock()
        .unwrap()
        .get_mut("test-session")
        .unwrap()
        .created_at = (chrono::Utc::now() - chrono::Duration::days(7))
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();
    assert!(!g.valid());
    register(&sessions);
    assert!(g.valid());
    users.lock().unwrap()[0].id = "replacement-account-id".into();
    assert!(!g.valid());
    users.lock().unwrap()[0].id = "root-test-id".into();
    crate::jwt_sessions::revoke_user_sessions(&sessions, "root-test", None);
    assert!(!g.valid());
}

async fn actor(req: HttpRequest) -> HttpResponse {
    HttpResponse::Ok().json(auth::extract_username(&req))
}
#[actix_web::test]
async fn real_auth_ticket_admission_rejects_replay_revoked_session_and_forged_role() {
    let secret: auth::JwtSecret = Arc::new("ticket-fixture-secret".into());
    let sessions = crate::jwt_sessions::new_session_store();
    register(&sessions);
    let users = Arc::new(Mutex::new(vec![user()]));
    let tickets = new_store();
    let token = jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &claims(),
        &jsonwebtoken::EncodingKey::from_secret(secret.as_bytes()),
    )
    .unwrap();
    let app = awtest::init_service(
        App::new()
            .wrap(AuthCheck::new(secret, sessions.clone(), users.clone()))
            .app_data(web::Data::new(crate::remote::extension_points()))
            .app_data(web::Data::new(tickets))
            .configure(config)
            .route("/api/terminal", web::get().to(actor)),
    )
    .await;
    let req = awtest::TestRequest::post()
        .uri("/api/terminal/tickets")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .insert_header(("Origin", "https://test.example"))
        .set_json(serde_json::json!({"query":"server_id=one&history=1"}))
        .to_request();
    let response = awtest::call_service(&app, req).await;
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers().get("Cache-Control").unwrap(), "no-store");
    let data: serde_json::Value = awtest::read_body_json(response).await;
    let ticket = data["data"]["ticket"].as_str().unwrap();
    let uri = format!("/api/terminal?server_id=one&history=1&ticket={ticket}");
    for expected in [200, 401] {
        let request = awtest::TestRequest::get()
            .uri(&uri)
            .insert_header(("Origin", "https://test.example"))
            .insert_header(("X-User", "intruder"))
            .to_request();
        let response = awtest::call_service(&app, request).await;
        assert_eq!(response.status(), expected);
        if expected == 200 {
            assert_eq!(
                awtest::read_body_json::<Option<String>, _>(response)
                    .await
                    .as_deref(),
                Some("root-test")
            );
        }
    }
    let req = awtest::TestRequest::post()
        .uri("/api/terminal/tickets")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .set_json(serde_json::json!({"query":""}))
        .to_request();
    let data: serde_json::Value =
        awtest::read_body_json(awtest::call_service(&app, req).await).await;
    let ticket = data["data"]["ticket"].as_str().unwrap();
    users.lock().unwrap()[0].role = UserRole::Operator;
    let req = awtest::TestRequest::get()
        .uri(&format!("/api/terminal?ticket={ticket}"))
        .to_request();
    assert_eq!(awtest::call_service(&app, req).await.status(), 403);
    let req = awtest::TestRequest::post()
        .uri("/api/terminal/tickets")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .insert_header(("X-User", "root-test"))
        .set_json(serde_json::json!({"query":""}))
        .to_request();
    assert_eq!(awtest::call_service(&app, req).await.status(), 403);
    users.lock().unwrap()[0].role = UserRole::Admin;
    let req = awtest::TestRequest::post()
        .uri("/api/terminal/tickets")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .set_json(serde_json::json!({"query":""}))
        .to_request();
    let data: serde_json::Value =
        awtest::read_body_json(awtest::call_service(&app, req).await).await;
    let ticket = data["data"]["ticket"].as_str().unwrap();
    crate::jwt_sessions::revoke_user_sessions(&sessions, "root-test", None);
    let req = awtest::TestRequest::get()
        .uri(&format!("/api/terminal?ticket={ticket}"))
        .to_request();
    assert_eq!(awtest::call_service(&app, req).await.status(), 401);
}
#[actix_web::test]
async fn query_jwt_is_rejected_and_header_identity_remains_supported() {
    let secret: auth::JwtSecret = Arc::new("ticket-fixture-secret".into());
    let sessions = crate::jwt_sessions::new_session_store();
    register(&sessions);
    let users = Arc::new(Mutex::new(vec![user()]));
    let token = jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &claims(),
        &jsonwebtoken::EncodingKey::from_secret(secret.as_bytes()),
    )
    .unwrap();
    let app = awtest::init_service(
        App::new()
            .wrap(AuthCheck::new(secret, sessions, users))
            .app_data(web::Data::new(crate::remote::extension_points()))
            .route("/api/terminal", web::get().to(actor)),
    )
    .await;
    let request = awtest::TestRequest::get()
        .uri(&format!("/api/terminal?token={token}"))
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();
    assert_eq!(awtest::call_service(&app, request).await.status(), 401);
    let request = awtest::TestRequest::get()
        .uri("/api/terminal")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();
    assert_eq!(awtest::call_service(&app, request).await.status(), 200);
}

#[test]
fn admission_never_exceeds_jwt_expiry_even_with_fractional_seconds() {
    let now = chrono::DateTime::<chrono::Utc>::from_timestamp(1_800_000_000, 500_000_000).unwrap();
    assert_eq!(
        admission_lifetime(1_800_000_010, now).unwrap(),
        Duration::from_millis(9500)
    );
    assert_eq!(admission_lifetime(1_800_000_100, now).unwrap(), TTL);
    assert!(admission_lifetime(1_800_000_000, now).is_none());
    assert!(admission_lifetime(1_800_000_001, now).is_none());
    assert!(admission_lifetime(usize::MAX, now).is_none());
}

#[tokio::test]
async fn guarded_stream_closes_while_idle_and_rechecks_before_each_event() {
    use futures::StreamExt;
    let sessions = crate::jwt_sessions::new_session_store();
    register(&sessions);
    let users = Arc::new(Mutex::new(vec![user()]));
    let grant = ConnectionGrant {
        user_id: "root-test-id".into(),
        username: "root-test".into(),
        sid: "test-session".into(),
        sessions: sessions.clone(),
        users: users.clone(),
        path: "/api/mec/v1/dashboard/live/stream".into(),
    };
    let pending = futures::stream::pending::<Result<web::Bytes, std::convert::Infallible>>();
    let guarded = crate::stream_access::guard(pending, grant.clone());
    tokio::pin!(guarded);
    let to_revoke = sessions.clone();
    let revoker = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        crate::jwt_sessions::revoke_user_sessions(&to_revoke, "root-test", None);
    });
    assert!(tokio::time::timeout(Duration::from_secs(2), guarded.next())
        .await
        .unwrap()
        .is_none());
    revoker.await.unwrap();
    register(&sessions);
    let inner = futures::stream::iter(vec![
        Ok::<_, std::convert::Infallible>(web::Bytes::from_static(b"one")),
        Ok(web::Bytes::from_static(b"two")),
    ]);
    let guarded = crate::stream_access::guard(inner, grant);
    tokio::pin!(guarded);
    assert_eq!(
        guarded.next().await.unwrap().unwrap(),
        web::Bytes::from_static(b"one")
    );
    users.lock().unwrap()[0].status = crate::models::AccountStatus::Suspended;
    assert!(guarded.next().await.is_none());
}
#[test]
fn stream_grant_uses_its_endpoint_policy_instead_of_terminal_privileges() {
    let sessions = crate::jwt_sessions::new_session_store();
    register(&sessions);
    let mut operator = user();
    operator.role = UserRole::Operator;
    let users = Arc::new(Mutex::new(vec![operator]));
    let mut grant = ConnectionGrant {
        user_id: "root-test-id".into(),
        username: "root-test".into(),
        sid: "test-session".into(),
        sessions,
        users: users.clone(),
        path: "/api/mec/v1/dashboard/live/stream".into(),
    };
    assert!(grant.valid());
    grant.path = "/api/terminal".into();
    assert!(!grant.valid());
    grant.path = "/api/mec/v1/dashboard/live/stream".into();
    users.lock().unwrap()[0].scope_restricted = true;
    assert!(!grant.valid());
}

fn register(sessions: &SessionStore) {
    crate::jwt_sessions::register_session(sessions, "test-session", "root-test", "root-test-id", "127.0.0.1");
}
