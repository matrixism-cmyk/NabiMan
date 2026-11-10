use super::*;
use actix_web::{http::StatusCode, test, App};

async fn upload(dir: &std::path::Path, name: &str, size: usize) -> StatusCode {
    let app = test::init_service(App::new().configure(config)).await;
    let req = test::TestRequest::post()
        .uri("/api/files/upload")
        .insert_header(("X-File-Path", dir.join(name).to_string_lossy().into_owned()))
        .set_payload(vec![7u8; size])
        .to_request();
    test::call_service(&app, req).await.status()
}

#[actix_rt::test]
async fn uploads_up_to_the_advertised_limit_are_taken_and_larger_refused() {
    let dir = std::env::temp_dir().join(format!("nabiman-upload-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    assert_eq!(upload(&dir, "ten.bin", 10 * 1024 * 1024).await, StatusCode::OK);
    assert_eq!(std::fs::metadata(dir.join("ten.bin")).unwrap().len(), 10 * 1024 * 1024);
    let over = MAX_UPLOAD_SIZE as usize + 1;
    assert_eq!(upload(&dir, "over.bin", over).await, StatusCode::PAYLOAD_TOO_LARGE);
    assert!(!dir.join("over.bin").exists());
    let _ = std::fs::remove_dir_all(dir);
}
