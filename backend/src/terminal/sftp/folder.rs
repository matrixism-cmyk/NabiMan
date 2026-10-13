//! A whole folder as one download: a ZIP archive written while the folder is
//! read, so it starts at once and never needs room on either server.
//!
//! Links and special files are left out rather than followed (a link to `/`
//! would otherwise pull in the whole machine), and so is anything that cannot
//! be read. What was left out, and why, is listed in a text file at the end of
//! the archive, so a missing file is never a silent surprise.

use std::io::Write;

use actix_web::{web, HttpRequest, HttpResponse};
use tokio::io::AsyncReadExt;
use tokio::sync::mpsc;

use super::fs::{Fs, Meta};
use super::paths::{self, join, valid_path};
use super::zip::{worth_compressing, ZipWriter, BIG};
use super::{bad, resolve};

/// Entries an archive takes before it stops and says so.
const MAX_ENTRIES: usize = 200_000;
const CHUNK: usize = 256 * 1024;
const SKIPPED_NOTE: &str = "NABIMAN-SKIPPED.txt";

type Tx = mpsc::Sender<Result<web::Bytes, std::io::Error>>;

#[derive(serde::Deserialize)]
pub(super) struct FolderQuery {
    path: String,
}

pub(super) async fn download(req: HttpRequest, q: web::Query<FolderQuery>) -> HttpResponse {
    let t = match resolve(&req) {
        Ok(t) => t,
        Err(r) => return r,
    };
    if !valid_path(&q.path) {
        return bad("Invalid path");
    }
    let fs = match t.fs().await {
        Ok(fs) => fs,
        Err(r) => return r,
    };
    let root = match fs.stat(&q.path).await {
        Ok(meta) if meta.is_dir() => meta,
        Ok(_) => return bad("Not a folder"),
        Err(e) => return t.error(e),
    };
    let top = match paths::base_name(&q.path) {
        "download" if q.path.trim_end_matches('/').is_empty() => "root",
        name => name,
    }
    .to_string();
    // A small queue: reading waits for the browser, so a slow download never
    // piles the folder up in memory.
    let (tx, rx) = mpsc::channel(4);
    let path = q.path.trim_end_matches('/').to_string();
    let path = if path.is_empty() { "/".to_string() } else { path };
    let name = top.clone();
    actix_web::rt::spawn(async move {
        if let Err(e) = write_archive(&fs, &path, &name, root, &tx).await {
            let _ = tx.send(Err(e)).await;
        }
    });
    HttpResponse::Ok()
        .content_type("application/zip")
        .insert_header(("Content-Disposition", paths::disposition(&format!("{top}.zip"))))
        .insert_header(("Cache-Control", "no-store"))
        .streaming(tokio_stream::wrappers::ReceiverStream::new(rx))
}

async fn send(tx: &Tx, bytes: Vec<u8>) -> std::io::Result<()> {
    if bytes.is_empty() {
        return Ok(());
    }
    tx.send(Ok(web::Bytes::from(bytes)))
        .await
        .map_err(|_| std::io::Error::other("the download was closed"))
}

async fn write_archive(fs: &Fs, root: &str, top: &str, root_meta: Meta, tx: &Tx) -> std::io::Result<()> {
    let mut zip = ZipWriter::default();
    let mut skipped: Vec<String> = Vec::new();
    let mut entries = 0usize;
    // Depth first, in name order, so the archive lists like the folder does.
    let mut stack = vec![(root.to_string(), top.to_string(), root_meta)];
    'walk: while let Some((abs, rel, meta)) = stack.pop() {
        send(tx, zip.dir(&rel, meta.mtime, meta.mode.unwrap_or(0o755))).await?;
        let mut items = match fs.read_dir(&abs).await {
            Ok(items) => items,
            Err(e) => {
                skipped.push(format!("{rel}/  ({})", e.message));
                continue;
            }
        };
        items.sort_by(|a, b| a.0.cmp(&b.0));
        let mut folders = Vec::new();
        for (name, meta) in items {
            entries += 1;
            if entries > MAX_ENTRIES {
                skipped.push(format!("… stopped after {MAX_ENTRIES} entries; download the rest separately"));
                break 'walk;
            }
            let child = (join(&abs, &name), format!("{rel}/{name}"), meta);
            match child.2.kind {
                "dir" => folders.push(child),
                "file" => add_file(fs, &mut zip, &child.0, &child.1, &child.2, &mut skipped, tx).await?,
                "link" => skipped.push(format!("{}  (link, not followed)", child.1)),
                _ => skipped.push(format!("{}  (special file)", child.1)),
            }
        }
        stack.extend(folders.into_iter().rev());
    }
    if !skipped.is_empty() {
        let note = format!(
            "These were not included in {top}.zip:\n\n{}\n",
            skipped.join("\n")
        );
        let body = note.into_bytes();
        let now = u32::try_from(super::super::session::now_unix()).ok();
        send(tx, zip.begin_file(&format!("{top}/{SKIPPED_NOTE}"), now, 0o644, false, false)).await?;
        zip.wrote(body.len());
        let (crc, size) = (crc32fast::hash(&body), body.len() as u64);
        send(tx, body).await?;
        send(tx, zip.end_file(crc, size, size)?).await?;
    }
    send(tx, zip.finish()).await
}

async fn add_file(
    fs: &Fs,
    zip: &mut ZipWriter,
    abs: &str,
    rel: &str,
    meta: &Meta,
    skipped: &mut Vec<String>,
    tx: &Tx,
) -> std::io::Result<()> {
    let mut reader = match fs.open(abs).await {
        Ok(r) => r,
        Err(e) => {
            skipped.push(format!("{rel}  ({})", e.message));
            return Ok(());
        }
    };
    let deflate = worth_compressing(rel);
    // The size at listing time bounds what is read, so a file still growing
    // cannot outgrow the header already sent for it.
    let mut left = meta.size;
    send(tx, zip.begin_file(rel, meta.mtime, meta.mode.unwrap_or(0o644), deflate, meta.size >= BIG)).await?;
    let mut crc = crc32fast::Hasher::new();
    let mut encoder = deflate.then(|| flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::fast()));
    let (mut size, mut compressed) = (0u64, 0u64);
    let mut buf = vec![0u8; CHUNK];
    while left > 0 {
        let want = buf.len().min(usize::try_from(left).unwrap_or(usize::MAX));
        let n = match reader.read(&mut buf[..want]).await {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) => {
                // The header is out; close the entry with what arrived and say so.
                skipped.push(format!("{rel}  (read failed after {size} bytes: {e}; the copy is incomplete)"));
                break;
            }
        };
        crc.update(&buf[..n]);
        size += n as u64;
        left -= n as u64;
        let out = match encoder.as_mut() {
            Some(enc) => {
                enc.write_all(&buf[..n])?;
                std::mem::take(enc.get_mut())
            }
            None => buf[..n].to_vec(),
        };
        compressed += out.len() as u64;
        zip.wrote(out.len());
        send(tx, out).await?;
    }
    if let Some(enc) = encoder {
        let out = enc.finish()?;
        compressed += out.len() as u64;
        zip.wrote(out.len());
        send(tx, out).await?;
    }
    if size < meta.size && !skipped.last().is_some_and(|s| s.starts_with(rel)) {
        skipped.push(format!("{rel}  (shrank while being read; {size} of {} bytes)", meta.size));
    }
    send(tx, zip.end_file(crc.finalize(), compressed, size)?).await
}
