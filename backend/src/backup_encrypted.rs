use super::*;
use age::secrecy::SecretString;
use nabiman_server::backup_crypto::{decrypt, encrypt, password_valid};
use std::sync::atomic::{AtomicBool, Ordering};
static BUSY: AtomicBool = AtomicBool::new(false);
struct Permit;
impl Drop for Permit {
    fn drop(&mut self) {
        BUSY.store(false, Ordering::Release);
    }
}
fn permit() -> Option<Permit> {
    BUSY.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .ok()
        .map(|_| Permit)
}
fn response(
    result: Result<Result<String, String>, actix_web::error::BlockingError>,
) -> HttpResponse {
    match result {
        Ok(Ok(value)) => HttpResponse::Ok()
            .insert_header(("Cache-Control", "no-store"))
            .json(ApiResponse::ok(value)),
        Ok(Err(error)) => HttpResponse::UnprocessableEntity()
            .insert_header(("Cache-Control", "no-store"))
            .json(ApiResponse::<String>::error(&error)),
        Err(_) => HttpResponse::ServiceUnavailable().json(ApiResponse::<String>::error(
            "Encrypted backup worker unavailable",
        )),
    }
}
fn busy() -> HttpResponse {
    HttpResponse::Conflict().json(ApiResponse::<()>::error(
        "Another encrypted backup or restore is running",
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EncryptedBackupRequest {
    paths: Vec<String>,
    name: Option<String>,
    password: String,
}
pub(super) async fn create_encrypted(body: web::Json<EncryptedBackupRequest>) -> HttpResponse {
    let body = body.into_inner();
    let password = SecretString::from(body.password);
    if !password_valid(&password) {
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error(
            "Use a passphrase of 12 or more characters (at most 1024 bytes, no control characters)",
        ));
    }
    let Some(permit) = permit() else {
        return busy();
    };
    response(
        web::block(move || {
            let _permit = permit;
            encrypt(
                &backup_dir(),
                body.name.as_deref().unwrap_or("archive"),
                &body.paths,
                password,
            )
        })
        .await,
    )
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EncryptedRestoreRequest {
    filename: String,
    target_dir: String,
    password: String,
}
pub(super) async fn restore_encrypted(body: web::Json<EncryptedRestoreRequest>) -> HttpResponse {
    let body = body.into_inner();
    let password = SecretString::from(body.password);
    if body.filename.ends_with(".enc") {
        return HttpResponse::UnprocessableEntity().json(ApiResponse::<()>::error(
            "Legacy CBC backup: use the documented offline recovery procedure; file was preserved",
        ));
    }
    if !password_valid(&password) {
        return HttpResponse::BadRequest()
            .json(ApiResponse::<()>::error("Invalid passphrase length"));
    }
    let Some(permit) = permit() else {
        return busy();
    };
    response(
        web::block(move || {
            let _permit = permit;
            decrypt(&backup_dir(), &body.filename, &body.target_dir, password)
        })
        .await,
    )
}
