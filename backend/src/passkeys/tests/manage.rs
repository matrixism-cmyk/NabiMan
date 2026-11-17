use super::*;

#[actix_web::test]
async fn registering_and_removing_ask_for_the_password_again() {
    let world = World::with(vec![user("u1", "kim", "operator")]);
    let app = serve!(world);
    let mut browser = Browser::at(ORIGIN);
    let token = password_token(&app, "kim").await;

    for wrong in [json!({ "password": "not it" }), json!({ "password": "" })] {
        let (status, body) = call(&app, "POST", "/api/auth/passkeys/register/begin", Some(&token), wrong).await;
        assert_eq!((status, body["message"].as_str()), (StatusCode::BAD_REQUEST, Some("Invalid password")));
    }
    assert_eq!(world.passkeys.challenges.len(), 0, "no challenge without the password");
    let id = register(&app, &mut browser, &token, "  회사 노트북  ").await;
    let stored = world.account("kim").mfa.passkeys;
    assert_eq!((stored.len(), stored[0].name.as_str()), (1, "회사 노트북"));
    let key = stored[0].id.clone();
    assert_ne!(key, id, "listed by a digest, not by the credential id itself");

    let (status, body) = call(&app, "POST", "/api/auth/passkeys/delete", Some(&token), json!({ "id": key, "password": "not it" })).await;
    assert_eq!((status, body["message"].as_str()), (StatusCode::BAD_REQUEST, Some("Invalid password")));
    assert_eq!(world.account("kim").mfa.passkeys.len(), 1);
    let (status, _) = call(&app, "POST", "/api/auth/passkeys/delete", Some(&token), json!({ "id": "nope", "password": PASSWORD })).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, _) = call(&app, "POST", "/api/auth/passkeys/delete", Some(&token), json!({ "id": key, "password": PASSWORD })).await;
    assert_eq!(status, StatusCode::OK);
    assert!(world.account("kim").mfa.passkeys.is_empty());
    // The removed passkey no longer signs in.
    let (status, body) = sign_in(&app, &mut browser, &id).await;
    assert_eq!((status, body["message"].as_str()), (StatusCode::UNAUTHORIZED, Some("Invalid credentials")));

    // Registration and removal are in the audit record, by name, without key material.
    let recorded = world.recorded.lock().unwrap();
    let noted = |path: &str| recorded.iter().filter(|(p, status, _)| p == path && *status == 200).filter_map(|(_, _, c)| c.clone()).next_back().unwrap();
    let created = noted("/api/auth/passkeys/register/finish");
    assert_eq!((created.action.as_str(), created.id.as_str()), ("create", key.as_str()));
    assert!(created.fields.iter().any(|f| f.field == "name" && f.new.as_deref() == Some("회사 노트북")));
    let deleted = noted("/api/auth/passkeys/delete");
    assert_eq!(deleted.action, "delete");
    assert!(recorded.iter().any(|(p, status, change)| p == "/api/auth/passkeys/delete" && *status == 400 && change.is_none()));
}

#[actix_web::test]
async fn an_account_lists_and_renames_only_its_own() {
    let world = World::with(vec![user("u1", "kim", "operator"), user("u2", "lee", "viewer")]);
    let app = serve!(world);
    let (mut kims, mut lees) = (Browser::at(ORIGIN), Browser::at(ORIGIN));
    let kim = password_token(&app, "kim").await;
    let lee = password_token(&app, "lee").await;
    register(&app, &mut kims, &kim, "kim's").await;
    register(&app, &mut lees, &lee, "lee's").await;
    let kims_key = world.account("kim").mfa.passkeys[0].id.clone();

    let (_, list) = call(&app, "GET", "/api/auth/passkeys", Some(&lee), Value::Null).await;
    assert_eq!(list["data"]["enabled"], true);
    assert_eq!(list["data"]["passkeys"].as_array().unwrap().len(), 1);
    assert_eq!(list["data"]["passkeys"][0]["name"], "lee's");

    // Lee can neither rename nor remove Kim's.
    let (status, _) = call(&app, "POST", "/api/auth/passkeys/rename", Some(&lee), json!({ "id": kims_key, "name": "mine now" })).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = call(&app, "POST", "/api/auth/passkeys/delete", Some(&lee), json!({ "id": kims_key, "password": PASSWORD })).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(world.account("kim").mfa.passkeys[0].name, "kim's");

    let (status, _) = call(&app, "POST", "/api/auth/passkeys/rename", Some(&kim), json!({ "id": kims_key, "name": "집 데스크톱" })).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(world.account("kim").mfa.passkeys[0].name, "집 데스크톱");
    for bad in ["", "   ", "a\u{0}b", &"가".repeat(65)] {
        let (status, _) = call(&app, "POST", "/api/auth/passkeys/rename", Some(&kim), json!({ "id": kims_key, "name": bad })).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{bad:?}");
    }
}

#[actix_web::test]
async fn a_registration_challenge_belongs_to_the_account_that_asked() {
    let world = World::with(vec![user("u1", "kim", "operator"), user("u2", "lee", "viewer")]);
    let app = serve!(world);
    let mut browser = Browser::at(ORIGIN);
    let kim = password_token(&app, "kim").await;
    let lee = password_token(&app, "lee").await;

    let (_, begun) = call(&app, "POST", "/api/auth/passkeys/register/begin", Some(&kim), json!({ "password": PASSWORD })).await;
    let credential = browser.create(&begun["data"]["publicKey"]);
    let body = json!({ "challenge_id": begun["data"]["challenge_id"], "name": "x", "credential": credential });
    // Lee answers Kim's challenge: refused, and the challenge is spent.
    let (status, _) = call(&app, "POST", "/api/auth/passkeys/register/finish", Some(&lee), body.clone()).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = call(&app, "POST", "/api/auth/passkeys/register/finish", Some(&kim), body).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "used up");
    assert!(world.account("kim").mfa.passkeys.is_empty() && world.account("lee").mfa.passkeys.is_empty());

    // A sign-in challenge registers nothing, and a registration made elsewhere is refused.
    let (_, login) = call(&app, "POST", "/api/auth/passkeys/login/begin", None, json!({})).await;
    let (_, begun) = call(&app, "POST", "/api/auth/passkeys/register/begin", Some(&kim), json!({ "password": PASSWORD })).await;
    let credential = browser.create(&begun["data"]["publicKey"]);
    let body = json!({ "challenge_id": login["data"]["challenge_id"], "name": "x", "credential": credential });
    assert_eq!(call(&app, "POST", "/api/auth/passkeys/register/finish", Some(&kim), body).await.0, StatusCode::BAD_REQUEST);
    let mut elsewhere = Browser::at("https://sub.man.example.com");
    let (_, begun) = call(&app, "POST", "/api/auth/passkeys/register/begin", Some(&kim), json!({ "password": PASSWORD })).await;
    let body = json!({ "challenge_id": begun["data"]["challenge_id"], "name": "x", "credential": elsewhere.create(&begun["data"]["publicKey"]) });
    let (status, answer) = call(&app, "POST", "/api/auth/passkeys/register/finish", Some(&kim), body).await;
    assert_eq!((status, answer["message"].as_str()), (StatusCode::BAD_REQUEST, Some("Passkey could not be verified")));

    // A password changed between the two steps voids the registration.
    let (_, begun) = call(&app, "POST", "/api/auth/passkeys/register/begin", Some(&kim), json!({ "password": PASSWORD })).await;
    let body = json!({ "challenge_id": begun["data"]["challenge_id"], "name": "x", "credential": browser.create(&begun["data"]["publicKey"]) });
    world.update("kim", |u| u.password_hash = bcrypt::hash("another password", 4).unwrap());
    let (status, _) = call(&app, "POST", "/api/auth/passkeys/register/finish", Some(&kim), body).await;
    assert_ne!(status, StatusCode::OK);
    assert!(world.account("kim").mfa.passkeys.is_empty());
}

#[actix_web::test]
async fn only_the_two_sign_in_steps_are_open_and_every_account_reaches_its_own() {
    let mut member = user("u3", "park", "remote");
    member.scope_restricted = true;
    let mut scoped = user("u4", "choi", "operator");
    scoped.scope_restricted = true;
    let world = World::with(vec![user("u1", "kim", "admin"), user("u2", "lee", "viewer"), member, scoped]);
    let app = serve!(world);

    for (method, path) in [
        ("GET", "/api/auth/passkeys"),
        ("POST", "/api/auth/passkeys/register/begin"),
        ("POST", "/api/auth/passkeys/register/finish"),
        ("POST", "/api/auth/passkeys/rename"),
        ("POST", "/api/auth/passkeys/delete"),
    ] {
        let (status, _) = call(&app, method, path, None, json!({})).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{method} {path} without signing in");
    }
    assert!(crate::auth::public_route("POST", "/api/auth/passkeys/login/begin"));
    assert!(crate::auth::public_route("POST", "/api/auth/passkeys/login/finish"));
    assert!(!crate::auth::public_route("GET", "/api/auth/passkeys/login/begin"));
    assert!(!crate::auth::public_route("POST", "/api/auth/passkeys/login/finish/"));
    assert!(!crate::auth::public_route("POST", "/api/auth/passkeys/register/begin"));

    for name in ["kim", "lee", "park", "choi"] {
        let mut browser = Browser::at(ORIGIN);
        let token = password_token(&app, name).await;
        let id = register(&app, &mut browser, &token, name).await;
        let (status, list) = call(&app, "GET", "/api/auth/passkeys", Some(&token), Value::Null).await;
        assert_eq!((status, list["data"]["passkeys"].as_array().map(Vec::len)), (StatusCode::OK, Some(1)), "{name}");
        let (status, body) = sign_in(&app, &mut browser, &id).await;
        assert_eq!((status, body["data"]["username"].as_str()), (StatusCode::OK, Some(name)));
    }
}

#[actix_web::test]
async fn an_account_without_a_local_password_or_past_the_limit_registers_none() {
    let world = World::with(vec![user("u1", "kim", "operator"), user("u2", "sso", "viewer")]);
    world.update("sso", |u| u.password_hash = String::new());
    let app = serve!(world);
    let mut browser = Browser::at(ORIGIN);
    let token = password_token(&app, "kim").await;
    for n in 0..crate::passkeys::MAX_PER_ACCOUNT {
        register(&app, &mut browser, &token, &format!("key {n}")).await;
    }
    let (status, body) = call(&app, "POST", "/api/auth/passkeys/register/begin", Some(&token), json!({ "password": PASSWORD })).await;
    assert_eq!((status, body["message"].as_str()), (StatusCode::BAD_REQUEST, Some("Too many passkeys")));

    // Signed in some other way (a session without a local password): no passkeys.
    crate::jwt_sessions::register_session(&world.sessions, "sso-session", "sso", "u2", "");
    let now = chrono::Utc::now().timestamp() as usize;
    let claims = crate::auth::Claims { sub: "sso".into(), role: "viewer".into(), sid: "sso-session".into(), iat: now, exp: now + 300 };
    let sso = jsonwebtoken::encode(&jsonwebtoken::Header::default(), &claims, &jsonwebtoken::EncodingKey::from_secret(b"test-secret")).unwrap();
    let (_, list) = call(&app, "GET", "/api/auth/passkeys", Some(&sso), Value::Null).await;
    assert_eq!(list["data"]["local_password_available"], false);
    let (status, _) = call(&app, "POST", "/api/auth/passkeys/register/begin", Some(&sso), json!({ "password": "" })).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
