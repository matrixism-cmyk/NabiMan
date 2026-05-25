use super::util::{created, from_service_error, list_response, ok_response};
use crate::mec::MecState;
use actix_web::{web, HttpResponse};
use serde::Deserialize;

pub async fn overview() -> HttpResponse {
    // Longhorn 볼륨/용량은 Phase 2에서 kube-rs로 조회 예정.
    ok_response(serde_json::json!({
        "longhorn": {
            "status": "pending integration",
            "total_bytes": null,
            "used_bytes": null,
        },
        "note": "Longhorn 통합은 Phase 2에서 제공됩니다."
    }))
}

pub async fn pvcs(state: web::Data<MecState>) -> HttpResponse {
    match state.services.kube.list_pvcs(None).await {
        Ok(v) => list_response(v),
        Err(e) => from_service_error(e),
    }
}

pub async fn harbor_projects(state: web::Data<MecState>) -> HttpResponse {
    match state.services.harbor.list_projects().await {
        Ok(p) => list_response(p),
        Err(e) => from_service_error(e),
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateHarborProjectRequest {
    pub name: String,
    #[serde(default)]
    pub public: bool,
}

pub async fn create_harbor_project(
    body: web::Json<CreateHarborProjectRequest>,
    state: web::Data<MecState>,
) -> HttpResponse {
    match state
        .services
        .harbor
        .create_project(&body.name, body.public)
        .await
    {
        Ok(p) => created(p),
        Err(e) => from_service_error(e),
    }
}

pub async fn harbor_repositories(
    path: web::Path<String>,
    state: web::Data<MecState>,
) -> HttpResponse {
    match state.services.harbor.list_repositories(&path).await {
        Ok(r) => list_response(r),
        Err(e) => from_service_error(e),
    }
}
