//! A file served a range at a time, for the preview's players: a video or a
//! song plays from where it is seeked to without the whole file coming
//! first, and an archive's directory is read from its end. Reached with a
//! stream link (see `links`), so the browser's own media requests get in.

use actix_web::http::{header, StatusCode};
use actix_web::{web, HttpRequest, HttpResponse};
use futures::stream;
use serde::Deserialize;
use tokio::io::AsyncReadExt;

use super::paths::{base_name, valid_path};
use super::{bad, resolve};

const CHUNK: usize = 256 * 1024;

#[derive(Deserialize)]
pub(super) struct StreamQuery {
    path: String,
}

/// `bytes=a-b`, `bytes=a-` or `bytes=-n` against a file of `size` bytes, as
/// the first and last byte; `Err` when it does not fit. One range only:
/// players never ask for more.
pub(crate) fn parse_range(value: &str, size: u64) -> Result<(u64, u64), ()> {
    let spec = value.trim().strip_prefix("bytes=").ok_or(())?;
    if spec.contains(',') || size == 0 {
        return Err(());
    }
    let (a, b) = spec.split_once('-').ok_or(())?;
    let (a, b) = (a.trim(), b.trim());
    let (start, end) = if a.is_empty() {
        let n: u64 = b.parse().map_err(|_| ())?;
        if n == 0 {
            return Err(());
        }
        (size.saturating_sub(n), size - 1)
    } else {
        let start: u64 = a.parse().map_err(|_| ())?;
        let end = if b.is_empty() { size - 1 } else { b.parse::<u64>().map_err(|_| ())?.min(size - 1) };
        (start, end)
    };
    if start > end || start >= size {
        return Err(());
    }
    Ok((start, end))
}

/// The type a player needs to recognise a media file; anything else is bytes.
pub(crate) fn media_type(name: &str) -> &'static str {
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "mp4" | "m4v" => "video/mp4",
        "webm" => "video/webm",
        "ogv" => "video/ogg",
        "mov" => "video/quicktime",
        "mkv" => "video/x-matroska",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "flac" => "audio/flac",
        "m4a" | "m4b" => "audio/mp4",
        "aac" => "audio/aac",
        "ogg" | "oga" | "opus" => "audio/ogg",
        "weba" => "audio/webm",
        _ => "application/octet-stream",
    }
}

pub(super) async fn serve(req: HttpRequest, q: web::Query<StreamQuery>) -> HttpResponse {
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
    let meta = match fs.stat(&q.path).await {
        Ok(m) => m,
        Err(e) => return t.error(e),
    };
    if meta.is_dir() {
        return bad("A folder cannot be played");
    }
    let size = meta.size;
    let asked = req.headers().get(header::RANGE).and_then(|v| v.to_str().ok());
    let (start, end, partial) = match asked.map(|v| parse_range(v, size)) {
        None if size == 0 => (0, 0, false),
        None => (0, size - 1, false),
        Some(Ok((a, b))) => (a, b, true),
        Some(Err(())) => {
            return HttpResponse::build(StatusCode::RANGE_NOT_SATISFIABLE)
                .insert_header((header::CONTENT_RANGE, format!("bytes */{size}")))
                .finish();
        }
    };
    let length = if size == 0 { 0 } else { end - start + 1 };
    let reader = match fs.open_from(&q.path, start).await {
        Ok(r) => r,
        Err(e) => return t.error(e),
    };
    // The connection travels with the stream so it stays open until the end.
    let state = Some((reader.take(length), fs, vec![0u8; CHUNK]));
    let body = stream::unfold(state, |state| async move {
        let (mut file, fs, mut buf) = state?;
        match file.read(&mut buf).await {
            Ok(0) => None,
            Ok(n) => Some((Ok(web::Bytes::copy_from_slice(&buf[..n])), Some((file, fs, buf)))),
            Err(e) => Some((Err(actix_web::error::ErrorBadGateway(e)), None)),
        }
    });
    let mut res = if partial { HttpResponse::PartialContent() } else { HttpResponse::Ok() };
    res.content_type(media_type(&q.path))
        .insert_header((header::ACCEPT_RANGES, "bytes"))
        .insert_header((header::CACHE_CONTROL, "private, no-store"))
        .insert_header((header::CONTENT_DISPOSITION, super::paths::disposition(base_name(&q.path)).replacen("attachment", "inline", 1)))
        .no_chunking(length);
    if partial {
        res.insert_header((header::CONTENT_RANGE, format!("bytes {start}-{end}/{size}")));
    }
    res.streaming(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_are_read_as_players_send_them() {
        assert_eq!(parse_range("bytes=0-", 1000), Ok((0, 999)));
        assert_eq!(parse_range("bytes=100-199", 1000), Ok((100, 199)));
        assert_eq!(parse_range("bytes=900-5000", 1000), Ok((900, 999)));
        assert_eq!(parse_range("bytes=-100", 1000), Ok((900, 999)));
        assert_eq!(parse_range("bytes=-5000", 1000), Ok((0, 999)));
        for bad in ["bytes=1000-", "bytes=5-1", "bytes=0-1,5-9", "items=0-1", "bytes=-0", "bytes=a-b"] {
            assert_eq!(parse_range(bad, 1000), Err(()), "{bad}");
        }
        assert_eq!(parse_range("bytes=0-", 0), Err(()));
    }

    #[test]
    fn media_is_labelled_for_players() {
        assert_eq!(media_type("Clip.MP4"), "video/mp4");
        assert_eq!(media_type("song.flac"), "audio/flac");
        assert_eq!(media_type("voice.opus"), "audio/ogg");
        assert_eq!(media_type("archive.zip"), "application/octet-stream");
    }
}
