use actix_web::http::StatusCode;
use actix_web::App;

use super::edit::failed;
use super::moves::Refusal;
use super::save::Failure;
use crate::models::{NabimanUser, UserRole};
use crate::route_access::{Class, Member};

const ROUTE: &str = "/api/rack/layout/moves";

#[test]
fn only_administrators_may_save_moves() {
    let route = crate::route_access::lookup(ROUTE).unwrap();
    assert_eq!((route.class, route.member), (Class::Admin, Member::No));
    for method in ["GET", "POST", "PUT", "DELETE", "PATCH"] {
        assert_eq!(crate::rbac::check_permission(&UserRole::Admin, method, ROUTE), true, "admin {method}");
        for role in [UserRole::Operator, UserRole::Viewer, UserRole::Remote] {
            assert!(!crate::rbac::check_permission(&role, method, ROUTE), "{role:?} {method}");
        }
        assert!(!crate::route_access::member_reachable(method, ROUTE), "member {method}");
        assert!(!crate::scope_access::every_account(method, ROUTE), "not among what every account gets");
    }
    let account = |role: &str, scoped: bool| -> NabimanUser {
        serde_json::from_value(serde_json::json!({
            "id": format!("rack-{role}"), "username": format!("rack-{role}"), "password_hash": "", "role": role, "created_at": "now", "status": "active",
            "scope_restricted": scoped
        }))
        .unwrap()
    };
    assert!(!crate::member_servers::permitted(&account("remote", false), "POST", ROUTE));
    // An account limited to a scope, whatever its role there.
    for role in ["operator", "viewer"] {
        assert!(!crate::scope_access::permission(&account(role, true), "POST", ROUTE), "scoped {role}");
        assert!(!crate::scope_access::permission(&account(role, false), "POST", ROUTE), "{role}");
    }
}

#[actix_web::test]
async fn a_caller_who_is_not_a_signed_in_administrator_is_refused_by_the_handler_too() {
    // No sign-in was verified for this request (the middleware is not in this app): the handler itself refuses.
    let app = actix_web::test::init_service(App::new().configure(super::config)).await;
    let body = serde_json::json!({ "base_version": "t1", "moves": [{ "id": "sw", "rack": "R1", "u": 40 }] });
    let res = actix_web::test::call_service(&app, actix_web::test::TestRequest::post().uri(ROUTE).set_json(&body).to_request()).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    for method in [actix_web::http::Method::GET, actix_web::http::Method::PUT, actix_web::http::Method::DELETE] {
        let res = actix_web::test::call_service(&app, actix_web::test::TestRequest::default().method(method.clone()).uri(ROUTE).to_request()).await;
        assert!(res.status().is_client_error(), "{method}");
    }
    let res = actix_web::test::call_service(&app, actix_web::test::TestRequest::post().uri(ROUTE).set_json(serde_json::json!({ "moves": "all" })).to_request()).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST, "a body of another shape");
}

async fn answer(failure: Failure) -> (StatusCode, serde_json::Value) {
    let res = failed(&failure);
    let status = res.status();
    assert_eq!(res.headers().get("cache-control").unwrap(), "no-store");
    let bytes = actix_web::body::to_bytes(res.into_body()).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[actix_web::test]
async fn each_failure_has_its_own_answer() {
    let text = |value: &serde_json::Value| value.as_str().unwrap_or_default().to_string();
    let (status, body) = answer(Failure::Conflict { current: "20261125_v7".into() }).await;
    assert_eq!((status, text(&body["code"]).as_str(), text(&body["data"]["current_version"]).as_str(), body["success"].as_bool()), (StatusCode::CONFLICT, "version_conflict", "20261125_v7", Some(false)));
    assert!(text(&body["message"]).contains("20261125_v7"));
    let (status, body) = answer(Failure::Refused(Refusal { reason: "overlap", detail: "sw: U26–U27 of R1 is taken by lenovo-1".into() })).await;
    assert_eq!((status, text(&body["data"]["code"]).as_str(), text(&body["data"]["reason"]).as_str()), (StatusCode::BAD_REQUEST, "invalid_move", "overlap"));
    assert!(text(&body["message"]).contains("lenovo-1"));
    let (status, body) = answer(Failure::Off).await;
    assert_eq!((status, text(&body["code"]).as_str()), (StatusCode::SERVICE_UNAVAILABLE, "not_configured"));
    let (status, body) = answer(Failure::Unusable { code: "invalid", detail: "racks[0].units: 0 given".into() }).await;
    assert_eq!((status, text(&body["code"]).as_str(), text(&body["data"]["fault"]).as_str()), (StatusCode::UNPROCESSABLE_ENTITY, "layout_unusable", "invalid"));
    let (status, body) = answer(Failure::Storage("cannot create a new layout file in /x: Permission denied".into())).await;
    assert_eq!((status, text(&body["code"]).as_str()), (StatusCode::INTERNAL_SERVER_ERROR, "write_failed"));
    assert!(text(&body["message"]).contains("Permission denied"));
}

#[test]
fn saves_are_counted_for_each_account() {
    let recent = Default::default();
    let start = std::time::Instant::now();
    for _ in 0..super::edit::MAX_SAVES {
        assert!(super::edit::admitted(&recent, "admin", start));
    }
    assert!(!super::edit::admitted(&recent, "admin", start), "one too many in a minute");
    assert!(super::edit::admitted(&recent, "other", start), "another account has its own count");
    assert!(super::edit::admitted(&recent, "admin", start + super::edit::WINDOW), "a minute later there is room again");
}
