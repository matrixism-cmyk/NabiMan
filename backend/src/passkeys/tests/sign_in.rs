use super::*;

#[actix_web::test]
async fn a_registered_passkey_signs_in_by_itself() {
    let world = World::with(vec![user("u1", "kim", "operator")]);
    let app = serve!(world);
    let mut browser = Browser::at(ORIGIN);
    let token = password_token(&app, "kim").await;
    let id = register(&app, &mut browser, &token, "노트북 지문").await;

    world.sessions.lock_safe().clear();
    let (status, body) = sign_in(&app, &mut browser, &id).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["username"], "kim");
    assert_eq!(body["data"]["role"], "operator");
    assert!(body["data"]["refresh_token"].is_string());
    assert_eq!(world.sessions.lock_safe().len(), 1, "one sign-in, recorded like any other");

    // The token is a normal one: it opens the account's own pages.
    let token = body["data"]["token"].as_str().unwrap();
    let (status, list) = call(&app, "GET", "/api/auth/passkeys", Some(token), Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    let row = &list["data"]["passkeys"][0];
    assert_eq!(row["name"], "노트북 지문");
    assert!(row["last_used"].is_string() && row["created_at"].is_string());
    assert!(row.get("credential").is_none(), "no key material in the list");
    assert!(world.account("kim").last_login.is_some());

    // The audit record says who signed in and with which passkey, and nothing secret.
    let recorded = world.recorded.lock().unwrap();
    let (_, _, change) = recorded.iter().rev().find(|(path, _, _)| path == "/api/auth/passkeys/login/finish").unwrap();
    let change = change.as_ref().expect("a noted sign-in");
    assert_eq!((change.resource.as_str(), change.action.as_str()), ("passkey", "sign-in"));
    assert!(change.fields.iter().any(|f| f.field == "username" && f.new.as_deref() == Some("kim")));
    let text = serde_json::to_string(change).unwrap();
    assert!(!text.contains("credential") && !text.contains(&id), "{text}");
}

#[actix_web::test]
async fn an_answer_from_another_origin_is_refused() {
    let world = World::with(vec![user("u1", "kim", "operator")]);
    let app = serve!(world);
    let mut browser = Browser::at(ORIGIN);
    let token = password_token(&app, "kim").await;
    let id = register(&app, &mut browser, &token, "key").await;
    world.sessions.lock_safe().clear();

    // Same device and passkey, but the page asking is somewhere else.
    for elsewhere in ["https://sub.man.example.com", "https://man.example.com:8443"] {
        browser.origin = Url::parse(elsewhere).unwrap();
        let (status, body) = sign_in(&app, &mut browser, &id).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{elsewhere}: {body}");
        assert_eq!(body["message"], "Invalid credentials");
    }
    assert!(world.sessions.lock_safe().is_empty());
    browser.origin = Url::parse(ORIGIN).unwrap();
    assert_eq!(sign_in(&app, &mut browser, &id).await.0, StatusCode::OK);
}

#[actix_web::test]
async fn a_challenge_is_good_for_one_answer_and_an_answer_for_one_challenge() {
    let world = World::with(vec![user("u1", "kim", "operator")]);
    let app = serve!(world);
    let mut browser = Browser::at(ORIGIN);
    let token = password_token(&app, "kim").await;
    let id = register(&app, &mut browser, &token, "key").await;
    world.sessions.lock_safe().clear();

    let body = answer(&app, &mut browser, &id).await;
    assert_eq!(call(&app, "POST", "/api/auth/passkeys/login/finish", None, body.clone()).await.0, StatusCode::OK);
    // Replayed as it was: the challenge is gone.
    assert_eq!(call(&app, "POST", "/api/auth/passkeys/login/finish", None, body.clone()).await.0, StatusCode::UNAUTHORIZED);
    // Replayed against a fresh challenge: it was signed for the old one.
    let (_, fresh) = call(&app, "POST", "/api/auth/passkeys/login/begin", None, json!({})).await;
    let replay = json!({ "challenge_id": fresh["data"]["challenge_id"], "credential": body["credential"] });
    assert_eq!(call(&app, "POST", "/api/auth/passkeys/login/finish", None, replay.clone()).await.0, StatusCode::UNAUTHORIZED);
    // And a wrong answer used that challenge up too.
    assert_eq!(world.passkeys.challenges.len(), 0);
    // The same credential written another way (to dodge the per-passkey count) is refused.
    let mut respelled = answer(&app, &mut browser, &id).await;
    respelled["credential"]["id"] = format!("{id}=").into();
    assert_eq!(call(&app, "POST", "/api/auth/passkeys/login/finish", None, respelled).await.0, StatusCode::UNAUTHORIZED);
    // An unknown challenge, and a registration's challenge, are no sign-in challenges.
    let unknown = json!({ "challenge_id": "00000000000000000000000000000000", "credential": body["credential"] });
    assert_eq!(call(&app, "POST", "/api/auth/passkeys/login/finish", None, unknown).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(world.sessions.lock_safe().len(), 1);
    for (bytes, written) in [(&b""[..], ""), (b"\xfb", "-w"), (b"\xfb\xff", "-_8"), (b"\xfb\xff\xfe", "-__-"), (b"\x01\x02\x03\x04", "AQIDBA")] {
        assert_eq!(crate::passkeys::store::browser_id(bytes), written);
    }
}

#[actix_web::test]
async fn an_older_answer_after_a_newer_one_is_a_cloned_passkey() {
    let world = World::with(vec![user("u1", "kim", "operator")]);
    let app = serve!(world);
    let mut browser = Browser::at(ORIGIN);
    let token = password_token(&app, "kim").await;
    let id = register(&app, &mut browser, &token, "key").await;

    let earlier = answer(&app, &mut browser, &id).await; // signature counter 1
    let later = answer(&app, &mut browser, &id).await; // signature counter 2
    assert_eq!(call(&app, "POST", "/api/auth/passkeys/login/finish", None, later).await.0, StatusCode::OK);
    let (status, _) = call(&app, "POST", "/api/auth/passkeys/login/finish", None, earlier).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "the counter went backwards");
}

#[actix_web::test]
async fn every_account_check_of_a_password_sign_in_applies() {
    let world = World::with(vec![user("u1", "kim", "operator"), user("u2", "lee", "viewer")]);
    let app = serve!(world);
    let mut browser = Browser::at(ORIGIN);
    let token = password_token(&app, "kim").await;
    let id = register(&app, &mut browser, &token, "key").await;
    world.sessions.lock_safe().clear();

    use crate::models::AccountStatus::{Active, Pending, Rejected, Suspended};
    for status in [Suspended, Pending, Rejected] {
        world.update("kim", |u| u.status = status.clone());
        let (code, body) = sign_in_from(&app, &mut browser, &id, "192.0.2.1:1").await;
        assert_eq!(code, StatusCode::FORBIDDEN, "{status:?}");
        // Word for word what the password sign-in says.
        let (password_code, password_body) =
            call_from(&app, "POST", "/api/auth/login", None, json!({ "username": "kim", "password": PASSWORD }), Some("192.0.2.9:1")).await;
        assert_eq!((code, &body["message"]), (password_code, &password_body["message"]));
    }
    world.update("kim", |u| u.status = Active);

    // An account whose period ended.
    world.update("kim", |u| u.access.expires_at = Some("2000-01-01T00:00:00Z".into()));
    assert!(world.account("kim").access.expired());
    assert_eq!(sign_in_from(&app, &mut browser, &id, "192.0.2.2:1").await.0, StatusCode::FORBIDDEN);
    world.update("kim", |u| u.access.expires_at = None);

    // An account limited to a network it is not on (the test client is elsewhere).
    world.update("kim", |u| u.access.allowed_networks = vec!["203.0.113.0/24".into()]);
    assert_eq!(sign_in_from(&app, &mut browser, &id, "192.0.2.3:1").await.0, StatusCode::FORBIDDEN);
    world.update("kim", |u| u.access.allowed_networks.clear());

    assert!(world.sessions.lock_safe().is_empty(), "none of those signed anyone in");
    assert!(world.account("kim").mfa.passkeys[0].last_used.is_none());
    assert_eq!(sign_in_from(&app, &mut browser, &id, "192.0.2.4:1").await.0, StatusCode::OK);

    // A deleted account's passkey finds nobody.
    world.users.lock_safe().retain(|u| u.username != "kim");
    assert_eq!(sign_in_from(&app, &mut browser, &id, "192.0.2.5:1").await.0, StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn an_account_with_an_authenticator_app_is_not_asked_for_its_code() {
    let world = World::with(vec![user("u1", "kim", "operator")]);
    let app = serve!(world);
    let mut browser = Browser::at(ORIGIN);
    let token = password_token(&app, "kim").await;
    let id = register(&app, &mut browser, &token, "key").await;
    world.update("kim", |u| {
        u.totp_enabled = true;
        u.totp_secret = Some(base32::encode(base32::Alphabet::Rfc4648 { padding: false }, &[7u8; 20]));
    });
    world.sessions.lock_safe().clear();

    // The password alone still stops at the second step.
    let (_, body) = call(&app, "POST", "/api/auth/login", None, json!({ "username": "kim", "password": PASSWORD })).await;
    assert_eq!(body["data"]["requires_2fa"], true);
    assert!(world.sessions.lock_safe().is_empty());

    let (status, body) = sign_in(&app, &mut browser, &id).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body["data"]["token"].is_string() && body["data"].get("requires_2fa").is_none());
    let sessions = world.sessions.lock_safe();
    assert!(sessions.values().next().unwrap().mfa_verified, "the authenticator verified the person");
}

#[actix_web::test]
async fn the_sign_in_rate_limits_apply() {
    let world = World::with(vec![user("u1", "kim", "operator")]);
    let app = serve!(world);
    let mut browser = Browser::at(ORIGIN);
    let token = password_token(&app, "kim").await;
    let id = register(&app, &mut browser, &token, "key").await;

    // Five failed answers for one passkey from one address, then that pair waits.
    let wrong = |challenge: &str| json!({ "challenge_id": challenge, "credential": { "id": id, "rawId": id, "type": "public-key",
        "response": { "authenticatorData": "AAAA", "clientDataJSON": "AAAA", "signature": "AAAA", "userHandle": "AAAA" } } });
    for _ in 0..5 {
        let (status, _) = call_from(&app, "POST", "/api/auth/passkeys/login/finish", None, wrong("none"), Some("198.51.100.7:1")).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
    let (status, body) = call_from(&app, "POST", "/api/auth/passkeys/login/finish", None, wrong("none"), Some("198.51.100.7:1")).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{body}");
    // The passkey's owner, elsewhere, is not shut out.
    let (status, _) = call_from(&app, "POST", "/api/auth/passkeys/login/finish", None, wrong("none"), Some("192.0.2.10:1")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // Asking for challenges is counted per address (60 a minute).
    let mut last = StatusCode::OK;
    for _ in 0..61 {
        last = call_from(&app, "POST", "/api/auth/passkeys/login/begin", None, json!({}), Some("203.0.113.50:1")).await.0;
    }
    assert_eq!(last, StatusCode::TOO_MANY_REQUESTS);
    // A sign-in answer that is not JSON at all never reaches the handler.
    let request = test::TestRequest::post().uri("/api/auth/passkeys/login/finish").set_payload("not json").to_request();
    assert_eq!(test::call_service(&app, request).await.status(), StatusCode::BAD_REQUEST);
}

#[actix_web::test]
async fn without_the_public_origin_passkeys_are_off_and_say_so() {
    let world = World::configured(vec![user("u1", "kim", "admin"), user("u2", "lee", "viewer")], None);
    let app = serve!(world);
    let (status, body) = call(&app, "POST", "/api/auth/passkeys/login/begin", None, json!({})).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["message"], "Passkeys are not configured on this server");
    let admin = password_token(&app, "kim").await;
    let (status, _) = call(&app, "POST", "/api/auth/passkeys/register/begin", Some(&admin), json!({ "password": PASSWORD })).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    let (_, list) = call(&app, "GET", "/api/auth/passkeys", Some(&admin), Value::Null).await;
    assert_eq!(list["data"]["enabled"], false);
    assert!(list["data"]["reason"].as_str().unwrap().contains("NABIMAN_PUBLIC_ORIGIN"));
    // Why is for whoever can fix it.
    let viewer = password_token(&app, "lee").await;
    let (_, list) = call(&app, "GET", "/api/auth/passkeys", Some(&viewer), Value::Null).await;
    assert!(list["data"]["enabled"] == false && list["data"]["reason"].is_null());
}
