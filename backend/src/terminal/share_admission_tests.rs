use super::*;

#[actix_web::test]
pub(super) async fn authenticated_api_enforces_finite_defaults_upgrade_guard_and_durable_failure() {
    use actix_web::{test as awtest, App};
    let fixture = Fixture::new();
    let secret = Arc::new("isolated-share-api-test-secret".to_string());
    let sessions = crate::jwt_sessions::new_session_store();
    crate::jwt_sessions::register_session(
        &sessions,
        "share-api-sid",
        "owner",
        "owner-immutable-id",
        "127.0.0.1",
    );
    let (token, _) =
        crate::jwt_sessions::create_token_pair(&secret, "owner", "admin", "share-api-sid").unwrap();
    let app = awtest::init_service(
        App::new()
            .app_data(web::Data::new(fixture.shares.clone()))
            .app_data(web::Data::new(share_ticket::new_store()))
            .app_data(web::Data::new(fixture.sessions.clone()))
            .app_data(web::Data::new(fixture.users.clone()))
            .app_data(web::Data::new(sessions.clone()))
            .app_data(web::Data::new(secret.clone()))
            .wrap(crate::AuthCheck::new(
                secret,
                sessions,
                fixture.users.clone(),
            ))
            .configure(config),
    )
    .await;
    for expiry in [0, MAX_EXPIRY_SECS + 1] {
        let req = awtest::TestRequest::post()
            .uri("/api/terminal/sessions/terminal-one/share-link")
            .insert_header(("Authorization", format!("Bearer {token}")))
            .set_json(serde_json::json!({"expires_in_secs":expiry}))
            .to_request();
        assert_eq!(awtest::call_service(&app, req).await.status(), 400);
    }
    let req = awtest::TestRequest::post()
        .uri("/api/terminal/sessions/terminal-one/share-link")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .set_json(serde_json::json!({"expires_in_secs":3600}))
        .to_request();
    let response = awtest::call_service(&app, req).await;
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers().get("Cache-Control").unwrap(), "no-store");
    let body: serde_json::Value = awtest::read_body_json(response).await;
    assert_eq!(body["success"], true);
    assert_eq!(body["data"]["read_only"], true);
    assert_eq!(body["data"]["owner_id"], fixture.link.owner_id);
    let created = body["data"]["token"].as_str().unwrap();
    assert_eq!(created.len(), 64);
    let req = awtest::TestRequest::get()
        .uri(&format!("/api/share/{created}/ws"))
        .to_request();
    assert_eq!(awtest::call_service(&app, req).await.status(), 401);
    let req = awtest::TestRequest::post()
        .uri(&format!("/api/share/{created}/auth"))
        .set_json(serde_json::json!({"password":"", "query":""}))
        .to_request();
    let response = awtest::call_service(&app, req).await;
    assert_eq!(response.status(), 200);
    let receipt: serde_json::Value = awtest::read_body_json(response).await;
    let admission = receipt["data"]["ticket"].as_str().unwrap();
    let req = awtest::TestRequest::get()
        .uri(&format!("/api/share/{created}/ws?ticket={admission}"))
        .to_request();
    assert_eq!(awtest::call_service(&app, req).await.status(), 400);
    let req = awtest::TestRequest::get()
        .uri(&format!("/api/share/{created}/ws?ticket={admission}"))
        .to_request();
    assert_eq!(awtest::call_service(&app, req).await.status(), 401);
    assert_eq!(fixture.sessions.lock().unwrap()["terminal-one"].ws_count, 0);
    let req = awtest::TestRequest::get()
        .uri("/api/share/nonexistent")
        .to_request();
    let response = awtest::call_service(&app, req).await;
    assert_eq!(response.headers().get("Cache-Control").unwrap(), "no-store");
    let path = fixture.directory.join("terminal_shares.json");
    std::fs::rename(&path, fixture.directory.join("last-published.json")).unwrap();
    std::fs::create_dir(&path).unwrap();
    let req = awtest::TestRequest::delete()
        .uri(&format!("/api/terminal/share-links/{created}"))
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();
    let response = awtest::call_service(&app, req).await;
    assert_eq!(response.status(), 503);
    let body: serde_json::Value = awtest::read_body_json(response).await;
    assert_eq!(body["success"], false);
    assert!(!fixture.grant().valid());
    for endpoint in [
        "/api/terminal/share-links".to_string(),
        format!("/api/share/{created}"),
    ] {
        let req = awtest::TestRequest::get()
            .uri(&endpoint)
            .insert_header(("Authorization", format!("Bearer {token}")))
            .to_request();
        let response = awtest::call_service(&app, req).await;
        assert_eq!(response.status(), 503);
        assert_eq!(response.headers().get("Cache-Control").unwrap(), "no-store");
    }
}

#[actix_web::test]
pub(super) async fn share_proof_binds_origin_query_current_policy_and_rejects_old_format() {
    use actix_web::{test as awtest, App};
    let fixture = Fixture::new();
    fixture.change(|l| l.password_hash = Some(bcrypt::hash("fixture-password", 4).unwrap()));
    let admission = share_ticket::new_store();
    let secret = Arc::new("share-proof-api-secret".to_string());
    let app = awtest::init_service(
        App::new()
            .app_data(web::Data::new(fixture.shares.clone()))
            .app_data(web::Data::new(admission.clone()))
            .app_data(web::Data::new(fixture.users.clone()))
            .app_data(web::Data::new(fixture.sessions.clone()))
            .wrap(crate::AuthCheck::new(
                secret,
                crate::jwt_sessions::new_session_store(),
                fixture.users.clone(),
            ))
            .configure(config),
    )
    .await;
    let endpoint = format!("/api/share/{}/auth", fixture.link.token);
    let issue = || {
        awtest::TestRequest::post()
            .uri(&endpoint)
            .insert_header(("Origin", "https://one.test"))
            .set_json(serde_json::json!({"password":"fixture-password", "query":"cols=80"}))
            .to_request()
    };
    let req = awtest::TestRequest::post()
        .uri(&endpoint)
        .set_json(serde_json::json!({"password":"wrong", "query":""}))
        .to_request();
    assert_eq!(awtest::call_service(&app, req).await.status(), 403);
    let response = awtest::call_service(&app, issue()).await;
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers().get("Cache-Control").unwrap(), "no-store");
    let receipt: serde_json::Value = awtest::read_body_json(response).await;
    let proof = receipt["data"]["ticket"].as_str().unwrap();
    assert_eq!(receipt["data"]["expires_in_seconds"], 30);
    let request = |proof: &str, query: &str, origin: &str| {
        awtest::TestRequest::get()
            .uri(&format!(
                "/api/share/{}/ws?{query}&ticket={proof}",
                fixture.link.token
            ))
            .insert_header(("Origin", origin))
            .insert_header(("Upgrade", "websocket"))
            .insert_header(("Connection", "upgrade"))
            .insert_header(("Sec-WebSocket-Version", "13"))
            .insert_header(("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ=="))
            .to_request()
    };
    assert_eq!(
        awtest::call_service(&app, request(proof, "cols=80", "https://other.test"))
            .await
            .status(),
        401
    );
    assert_eq!(
        awtest::call_service(&app, request(proof, "cols=80", "https://one.test"))
            .await
            .status(),
        401
    );
    let receipt: serde_json::Value =
        awtest::read_body_json(awtest::call_service(&app, issue()).await).await;
    let proof = receipt["data"]["ticket"].as_str().unwrap();
    assert_eq!(
        awtest::call_service(&app, request(proof, "cols=81", "https://one.test"))
            .await
            .status(),
        401
    );
    let receipt: serde_json::Value =
        awtest::read_body_json(awtest::call_service(&app, issue()).await).await;
    let proof = receipt["data"]["ticket"].as_str().unwrap();
    fixture.change(|l| l.read_only = false);
    assert_eq!(
        awtest::call_service(&app, request(proof, "cols=80", "https://one.test"))
            .await
            .status(),
        403
    );
    let receipt: serde_json::Value =
        awtest::read_body_json(awtest::call_service(&app, issue()).await).await;
    let proof = receipt["data"]["ticket"].as_str().unwrap();
    fixture.users.lock().unwrap()[0].role = UserRole::Viewer;
    assert_eq!(
        awtest::call_service(&app, request(proof, "cols=80", "https://one.test"))
            .await
            .status(),
        403
    );
    assert_eq!(
        awtest::call_service(
            &app,
            request("9999999999.old-signed-proof", "cols=80", "https://one.test")
        )
        .await
        .status(),
        401
    );
    assert_eq!(fixture.sessions.lock().unwrap()["terminal-one"].ws_count, 0);
    assert_eq!(admission.tickets.lock().unwrap().pending_count(), 0);
}

#[actix_web::test]
pub(super) async fn public_password_attempts_are_bounded_even_with_forged_forwarded_addresses() {
    use actix_web::{test as awtest, App};
    let fixture = Fixture::new();
    fixture.change(|l| l.password_hash = Some(bcrypt::hash("fixture-password", 4).unwrap()));
    let admission = share_ticket::new_store();
    let app = awtest::init_service(
        App::new()
            .app_data(web::Data::new(fixture.shares.clone()))
            .app_data(web::Data::new(fixture.users.clone()))
            .app_data(web::Data::new(admission.clone()))
            .configure(config),
    )
    .await;
    let endpoint = format!("/api/share/{}/auth", fixture.link.token);
    for index in 0..30 {
        let req = awtest::TestRequest::post()
            .uri(&endpoint)
            .peer_addr("127.0.0.1:9000".parse().unwrap())
            .insert_header(("X-Forwarded-For", format!("192.0.2.{index}")))
            .set_json(serde_json::json!({"password":"wrong", "query":""}))
            .to_request();
        assert_eq!(awtest::call_service(&app, req).await.status(), 403);
    }
    let req = awtest::TestRequest::post()
        .uri(&endpoint)
        .peer_addr("127.0.0.2:9000".parse().unwrap())
        .set_json(serde_json::json!({"password":"fixture-password", "query":""}))
        .to_request();
    let response = awtest::call_service(&app, req).await;
    assert_eq!(response.status(), 429);
    assert_eq!(response.headers().get("Retry-After").unwrap(), "60");
    assert_eq!(response.headers().get("Cache-Control").unwrap(), "no-store");
    assert_eq!(admission.tickets.lock().unwrap().pending_count(), 0);
}

#[actix_web::test]
pub(super) async fn verification_capacity_and_owner_revocation_during_bcrypt_do_not_issue_admission(
) {
    use actix_web::{test as awtest, App};
    let fixture = Fixture::new();
    fixture.change(|l| l.password_hash = Some(bcrypt::hash("fixture-password", 12).unwrap()));
    let admission = share_ticket::new_store();
    let app = awtest::init_service(
        App::new()
            .app_data(web::Data::new(fixture.shares.clone()))
            .app_data(web::Data::new(fixture.users.clone()))
            .app_data(web::Data::new(admission.clone()))
            .configure(config),
    )
    .await;
    let endpoint = format!("/api/share/{}/auth", fixture.link.token);
    let request = || {
        awtest::TestRequest::post()
            .uri(&endpoint)
            .set_json(serde_json::json!({"password":"fixture-password", "query":""}))
            .to_request()
    };
    let permit = admission
        .verification
        .clone()
        .acquire_many_owned(4)
        .await
        .unwrap();
    assert_eq!(awtest::call_service(&app, request()).await.status(), 429);
    drop(permit);
    let revoke = async {
        for _ in 0..1000 {
            if admission.verification.available_permits() < 4 {
                fixture.users.lock().unwrap()[0].role = UserRole::Viewer;
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }
        panic!("Verification did not become live");
    };
    let (response, ()) = futures::join!(awtest::call_service(&app, request()), revoke);
    assert_eq!(response.status(), 403);
    assert_eq!(admission.tickets.lock().unwrap().pending_count(), 0);
    assert_eq!(admission.verification.available_permits(), 4);
}
