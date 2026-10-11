use super::*;
use actix_web::{http::StatusCode, test, App};
fn attempt(path: &str, user: &str, peer: &str) -> test::TestRequest {
    test::TestRequest::post()
        .uri(path)
        .peer_addr(peer.parse().unwrap())
        .set_json(serde_json::json!({"username":user,"password":"fixture"}))
}
#[actix_web::test]
async fn account_limit_crosses_routes_sources_and_forged_forwarding() {
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
        let response = test::call_service(
            &app,
            attempt(path, "account", &format!("127.0.0.{}:1000", i + 1)).to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
    let response = test::call_service(
        &app,
        attempt("/api/auth/ldap/login", "ACCOUNT", "127.0.0.9:1000")
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
    // One attacked account does not exhaust other accounts behind the same proxy.
    assert_eq!(
        test::call_service(
            &app,
            attempt("/api/auth/login", "different", "127.0.0.1:1000").to_request()
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
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
async fn missing_local_username_cannot_bypass_admin_account_limit() {
    let app = test::init_service(
        App::new()
            .wrap(RateLimiter::new())
            .default_service(web::to(|| async { HttpResponse::Unauthorized().finish() })),
    )
    .await;
    for _ in 0..5 {
        assert_eq!(
            test::call_service(
                &app,
                test::TestRequest::post()
                    .uri("/api/auth/login")
                    .set_json(serde_json::json!({"password":"fixture"}))
                    .to_request()
            )
            .await
            .status(),
            StatusCode::UNAUTHORIZED
        );
    }
    assert_eq!(
        test::call_service(
            &app,
            attempt("/api/auth/login", "admin", "127.0.0.2:1000").to_request()
        )
        .await
        .status(),
        StatusCode::TOO_MANY_REQUESTS
    );
}
