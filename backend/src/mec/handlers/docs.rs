use super::util::{from_service_error, list_response, ok_response};
use crate::mec::MecState;
use actix_web::{web, HttpResponse};
use serde::Deserialize;

pub async fn templates() -> HttpResponse {
    list_response(vec![
        serde_json::json!({
            "id": "tenant_access_guide",
            "name": "기업별 접속 가이드",
            "formats": ["txt", "docx"],
        }),
        serde_json::json!({
            "id": "cluster_report",
            "name": "클러스터 현황 보고서",
            "formats": ["txt"],
            "status": "planned"
        }),
    ])
}

pub async fn list_generated(state: web::Data<MecState>) -> HttpResponse {
    match state.docs_store.list_recent(50) {
        Ok(v) => list_response(v),
        Err(e) => from_service_error(e.into()),
    }
}

#[derive(Debug, Deserialize)]
pub struct GenerateDocsRequest {
    pub template: String,
    #[serde(default = "default_format")]
    pub format: String,
    #[serde(default)]
    pub tenant_ids: Vec<String>,
}

fn default_format() -> String {
    "docx".into()
}

pub async fn generate(
    body: web::Json<GenerateDocsRequest>,
    state: web::Data<MecState>,
) -> HttpResponse {
    if body.template != "tenant_access_guide" {
        return from_service_error(
            crate::mec::services::ServiceError::InvalidInput(format!(
                "template {} not supported yet",
                body.template
            )),
        );
    }
    let output_dir = crate::mec::docgen::DocGenerator::default_output_dir();
    let text_gen = crate::mec::docgen::DocGenerator::new(
        state.docs_store.clone(),
        output_dir.clone(),
    );
    let docx_gen = crate::mec::docgen::DocxGenerator::new(
        state.docs_store.clone(),
        output_dir,
    );
    let mut results: Vec<serde_json::Value> = Vec::new();
    for tid in &body.tenant_ids {
        let tenant = match state.tenants.get(tid) {
            Ok(Some(t)) => t,
            Ok(None) => continue,
            Err(e) => return from_service_error(e.into()),
        };
        let doc_res = if body.format == "txt" {
            text_gen
                .generate_tenant_guide_text_file(&tenant, "admin")
                .map(|d| serde_json::to_value(&d).unwrap_or_default())
        } else {
            docx_gen
                .generate_tenant_guide(&tenant, "admin")
                .map(|d| serde_json::to_value(&d).unwrap_or_default())
        };
        match doc_res {
            Ok(v) => results.push(v),
            Err(e) => {
                return from_service_error(
                    crate::mec::services::ServiceError::Internal(e.to_string()),
                );
            }
        }
    }
    ok_response(serde_json::json!({
        "generated": results.len(),
        "docs": results,
    }))
}

pub async fn download(
    path: web::Path<String>,
    state: web::Data<MecState>,
) -> HttpResponse {
    let doc = match state.docs_store.get(&path) {
        Ok(Some(d)) => d,
        Ok(None) => {
            return from_service_error(
                crate::mec::services::ServiceError::NotFound(format!("doc {}", &path)),
            );
        }
        Err(e) => return from_service_error(e.into()),
    };
    match std::fs::read(&doc.file_path) {
        Ok(data) => {
            let _ = state.docs_store.increment_download(&doc.id);
            let mime = match doc.format.as_str() {
                "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
                "pdf" => "application/pdf",
                _ => "text/plain; charset=utf-8",
            };
            HttpResponse::Ok()
                .insert_header((
                    "Content-Disposition",
                    format!(r#"attachment; filename="{}""#, doc.filename),
                ))
                .content_type(mime)
                .body(data)
        }
        Err(e) => from_service_error(
            crate::mec::services::ServiceError::Internal(e.to_string()),
        ),
    }
}
