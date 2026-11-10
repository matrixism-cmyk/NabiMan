use super::*;
use actix_web::{http::StatusCode, test, App};
fn attempt(path: &str, user: &str, peer: &str) -> test::TestRequest {
    test::TestRequest::post()
        .uri(path)
        .peer_addr(peer.parse().unwrap())
        .set_json(serde_json::json!({"username":user,"password":"fixture"}))
}
#[actix_web::test]
async fn account_limit_crosses_routes_and_ignores_forwarding_from_untrusted_peers() {
    let app = test::init_service(
        App::new()
            .wrap(RateLimiter::new())
            .default_service(web::to(|| async { HttpResponse::Unauthorized().finish() })),
    )
    .await;
    for i in 0..5 {
        let path = if i % 2 == 0 {
            "/api/auth/login"
        } else {
            "/api/auth/2fa/verify"
        };
        let response = test::call_service(&app, attempt(path, "account", "198.51.100.7:1000").to_request()).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
    // Not a trusted proxy: its X-Forwarded-For does not make it someone else.
    let response = test::call_service(
        &app,
        attempt("/api/auth/ldap/login", "ACCOUNT", "198.51.100.7:1001")
            .insert_header(("X-Forwarded-For", "203.0.113.45"))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    let retry: u64 = response
        .headers()
        .get("Retry-After")
        .unwrap()
        .to_str()
        .unwrap()
        .parse()
        .unwrap();
    assert!((1..=300).contains(&retry));
    assert_eq!(response.headers().get("Cache-Control").unwrap(), "no-store");
    for (user, peer) in [("different", "198.51.100.7:1000"), ("account", "192.0.2.10:1000")] {
        // Another account from there, and the account's owner from elsewhere, still get through.
        let status = test::call_service(&app, attempt("/api/auth/login", user, peer).to_request()).await.status();
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{user} from {peer}");
    }
}
#[actix_web::test]
async fn behind_the_local_proxy_each_client_has_its_own_budget() {
    let app = test::init_service(
        App::new()
            .wrap(RateLimiter::new())
            .default_service(web::to(|| async { HttpResponse::Unauthorized().finish() })),
    )
    .await;
    let via_proxy = |user: &str, chain: &str| {
        attempt("/api/auth/login", user, "127.0.0.1:5000").insert_header(("X-Forwarded-For", chain.to_string()))
    };
    for i in 0..5 {
        // The attacker varies the forged left entry; nginx appended the real one.
        let chain = format!("10.9.9.{i}, 203.0.113.66");
        assert_eq!(test::call_service(&app, via_proxy("admin", &chain).to_request()).await.status(), StatusCode::UNAUTHORIZED);
    }
    let blocked = test::call_service(&app, via_proxy("admin", "1.1.1.1, 203.0.113.66").to_request()).await;
    assert_eq!(blocked.status(), StatusCode::TOO_MANY_REQUESTS);
    let owner = test::call_service(&app, via_proxy("admin", "198.51.100.20").to_request()).await;
    assert_eq!(owner.status(), StatusCode::UNAUTHORIZED, "the same proxy, another client");
}
#[actix_web::test]
async fn successful_requests_preserve_payload_but_network_budget_still_applies() {
    let app = test::init_service(App::new().wrap(RateLimiter::new()).route(
        "/api/auth/login",
        web::post().to(|b: web::Json<serde_json::Value>| async move {
            assert_eq!(b["password"], "fixture");
            HttpResponse::Ok().finish()
        }),
    ))
    .await;
    for _ in 0..60 {
        assert_eq!(
            test::call_service(
                &app,
                attempt("/api/auth/login", "u", "127.0.0.1:1000").to_request()
            )
            .await
            .status(),
            StatusCode::OK
        );
    }
    let response = test::call_service(
        &app,
        attempt("/api/auth/login", "v", "127.0.0.1:1000")
            .insert_header(("Forwarded", "for=1.2.3.4"))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
}
#[actix_web::test]
async fn registration_has_separate_source_budget_and_cannot_lock_login() {
    let app = test::init_service(
        App::new()
            .wrap(RateLimiter::new())
            .default_service(web::to(|| async { HttpResponse::Ok().finish() })),
    )
    .await;
    for i in 0..5 {
        assert_eq!(
            test::call_service(
                &app,
                attempt("/api/auth/register", &format!("u{i}"), "127.0.0.1:1000").to_request()
            )
            .await
            .status(),
            StatusCode::OK
        );
    }
    assert_eq!(
        test::call_service(
            &app,
            attempt("/api/auth/register", "new", "127.0.0.1:1000").to_request()
        )
        .await
        .status(),
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(
        test::call_service(
            &app,
            attempt("/api/auth/login", "u", "127.0.0.1:1000").to_request()
        )
        .await
        .status(),
        StatusCode::OK
    );
}
#[actix_web::test]
async fn oversized_or_ambiguous_payloads_do_not_reach_authentication() {
    let app = test::init_service(App::new().wrap(RateLimiter::new()).default_service(web::to(
        || async {
            panic!("Invalid payload reached authentication");
            #[allow(unreachable_code)]
            HttpResponse::Ok().finish()
        },
    )))
    .await;
    for body in [
        "a".repeat(32769),
        r#"{"username":"a","username":"b"}"#.into(),
        "{bad".into(),
    ] {
        assert_eq!(
            test::call_service(
                &app,
                test::TestRequest::post()
                    .uri("/api/auth/login")
                    .set_payload(body)
                    .to_request()
            )
            .await
            .status(),
            StatusCode::BAD_REQUEST
        );
    }
}
#[actix_web::test]
async fn a_password_sign_in_without_a_username_is_refused() {
    let app = test::init_service(App::new().wrap(RateLimiter::new()).default_service(web::to(
        || async {
            panic!("A sign-in without a username reached authentication");
            #[allow(unreachable_code)]
            HttpResponse::Ok().finish()
        },
    )))
    .await;
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/auth/login")
            .set_json(serde_json::json!({"password":"fixture"}))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
