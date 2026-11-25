use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use actix_web::{web, App, HttpResponse, HttpServer};

use super::layout_tests::SAMPLE;
use super::prometheus::QUERIES;
use super::status::Level;
use super::status_tests::canned;
use super::*;
use crate::models::{NabimanUser, UserRole};

/// A stand-in Prometheus on a local port: answers the fixed queries with
/// the canned series, counts what it is asked, and can be switched to fail.
struct Fake {
    url: String,
    asked: Arc<AtomicUsize>,
    unknown_queries: Arc<AtomicUsize>,
    broken: Arc<AtomicBool>,
    handle: actix_web::dev::ServerHandle,
}

async fn fake() -> Fake {
    let (asked, unknown_queries, broken) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)), Arc::new(AtomicBool::new(false)));
    let state = (asked.clone(), unknown_queries.clone(), broken.clone());
    let server = HttpServer::new(move || {
        let (asked, unknown, broken) = state.clone();
        App::new().route(
            "/api/v1/query",
            web::get().to(move |q: web::Query<HashMap<String, String>>| {
                asked.fetch_add(1, Ordering::SeqCst);
                let known = QUERIES.iter().find(|(_, text)| q.get("query").is_some_and(|asked| asked == text));
                let response = match known {
                    _ if broken.load(Ordering::SeqCst) => HttpResponse::ServiceUnavailable().body("down"),
                    Some((metric, _)) => HttpResponse::Ok().content_type("application/json").body(canned(*metric)),
                    None => {
                        unknown.fetch_add(1, Ordering::SeqCst);
                        HttpResponse::BadRequest().body("unexpected query")
                    }
                };
                async move { response }
            }),
        )
    })
    .workers(1)
    .bind(("127.0.0.1", 0))
    .unwrap();
    let url = format!("http://{}", server.addrs()[0]);
    let server = server.run();
    let handle = server.handle();
    actix_web::rt::spawn(server);
    Fake { url, asked, unknown_queries, broken, handle }
}

fn scratch() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("nabiman-rack-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn layout_file(text: &str) -> (PathBuf, PathBuf) {
    let dir = scratch();
    let file = dir.join("layout.json");
    std::fs::write(&file, text).unwrap();
    (dir, file)
}

#[actix_web::test]
async fn status_comes_from_the_fixed_queries_and_is_kept_for_a_while() {
    let prometheus = fake().await;
    let (dir, file) = layout_file(SAMPLE);
    let service = Service::new(Some(file), Some(prometheus.url.clone()), Duration::from_secs(60));
    let layout = service.layout().unwrap();
    let first = service.status(&layout).await;
    assert_eq!((first.source, first.summary.nodes_ready, first.summary.gpus), ("ok", 4, 5));
    assert_eq!(first.nodes["mec-wn02"].gpus[0].util_pct, Some(35.0));
    assert_eq!(first.devices["lenovo-1"].level, Level::Ok);
    assert_eq!(prometheus.asked.load(Ordering::SeqCst), QUERIES.len(), "one request per metric, not per device");
    assert_eq!(prometheus.unknown_queries.load(Ordering::SeqCst), 0, "only the fixed queries are sent, spelled as in the code");
    // Many pages asking again, at once and later, cost Prometheus nothing more.
    let again = futures::future::join_all((0..8).map(|_| service.status(&layout))).await;
    assert!(again.iter().all(|s| *s == first));
    assert_eq!(prometheus.asked.load(Ordering::SeqCst), QUERIES.len());
    prometheus.handle.stop(false).await;
    let _ = std::fs::remove_dir_all(dir);
}

#[actix_web::test]
async fn the_answers_are_asked_for_again_once_they_are_old() {
    let prometheus = fake().await;
    let (dir, file) = layout_file(SAMPLE);
    let service = Service::new(Some(file), Some(format!("{}/", prometheus.url)), Duration::from_millis(50));
    let layout = service.layout().unwrap();
    assert_eq!(service.status(&layout).await.source, "ok");
    tokio::time::sleep(Duration::from_millis(80)).await;
    prometheus.broken.store(true, Ordering::SeqCst);
    let failed = service.status(&layout).await;
    assert_eq!(prometheus.asked.load(Ordering::SeqCst), 2 * QUERIES.len());
    assert_eq!((failed.source, failed.message.as_deref()), ("unreachable", Some("HTTP 503")));
    assert!(failed.nodes.values().all(|n| n.level == Level::Unknown), "no stale numbers are passed off as current");
    prometheus.handle.stop(false).await;
    let _ = std::fs::remove_dir_all(dir);
}

#[actix_web::test]
async fn an_unreachable_or_misnamed_prometheus_is_a_message_not_an_error() {
    let (dir, file) = layout_file(SAMPLE);
    // A port nothing listens on.
    let closed = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap().local_addr().unwrap();
    let service = Service::new(Some(file.clone()), Some(format!("http://{closed}")), Duration::from_secs(60));
    let answer = status_answer(&service).await;
    let status = answer.status.expect("a status all the same");
    assert!(answer.configured && answer.error.is_none());
    assert_eq!((status.source, status.message.as_deref()), ("unreachable", Some("cannot connect")));
    assert_eq!((status.devices["t4"].level, status.devices["sw"].level), (Level::Unknown, Level::None));

    for bad in ["prometheus:9090", "ftp://host", "http://user:pw@host", "http://host/?query=up"] {
        let service = Service::new(Some(file.clone()), Some(bad.into()), Duration::from_secs(60));
        let status = status_answer(&service).await.status.unwrap();
        assert_eq!(status.source, "unreachable", "{bad}");
        assert!(status.message.unwrap().starts_with("rack_prometheus_url:"), "{bad}");
    }

    let only_layout = Service::new(Some(file), None, Duration::from_secs(60));
    let status = status_answer(&only_layout).await.status.unwrap();
    assert_eq!((status.source, status.devices["t4"].level), ("not_configured", Level::Unknown));
    let _ = std::fs::remove_dir_all(dir);
}

#[actix_web::test]
async fn with_neither_setting_the_feature_is_off() {
    let none = |_: &str| None;
    assert!(!enabled_in(&none));
    let service = Service::from_settings(&none);
    let layout = serde_json::to_value(layout_answer(&service)).unwrap();
    assert_eq!(layout, serde_json::json!({"configured": false, "layout": null, "error": null}));
    let status = serde_json::to_value(status_answer(&service).await).unwrap();
    assert_eq!(status, serde_json::json!({"configured": false, "status": null, "error": null}));
    let blank = |name: &str| (name == "NABIMAN_RACK_LAYOUT_FILE").then(|| "  ".to_string());
    assert!(!enabled_in(&blank), "set to nothing is not set");
    let file_only = |name: &str| (name == "NABIMAN_RACK_LAYOUT_FILE").then(|| "/x.json".to_string());
    let url_only = |name: &str| (name == "NABIMAN_RACK_PROMETHEUS_URL").then(|| "http://p:9090".to_string());
    assert!(enabled_in(&file_only) && enabled_in(&url_only));
    // Prometheus alone: on, with nothing to draw, and it says which setting is missing.
    let answer = layout_answer(&Service::from_settings(&url_only));
    assert_eq!((answer.configured, answer.error.map(|e| e.code)), (true, Some("no_file")));
}

#[actix_web::test]
async fn a_missing_or_broken_file_is_reported_and_read_again_when_it_changes() {
    let dir = scratch();
    let file = dir.join("layout.json");
    let service = Service::new(Some(file.clone()), None, Duration::from_secs(60));
    let fault = service.layout().unwrap_err();
    assert_eq!(fault.code, "unreadable");
    assert!(fault.detail.contains("layout.json"), "{}", fault.detail);

    std::fs::write(&file, "{ not json").unwrap();
    let fault = service.layout().unwrap_err();
    assert_eq!(fault.code, "invalid");
    assert!(fault.detail.contains("not a rack layout"), "{}", fault.detail);
    let answer = status_answer(&service).await;
    assert!(answer.configured && answer.status.is_none());
    assert_eq!(answer.error.unwrap().code, "invalid");

    // Mended while the server runs: the next request sees it.
    std::fs::write(&file, SAMPLE).unwrap();
    let first = service.layout().unwrap();
    assert_eq!(first.version, "t1");
    assert!(Arc::ptr_eq(&first, &service.layout().unwrap()), "an unchanged file is not parsed again");

    std::fs::write(&file, SAMPLE.replace(r#""version": "t1""#, r#""version": "t2-longer""#)).unwrap();
    assert_eq!(service.layout().unwrap().version, "t2-longer");

    std::fs::remove_file(&file).unwrap();
    assert_eq!(service.layout().unwrap_err().code, "unreadable");
    std::fs::create_dir(&file).unwrap();
    assert_eq!(service.layout().unwrap_err().code, "unreadable");
    let _ = std::fs::remove_dir_all(dir);
}

#[actix_web::test]
async fn a_link_moved_to_a_newer_file_is_followed() {
    let dir = scratch();
    let (v1, v2, link) = (dir.join("v1.json"), dir.join("v2.json"), dir.join("current.json"));
    // Same size, and possibly the same second: only where the link leads differs.
    std::fs::write(&v1, SAMPLE).unwrap();
    std::fs::write(&v2, SAMPLE.replace(r#""version": "t1""#, r#""version": "t2""#)).unwrap();
    std::os::unix::fs::symlink("v1.json", &link).unwrap();
    let service = Service::new(Some(link.clone()), None, Duration::from_secs(60));
    assert_eq!(service.layout().unwrap().version, "t1");
    std::fs::remove_file(&link).unwrap();
    std::os::unix::fs::symlink("v2.json", &link).unwrap();
    assert_eq!(service.layout().unwrap().version, "t2");
    let _ = std::fs::remove_dir_all(dir);
}

#[actix_web::test]
async fn the_routes_answer_and_are_never_cached() {
    let app = actix_web::test::init_service(App::new().configure(config)).await;
    for path in ["/api/rack/layout", "/api/rack/status"] {
        let res = actix_web::test::call_service(&app, actix_web::test::TestRequest::get().uri(path).to_request()).await;
        assert!(res.status().is_success(), "{path}");
        assert_eq!(res.headers().get("cache-control").unwrap(), "no-store");
        let body: serde_json::Value = actix_web::test::read_body_json(res).await;
        assert!(body["success"] == true && body["data"]["configured"].is_boolean(), "{path}: {body}");
        // Nothing but GET is served: there is nothing to change.
        for method in [actix_web::http::Method::POST, actix_web::http::Method::PUT, actix_web::http::Method::DELETE] {
            let res = actix_web::test::call_service(&app, actix_web::test::TestRequest::default().method(method.clone()).uri(path).to_request()).await;
            assert!(res.status().is_client_error(), "{method} {path}");
        }
    }
}

#[test]
fn staff_may_read_and_members_may_not() {
    let member: NabimanUser = serde_json::from_value(serde_json::json!({
        "id": "rack-member", "username": "rack-member", "password_hash": "", "role": "remote", "created_at": "now", "status": "active"
    }))
    .unwrap();
    for path in ["/api/rack/layout", "/api/rack/status"] {
        for role in [UserRole::Admin, UserRole::Operator, UserRole::Viewer] {
            assert!(crate::rbac::check_permission(&role, "GET", path), "{role:?} GET {path}");
        }
        for method in ["POST", "PUT", "DELETE", "PATCH"] {
            assert!(!crate::rbac::check_permission(&UserRole::Viewer, method, path), "viewer {method} {path}");
        }
        assert!(!crate::rbac::check_permission(&UserRole::Remote, "GET", path));
        assert!(!crate::route_access::member_reachable("GET", path) && !crate::member_servers::permitted(&member, "GET", path), "{path}");
        assert!(!crate::scope_access::every_account("GET", path), "{path}: not among what every scoped account gets");
        // The same class as the MEC status pages.
        let (rack, mec) = (crate::route_access::lookup(path).unwrap(), crate::route_access::lookup("/api/mec/v1/health/summary").unwrap());
        assert_eq!((rack.class, rack.member), (mec.class, mec.member));
    }
    assert!(!crate::rbac::check_permission(&UserRole::Operator, "GET", "/api/rack/other"), "nothing else under /api/rack is open");
}
