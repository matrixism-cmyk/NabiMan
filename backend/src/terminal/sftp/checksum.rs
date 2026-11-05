//! A file's SHA-256, asked for from its properties: reading a large file
//! takes a while, so the answer is JSON lines — how much has been read, a few
//! times a second, then the digest. Closing the request stops the reading.

use std::time::{Duration, Instant};

use actix_web::{web, HttpRequest, HttpResponse};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokio::io::AsyncReadExt;
use tokio::sync::mpsc;

use super::paths::valid_path;
use super::{bad, resolve};

const CHUNK: usize = 256 * 1024;
const TICK: Duration = Duration::from_millis(300);

#[derive(Deserialize)]
pub(super) struct ChecksumBody {
    path: String,
}

type Tx = mpsc::Sender<Result<web::Bytes, std::io::Error>>;

async fn line(tx: &Tx, value: serde_json::Value) -> bool {
    tx.send(Ok(web::Bytes::from(format!("{value}\n")))).await.is_ok()
}

/// `POST …/checksum` with `{"path": …}`: `{"read","size"}` lines, then `{"sha256"}` or `{"error"}`.
pub(super) async fn checksum(req: HttpRequest, b: web::Json<ChecksumBody>) -> HttpResponse {
    let t = match resolve(&req) {
        Ok(t) => t,
        Err(r) => return r,
    };
    if !valid_path(&b.path) {
        return bad("Invalid path");
    }
    let fs = match t.fs().await {
        Ok(fs) => fs,
        Err(r) => return r,
    };
    let size = match fs.stat(&b.path).await {
        Ok(meta) if meta.kind == "file" => meta.size,
        Ok(_) => return bad("Only a file has a checksum"),
        Err(e) => return t.error(e),
    };
    let mut file = match fs.open(&b.path).await {
        Ok(f) => f,
        Err(e) => return t.error(e),
    };
    let (tx, rx) = mpsc::channel(4);
    actix_web::rt::spawn(async move {
        let mut hash = Sha256::new();
        let mut buf = vec![0u8; CHUNK];
        let (mut read, mut told) = (0u64, Instant::now());
        loop {
            match file.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    hash.update(&buf[..n]);
                    read += n as u64;
                }
                Err(e) => {
                    line(&tx, serde_json::json!({ "error": e.to_string() })).await;
                    return;
                }
            }
            if told.elapsed() >= TICK {
                told = Instant::now();
                // The panel closed the request: stop reading.
                if !line(&tx, serde_json::json!({ "read": read, "size": size })).await {
                    return;
                }
            }
        }
        line(&tx, serde_json::json!({ "read": read, "size": size, "sha256": hex::encode(hash.finalize()) })).await;
    });
    HttpResponse::Ok()
        .content_type("application/x-ndjson")
        .insert_header(("Cache-Control", "no-store"))
        .streaming(tokio_stream::wrappers::ReceiverStream::new(rx))
}
