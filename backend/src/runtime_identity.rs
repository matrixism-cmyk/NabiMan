//! Identify the executable this process actually started, including after atomic replacement.
use crate::models::ApiResponse;
use actix_web::{web, HttpResponse};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{self, Read},
};

#[derive(Serialize, Clone)]
pub struct RuntimeIdentity {
    package_version: &'static str,
    executable_sha256: String,
    instance_id: String,
    started_at: String,
}

impl RuntimeIdentity {
    pub fn capture() -> io::Result<Self> {
        // Do not follow argv[0], an environment-supplied build label, or the
        // installation pathname: an update can already have replaced that file.
        let mut executable = File::open("/proc/self/exe")?;
        let mut digest = Sha256::new();
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let read = executable.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            digest.update(&buffer[..read]);
        }
        Ok(Self {
            package_version: env!("CARGO_PKG_VERSION"),
            executable_sha256: hex::encode(digest.finalize()),
            instance_id: uuid::Uuid::new_v4().to_string(),
            started_at: chrono::Utc::now().to_rfc3339(),
        })
    }
}

async fn inspect(identity: web::Data<RuntimeIdentity>) -> HttpResponse {
    HttpResponse::Ok()
        .insert_header(("Cache-Control", "no-store"))
        .json(ApiResponse::ok(identity.get_ref()))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.route("/runtime", web::get().to(inspect));
}
