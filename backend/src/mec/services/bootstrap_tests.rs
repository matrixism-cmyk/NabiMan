use super::*;
use crate::mec::services::axgate_service::AxgateService;
use crate::mec::services::harbor_service::HarborService;
use crate::mec::services::rancher_service::RancherService;
use crate::mec::services::ServiceError;
use std::sync::atomic::{AtomicBool, Ordering};

fn unavailable<T: std::fmt::Debug>(result: super::super::ServiceResult<T>) {
    let err = result.unwrap_err();
    assert!(matches!(err, ServiceError::Unavailable(_)));
    assert_eq!(err.http_status(), 503);
}

#[test]
fn mode_requires_explicit_demo_and_rejects_typos() {
    assert_eq!(ServiceMode::parse(None).unwrap(), ServiceMode::Auto);
    assert_eq!(ServiceMode::parse(Some("auto")).unwrap(), ServiceMode::Auto);
    assert_eq!(ServiceMode::parse(Some("real")).unwrap(), ServiceMode::Real);
    assert_eq!(ServiceMode::parse(Some("mock")).unwrap(), ServiceMode::Mock);
    assert!(ServiceMode::parse(Some("production")).is_err());
    assert!(ServiceMode::parse(Some("")).is_err());
}

#[tokio::test]
async fn demo_mode_remains_available_when_explicit() {
    let bundle = bootstrap(ServiceMode::Mock).await;
    assert!(!bundle.kube.list_nodes().await.unwrap().is_empty());
    assert!(!bundle.axgate.list_public_ips().await.unwrap().is_empty());
    assert!(bundle
        .kube
        .health_check()
        .await
        .unwrap()
        .endpoint
        .starts_with("mock://"));
}

#[tokio::test]
async fn invalid_kube_config_cannot_return_demo_inventory_or_accept_changes() {
    let client = kube_from_config(KubeRealConfig {
        kubeconfig_path: Some(format!("/nonexistent/{}.yaml", uuid::Uuid::new_v4())),
        default_namespace: "default".into(),
    })
    .await;
    unavailable(client.list_nodes().await);
    unavailable(client.create_namespace("customer", &[]).await);
    unavailable(client.health_check().await);
}

#[tokio::test]
async fn disconnected_providers_reject_both_read_and_write() {
    let service = UnavailableService::new("connector", "configuration missing");
    unavailable(AxgateService::list_public_ips(&service).await);
    unavailable(AxgateService::delete_nat_rule(&service, "rule").await);
    unavailable(AxgateService::list_vpn_sessions(&service).await);
    unavailable(AxgateService::health_check(&service).await);
    unavailable(RancherService::list_users(&service).await);
    unavailable(RancherService::create_user(&service, "user", "not-a-secret").await);
    unavailable(RancherService::health_check(&service).await);
    unavailable(HarborService::list_projects(&service).await);
    unavailable(HarborService::create_project(&service, "project", false).await);
    unavailable(HarborService::health_check(&service).await);
}

#[actix_web::test]
async fn real_kube_auth_failure_recovers_without_becoming_demo() {
    use actix_web::{web, App, HttpResponse};
    let recovered = Arc::new(AtomicBool::new(false));
    let flag = recovered.clone();
    let server = actix_test::start(move || {
        let flag = flag.clone();
        App::new().default_service(web::to(move || {
            let flag = flag.clone();
            async move {
                if flag.load(Ordering::SeqCst) {
                    HttpResponse::Ok().json(serde_json::json!({
                        "kind":"NamespaceList", "apiVersion":"v1", "metadata":{}, "items":[]
                    }))
                } else {
                    HttpResponse::Unauthorized().json(serde_json::json!({
                        "kind":"Status", "apiVersion":"v1", "status":"Failure",
                        "message":"expired token", "reason":"Unauthorized", "code":401
                    }))
                }
            }
        }))
    });
    let path = std::env::temp_dir().join(format!("nabiman-kube-{}.json", uuid::Uuid::new_v4()));
    std::fs::write(
        &path,
        serde_json::json!({
            "apiVersion":"v1", "kind":"Config", "current-context":"test",
            "clusters":[{"name":"test", "cluster":{"server":server.url("")}}],
            "contexts":[{"name":"test", "context":{"cluster":"test", "user":"test"}}],
            "users":[{"name":"test", "user":{"token":"test-token"}}]
        })
        .to_string(),
    )
    .unwrap();
    let service = kube_from_config(KubeRealConfig {
        kubeconfig_path: Some(path.to_string_lossy().into_owned()),
        default_namespace: "default".into(),
    })
    .await;
    std::fs::remove_file(path).unwrap();
    assert!(matches!(
        service.health_check().await,
        Err(ServiceError::Upstream { .. })
    ));
    assert!(service.list_namespaces().await.is_err());
    recovered.store(true, Ordering::SeqCst);
    let health = service.health_check().await.unwrap();
    assert!(health.connected);
    assert_eq!(
        health.endpoint.trim_end_matches('/'),
        server.url("").trim_end_matches('/')
    );
    assert!(service.list_namespaces().await.unwrap().is_empty());
}

#[tokio::test]
async fn missing_provider_configuration_does_not_choose_demo() {
    let rancher = rancher_from_config(None);
    unavailable(rancher.list_projects("local").await);
    unavailable(rancher.create_project("local", "customer").await);
    let axgate = axgate_from_config(None);
    unavailable(axgate.list_nat_rules().await);
    unavailable(axgate.delete_nat_rule("rule").await);
    let harbor = harbor_from_config(None);
    unavailable(harbor.list_projects().await);
    unavailable(harbor.create_project("customer", false).await);
}

#[actix_web::test]
async fn disconnected_inventory_is_http_503_and_health_reports_failure() {
    use crate::mec::{db, handlers, MecState};
    use actix_web::{test, web, App};
    let bundle = ServiceBundle {
        kube: Arc::new(UnavailableService::new(
            "kubernetes",
            "configuration missing",
        )),
        rancher: rancher_from_config(None),
        axgate: axgate_from_config(None),
        harbor: harbor_from_config(None),
    };
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(MecState::new(
                db::open_in_memory().unwrap(),
                bundle,
            )))
            .route("/nodes", web::get().to(handlers::nodes::list))
            .route("/health", web::get().to(handlers::health::summary)),
    )
    .await;
    let response =
        test::call_service(&app, test::TestRequest::get().uri("/nodes").to_request()).await;
    assert_eq!(response.status(), 503);
    let body: serde_json::Value = test::read_body_json(response).await;
    assert!(body["data"].is_null());
    assert_eq!(body["error"]["code"], "UNAVAILABLE");
    let response =
        test::call_service(&app, test::TestRequest::get().uri("/health").to_request()).await;
    let health: serde_json::Value = test::read_body_json(response).await;
    for source in ["kubernetes", "rancher", "axgate", "harbor"] {
        assert_eq!(health["data"][source]["connected"], false);
        assert!(health["data"][source]["error"]
            .as_str()
            .unwrap()
            .contains("unavailable"));
    }
}
