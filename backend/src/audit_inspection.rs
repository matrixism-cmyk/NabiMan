//! Local checks detect inconsistencies, not rewriting by a privileged host operator.
use super::{storage, AuditEntry, AuditLog};
use crate::models::ApiResponse;
use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
#[derive(Serialize)]
pub(super) struct Segment {
    pub name: String,
    pub valid: bool,
    pub entries_checked: usize,
    pub malformed_lines: usize,
    pub hash_mismatches: usize,
    pub unhashed_entries: usize,
    pub truncated: bool,
    pub sha256: Option<String>,
    pub bytes: usize,
    pub error: Option<String>,
}
pub(super) fn inspect(name: &str, bytes: &[u8]) -> Segment {
    let mut result = Segment {
        name: name.into(),
        valid: true,
        entries_checked: 0,
        malformed_lines: 0,
        hash_mismatches: 0,
        unhashed_entries: 0,
        truncated: !bytes.is_empty() && !bytes.ends_with(b"\n"),
        sha256: Some(hex::encode(Sha256::digest(bytes))),
        bytes: bytes.len(),
        error: None,
    };
    let mut previous = "0".to_string();
    for line in bytes.split(|b| *b == b'\n').filter(|line| !line.is_empty()) {
        let entry: AuditEntry = match serde_json::from_slice(line) {
            Ok(entry) => entry,
            Err(_) => {
                result.malformed_lines += 1;
                continue;
            }
        };
        result.entries_checked += 1;
        let expected = super::compute_hash(&previous, &entry);
        match entry.hash.as_ref() {
            None => result.unhashed_entries += 1,
            Some(hash) => {
                if *hash != expected {
                    result.hash_mismatches += 1;
                }
                previous = hash.clone();
            }
        }
    }
    result.valid = result.malformed_lines == 0
        && result.hash_mismatches == 0
        && result.unhashed_entries == 0
        && !result.truncated;
    result
}
fn failure(name: &str, error: &str) -> Segment {
    Segment {
        name: name.into(),
        valid: false,
        entries_checked: 0,
        malformed_lines: 0,
        hash_mismatches: 0,
        unhashed_entries: 0,
        truncated: false,
        sha256: None,
        bytes: 0,
        error: Some(error.into()),
    }
}
fn response(value: impl Serialize) -> HttpResponse {
    HttpResponse::Ok()
        .insert_header(("Cache-Control", "no-store"))
        .json(ApiResponse::ok(value))
}
pub async fn health(store: web::Data<AuditLog>) -> HttpResponse {
    let state = store.lock().unwrap();
    response(
        serde_json::json!({"writable":state.error.is_none(),"error":state.error,"last_persisted_at":state.last_persisted_at,"memory_entries":state.entries.len(),"unparsed_startup_lines":state.unparsed_lines,"retention":"active plus up to five rotated 10 MB segments","durability":"local fsync; not independent storage","operation_results":"persistence failure does not roll back completed operations"}),
    )
}
pub async fn verify(store: web::Data<AuditLog>) -> HttpResponse {
    let result=web::block(move || {
        let state=store.lock().unwrap();
        let mut segments=vec![];
        for index in (0..=5).rev() {
            let name=if index==0 {"audit.jsonl".into()} else {format!("audit.jsonl.{index}")};
            let path=state.path.with_file_name(&name);
            match std::fs::symlink_metadata(&path) {
                Err(e) if e.kind()==std::io::ErrorKind::NotFound=>continue,
                _=>(),
            }
            segments.push(match storage::read(&path) {Ok(bytes)=>inspect(&name,&bytes),Err(error)=>failure(&name,error)});
        }
        serde_json::json!({"valid":!segments.is_empty()&&segments.iter().all(|s|s.valid),"entries_checked":segments.iter().map(|s|s.entries_checked).sum::<usize>(),"error":state.error,"segments":segments,"scope":"retained files; each legacy segment starts its own chain","independent_proof":false,"complete_history_proven":false})
    }).await;
    match result {
        Ok(value) => response(value),
        Err(_) => HttpResponse::ServiceUnavailable()
            .json(ApiResponse::<()>::error("Audit verification unavailable")),
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Export {
    segment: String,
}
pub async fn export(store: web::Data<AuditLog>, body: web::Json<Export>) -> HttpResponse {
    let name = body.into_inner().segment;
    if !(name == "audit.jsonl" || (1..=5).any(|i| name == format!("audit.jsonl.{i}"))) {
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error("Unknown audit segment"));
    }
    let result=web::block(move || {
        let state=store.lock().unwrap();
        let bytes=storage::read(&state.path.with_file_name(&name))?;
        let verification=inspect(&name,&bytes);
        let entries:Vec<_>=bytes.split(|b|*b==b'\n').filter_map(|line|serde_json::from_slice::<AuditEntry>(line).ok()).map(|mut e|{
            e.path=crate::audit_middleware::audit_path(&e.path);e.hash=None;e
        }).collect();
        Ok::<_,&'static str>(serde_json::json!({"schema_version":1,"generated_at":chrono::Utc::now().to_rfc3339(),"segment":name,"verification":verification,"sanitized":true,"original_chain_not_reconstructible":true,"entries":entries}))
    }).await;
    match result {
        Ok(Ok(value)) => response(value),
        _ => HttpResponse::ServiceUnavailable().json(ApiResponse::<()>::error(
            "Audit segment cannot be exported; inspect storage",
        )),
    }
}
