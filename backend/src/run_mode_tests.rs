use super::*;
use actix_web::{test as web_test, web, App};

fn lookup(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
    move |name| pairs.iter().find(|(k, _)| *k == name).map(|(_, v)| v.to_string())
}

#[test]
fn the_mode_is_root_unless_service_is_asked_for() {
    assert_eq!(mode_in(&lookup(&[])), Ok(RunMode::Root));
    assert_eq!(mode_in(&lookup(&[("NABIMAN_RUN_MODE", " ")])), Ok(RunMode::Root));
    assert_eq!(mode_in(&lookup(&[("NABIMAN_RUN_MODE", "root")])), Ok(RunMode::Root));
    assert_eq!(mode_in(&lookup(&[("NABIMAN_RUN_MODE", "Service")])), Ok(RunMode::Service));
    assert!(mode_in(&lookup(&[("NABIMAN_RUN_MODE", "nabiman")])).is_err());
}

#[test]
fn service_mode_refuses_to_start_as_root_and_root_mode_as_a_user_only_notes_it() {
    assert!(decide(RunMode::Service, 0).is_err());
    assert_eq!(decide(RunMode::Service, 990), Ok(None));
    assert_eq!(decide(RunMode::Root, 0), Ok(None));
    assert!(decide(RunMode::Root, 1000).unwrap().unwrap().contains("run_mode = service"));
}

#[test]
fn root_mode_refuses_nothing() {
    for (method, path) in [(Method::POST, "/api/accounts"), (Method::POST, "/api/preview/office"), (Method::DELETE, "/api/vhost/config/x")] {
        assert!(refused(RunMode::Root, &method, path).is_none(), "{path}");
    }
}

#[test]
fn service_mode_refuses_root_work_the_helper_does_not_do() {
    let key = |method: Method, path: &str| refused(RunMode::Service, &method, path).map(|f| f.key);
    assert_eq!(key(Method::POST, "/api/accounts"), Some("os_accounts"));
    assert_eq!(key(Method::DELETE, "/api/accounts/alice"), Some("os_accounts"));
    assert_eq!(key(Method::POST, "/api/packages/install"), Some("package_install"));
    assert_eq!(key(Method::POST, "/api/config/nginx/restart"), Some("config_files"));
    assert_eq!(key(Method::POST, "/api/ssl/issue"), Some("certificates"));
    assert_eq!(key(Method::GET, "/api/preview/office"), Some("office_preview"), "every method");
    assert_eq!(key(Method::POST, "/api/database/query"), Some("databases"));
    // Reading still works; so does what the helper does, and look-alike paths.
    assert_eq!(key(Method::GET, "/api/accounts"), None);
    assert_eq!(key(Method::GET, "/api/config/services"), None);
    assert_eq!(key(Method::POST, "/api/packages/search"), None);
    assert_eq!(key(Method::POST, "/api/firewall/add"), None);
    assert_eq!(key(Method::POST, "/api/updates/upgrade"), None);
    assert_eq!(key(Method::POST, "/api/services/action"), None);
    assert_eq!(key(Method::POST, "/api/accountsx"), None);
    assert_eq!(key(Method::POST, "/api/config/"), None);
}

async fn call(mode: RunMode, method: Method, path: &str) -> (u16, serde_json::Value) {
    let app = web_test::init_service(
        App::new()
            .app_data(web::Data::new(mode))
            .wrap(actix_web::middleware::from_fn(gate))
            .default_service(web::to(|| async { HttpResponse::Ok().json(serde_json::json!({ "success": true })) })),
    )
    .await;
    let res = web_test::call_service(&app, web_test::TestRequest::default().method(method).uri(path).to_request()).await;
    let status = res.status().as_u16();
    (status, web_test::read_body_json(res).await)
}

#[actix_web::test]
async fn a_root_feature_in_service_mode_answers_unavailable_and_in_root_mode_runs() {
    let (status, body) = call(RunMode::Service, Method::POST, "/api/accounts").await;
    assert_eq!(status, 503);
    assert_eq!(body["success"], false);
    assert_eq!(body["data"]["feature"], "os_accounts");
    assert_eq!(body["data"]["unavailable"], true);
    let message = body["message"].as_str().unwrap();
    assert!(message.contains("not available") && message.contains("useradd"), "{message}");

    let (status, body) = call(RunMode::Root, Method::POST, "/api/accounts").await;
    assert_eq!((status, body["success"].as_bool()), (200, Some(true)));
    let (status, _) = call(RunMode::Service, Method::GET, "/api/accounts").await;
    assert_eq!(status, 200);
}

#[test]
fn the_report_says_what_each_mode_does() {
    let status = |report: &Report, key: &str| report.features.iter().find(|f| f.key == key).map(|f| f.status);
    let service = report_for(RunMode::Service);
    assert_eq!(status(&service, "firewall"), Some(Status::ViaHelper));
    assert_eq!(status(&service, "office_preview"), Some(Status::Unavailable));
    assert_eq!(status(&service, "local_terminal"), Some(Status::Limited));
    let root = report_for(RunMode::Root);
    assert_eq!(status(&root, "firewall"), Some(Status::Available));
    assert_eq!(status(&root, "service_control"), Some(Status::Unavailable));
    assert_eq!(serde_json::to_value(&root).unwrap()["run_mode"], "root");
}

#[test]
fn every_refused_route_is_a_registered_route_and_only_unavailable_features_refuse() {
    let doc = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../docs/security/route-access.md")).unwrap();
    let routes: Vec<&str> = doc.lines().filter_map(|l| l.strip_prefix("| `")).filter_map(|l| l.split('`').next()).collect();
    for feature in FEATURES {
        assert_eq!(feature.routes.is_empty(), feature.service != Status::Unavailable || feature.key == "terminal_as_user", "{}", feature.key);
        for (_, route) in feature.routes {
            let known = match route.strip_suffix('/') {
                Some(dir) => routes.iter().any(|r| r.starts_with(&format!("{dir}/"))),
                None => routes.contains(route),
            };
            assert!(known, "{route} ({}) is not a registered route", feature.key);
        }
    }
}
