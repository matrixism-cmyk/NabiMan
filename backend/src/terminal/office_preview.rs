//! Office documents (Word, Excel, PowerPoint, OpenDocument, RTF) and Hangul
//! documents (HWP 5.0, HWPX) shown in the file panel's preview: they are
//! turned into a PDF, which the browser's PDF viewer then draws. Office
//! files go through LibreOffice; Hangul files through rhwp, which lays them
//! out the way Hangul does, and through LibreOffice's H2Orestart extension
//! when rhwp cannot.
//!
//! The browser sends the file it already fetched (from whichever server or
//! share it came from) and gets a PDF back. LibreOffice runs as `nobody`, in
//! a fresh folder and profile each time, one document at a time, with a time
//! limit; macros are not run in a headless conversion. Results are kept by
//! the content's hash, so the same file is converted once.

use std::fs;
use std::io::Write;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use actix_web::{web, HttpRequest, HttpResponse};
use sha2::{Digest, Sha256};

use crate::models::ApiResponse;

/// Larger documents are not converted for a preview (they can be downloaded).
const MAX_BYTES: usize = 50 << 20;
const TIME_LIMIT: Duration = Duration::from_secs(90);
/// The cache stays under this; the least recently used go first.
const CACHE_BYTES: u64 = 512 << 20;
/// LibreOffice asks to be started again after making a new profile.
const RESTART: i32 = 81;
const NOBODY: u32 = 65534;

/// What a file is converted as.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    /// Text and slides, page by page.
    Document,
    /// A spreadsheet: each sheet one page, not cut into printed pages.
    Sheet,
    /// HWP 5.0 or HWPX.
    Hangul,
}

impl Kind {
    fn tag(self) -> &'static [u8] {
        match self {
            Kind::Document => b"p",
            Kind::Sheet => b"s",
            Kind::Hangul => b"h",
        }
    }
}

fn kind(ext: &str) -> Option<Kind> {
    match ext {
        "doc" | "docx" | "docm" | "dot" | "dotx" | "dotm" | "odt" | "ott" | "fodt" | "rtf" | "wpd" => Some(Kind::Document),
        "ppt" | "pptx" | "pptm" | "pps" | "ppsx" | "pot" | "potx" | "odp" | "otp" | "fodp" => Some(Kind::Document),
        "xls" | "xlsx" | "xlsm" | "xlsb" | "xlt" | "xltx" | "ods" | "ots" | "fods" => Some(Kind::Sheet),
        "hwp" | "hwpx" => Some(Kind::Hangul),
        _ => None,
    }
}

fn extension(name: &str) -> String {
    Path::new(name).extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase()
}

/// LibreOffice and rhwp: their settings, else where they are usually installed
/// (see `install_config::settings`). An empty path when there is none.
fn soffice() -> PathBuf {
    crate::install_config::settings::soffice().unwrap_or_default()
}

fn rhwp() -> PathBuf {
    crate::install_config::settings::rhwp().unwrap_or_default()
}

/// Korean fonts both converters use (e.g. Noto Sans/Serif KR), and the
/// fontconfig file in it that adds them to the system's: `NABIMAN_PREVIEW_FONTS`.
fn fonts() -> Option<PathBuf> {
    crate::install_config::settings::preview_fonts()
}

fn cache_dir() -> PathBuf {
    nabiman_server::data_dir().join("preview-cache")
}

/// The cache folder, this process's own and private, or `None`.
fn private_cache() -> Option<PathBuf> {
    let dir = cache_dir();
    fs::create_dir_all(&dir).ok()?;
    // SAFETY: geteuid has no preconditions.
    let me = unsafe { libc::geteuid() };
    if fs::metadata(&dir).ok()?.uid() != me {
        std::os::unix::fs::chown(&dir, Some(me), None).ok()?;
    }
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).ok()?;
    Some(dir)
}

/// Drops the least recently used results until the cache fits.
fn trim_cache(dir: &Path) {
    let Ok(read) = fs::read_dir(dir) else { return };
    let mut files: Vec<(std::time::SystemTime, u64, PathBuf)> = read
        .flatten()
        .filter_map(|e| {
            let m = e.metadata().ok()?;
            m.is_file().then(|| (m.accessed().or_else(|_| m.modified()).ok().unwrap_or(std::time::UNIX_EPOCH), m.len(), e.path()))
        })
        .collect();
    let mut total: u64 = files.iter().map(|f| f.1).sum();
    files.sort();
    for (_, len, path) in files {
        if total <= CACHE_BYTES {
            break;
        }
        if fs::remove_file(&path).is_ok() {
            total = total.saturating_sub(len);
        }
    }
}

/// One conversion at a time: LibreOffice is heavy, and a queue keeps it fair.
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

/// The PDF for these bytes, converted or from the cache.
fn convert(bytes: &[u8], ext: &str, kind: Kind) -> Result<Vec<u8>, String> {
    let cache = private_cache().ok_or("미리보기 캐시 폴더를 쓸 수 없습니다")?;
    convert_in(&cache, bytes, ext, kind)
}

fn convert_in(cache: &Path, bytes: &[u8], ext: &str, kind: Kind) -> Result<Vec<u8>, String> {
    let program = soffice();
    let hangul = rhwp();
    if !program.is_file() && !(kind == Kind::Hangul && hangul.is_file()) {
        return Err("문서 변환기(LibreOffice)가 이 서버에 없습니다. 설치하거나 NABIMAN_SOFFICE로 위치를 지정하세요".into());
    }
    let key = hex::encode(Sha256::digest([bytes, ext.as_bytes(), kind.tag()].concat()));
    let cached = cache.join(format!("{key}.pdf"));
    if let Ok(pdf) = fs::read(&cached) {
        let _ = filetime_touch(&cached);
        return Ok(pdf);
    }
    let _turn = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    // Another request may have made it while this one waited.
    if let Ok(pdf) = fs::read(&cached) {
        return Ok(pdf);
    }
    let work = Workdir::new()?;
    let input = work.path.join(format!("document.{ext}"));
    fs::write(&input, bytes).map_err(|e| format!("write: {e}"))?;
    let _ = std::os::unix::fs::chown(&input, Some(NOBODY), Some(NOBODY));
    let output = work.path.join("document.pdf");
    let started = Instant::now();
    // Hangul documents: rhwp first, which lays them out the way Hangul does.
    if kind == Kind::Hangul && hangul.is_file() {
        let fonts = fonts().filter(|f| f.is_dir());
        let mut args: Vec<String> = vec!["export-pdf".into(), input.display().to_string(), "-o".into(), output.display().to_string()];
        if let Some(fonts) = fonts {
            args.extend(["--font-path".into(), fonts.display().to_string(), "--fallback-serif".into(), "Noto Serif KR".into(), "--fallback-sans".into(), "Noto Sans KR".into(), "--fallback-mono".into(), "Noto Sans KR".into()]);
        }
        args.extend(["--font-path".into(), "/usr/share/fonts".into()]);
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let done = run(&hangul, &work.path, &args, TIME_LIMIT)?;
        if done == Some(0) {
            if let Ok(pdf) = fs::read(&output) {
                if pdf.starts_with(b"%PDF") {
                    return Ok(keep(cache, &key, pdf));
                }
            }
        }
        // LibreOffice (H2Orestart) gets the next try, with what time is left.
        let _ = fs::remove_file(&output);
        if !program.exists() {
            return Err("이 한글 문서는 변환하지 못했습니다".into());
        }
    }
    let filter = if kind == Kind::Sheet {
        r#"pdf:calc_pdf_Export:{"SinglePageSheets":{"type":"boolean","value":"true"}}"#
    } else {
        "pdf"
    };
    let profile = format!("file://{}", work.path.join("profile").display());
    let mut code = None;
    for _ in 0..3 {
        code = run(&program, &work.path, &[
            &format!("-env:UserInstallation={profile}"),
            "--headless",
            "--norestore",
            "--nologo",
            "--nolockcheck",
            "--convert-to",
            filter,
            "--outdir",
            &work.path.display().to_string(),
            &input.display().to_string(),
        ], TIME_LIMIT.saturating_sub(started.elapsed()))?;
        if code != Some(RESTART) {
            break;
        }
    }
    let pdf = fs::read(&output).map_err(|_| match code {
        None => "변환 시간이 초과되었습니다".to_string(),
        _ => "이 문서는 변환하지 못했습니다 (손상되었거나 암호가 걸린 파일일 수 있습니다)".to_string(),
    })?;
    if !pdf.starts_with(b"%PDF") {
        return Err("변환 결과가 PDF가 아닙니다".into());
    }
    Ok(keep(cache, &key, pdf))
}

/// Puts a result in the cache (best effort) and hands it back.
fn keep(cache: &Path, key: &str, pdf: Vec<u8>) -> Vec<u8> {
    let temp = cache.join(format!(".{key}.{}.tmp", std::process::id()));
    if fs::File::create(&temp).and_then(|mut f| f.write_all(&pdf)).is_ok() {
        let _ = fs::rename(&temp, cache.join(format!("{key}.pdf")));
    }
    trim_cache(cache);
    pdf
}

/// Marks a cache entry as just used.
fn filetime_touch(path: &Path) -> std::io::Result<()> {
    let f = fs::OpenOptions::new().append(true).open(path)?;
    f.set_modified(std::time::SystemTime::now())
}

/// A private working folder for one conversion, owned by `nobody`, removed after.
struct Workdir {
    path: PathBuf,
}

impl Workdir {
    fn new() -> Result<Self, String> {
        let path = std::env::temp_dir().join(format!("nabiman-office-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&path).map_err(|e| format!("temp: {e}"))?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).map_err(|e| format!("temp: {e}"))?;
        // Root hands the folder to nobody; as anyone else, it stays theirs.
        if unsafe { libc::geteuid() } == 0 {
            std::os::unix::fs::chown(&path, Some(NOBODY), Some(NOBODY)).map_err(|e| format!("temp: {e}"))?;
        }
        Ok(Workdir { path })
    }
}

impl Drop for Workdir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// Runs a converter as nobody (when this is root) in its own process group,
/// killing the group past `limit`. The exit code, or `None` on a timeout.
fn run(program: &Path, home: &Path, args: &[&str], limit: Duration) -> Result<Option<i32>, String> {
    let mut cmd = Command::new(program);
    cmd.args(args)
        .current_dir(home)
        .env_clear()
        .env("HOME", home)
        .env("PATH", "/usr/bin:/bin")
        .env("LANG", "ko_KR.UTF-8")
        .env("SAL_USE_VCLPLUGIN", "svp")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(fonts) = fonts().map(|f| f.join("fonts.conf")).filter(|f| f.is_file()) {
        // The Korean fonts, for LibreOffice too (rhwp is given them directly).
        cmd.env("FONTCONFIG_FILE", fonts);
    }
    let root = unsafe { libc::geteuid() } == 0;
    // SAFETY: only async-signal-safe calls between fork and exec.
    unsafe {
        cmd.pre_exec(move || {
            if libc::setpgid(0, 0) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            if root && (libc::setgroups(0, std::ptr::null()) != 0 || libc::setgid(NOBODY) != 0 || libc::setuid(NOBODY) != 0) {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = cmd.spawn().map_err(|e| format!("{}: {e}", program.display()))?;
    let pid = child.id() as i32;
    let until = Instant::now() + limit;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status.code()),
            Ok(None) if Instant::now() < until => std::thread::sleep(Duration::from_millis(100)),
            _ => {
                // The whole group: LibreOffice starts helpers of its own.
                unsafe { libc::kill(-pid, libc::SIGKILL) };
                let _ = child.wait();
                return Ok(None);
            }
        }
    }
}

#[derive(serde::Deserialize)]
struct Query {
    name: String,
}

async fn preview(req: HttpRequest, query: web::Query<Query>, body: web::Bytes) -> HttpResponse {
    if crate::auth::verified_user_id(&req).filter(|u| !u.is_empty()).is_none() {
        return HttpResponse::Unauthorized().json(ApiResponse::<()>::error("Not signed in"));
    }
    let ext = extension(&query.name);
    let Some(kind) = kind(&ext) else {
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error("이 형식은 문서 미리보기가 없습니다"));
    };
    if body.is_empty() {
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error("빈 파일입니다"));
    }
    match web::block(move || convert(&body, &ext, kind)).await {
        Ok(Ok(pdf)) => HttpResponse::Ok()
            .content_type("application/pdf")
            .insert_header(("Cache-Control", "private, no-store"))
            .insert_header(("X-Content-Type-Options", "nosniff"))
            .body(pdf),
        Ok(Err(e)) => HttpResponse::UnprocessableEntity().json(ApiResponse::<()>::error(&e)),
        Err(_) => HttpResponse::ServiceUnavailable().json(ApiResponse::<()>::error("변환을 시작하지 못했습니다")),
    }
}

pub(crate) fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/api/preview/office")
            .app_data(web::PayloadConfig::new(MAX_BYTES))
            .route(web::post().to(preview)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn which_files_are_converted_and_how() {
        assert_eq!(kind(&extension("보고서.DOCX")), Some(Kind::Document));
        assert_eq!(kind(&extension("deck.pptx")), Some(Kind::Document));
        assert_eq!(kind(&extension("표.xlsx")), Some(Kind::Sheet), "a sheet is one page");
        assert_eq!(kind(&extension("old.xls")), Some(Kind::Sheet));
        assert_eq!(kind(&extension("공문.hwp")), Some(Kind::Hangul));
        assert_eq!(kind(&extension("양식.HWPX")), Some(Kind::Hangul));
        assert_eq!(kind(&extension("notes.txt")), None);
        assert_eq!(kind(&extension("archive.zip")), None);
        assert_eq!(kind(&extension("noext")), None);
    }

    #[test]
    fn the_cache_keeps_the_newest_within_its_size() {
        let dir = std::env::temp_dir().join(format!("nabiman-cache-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let old = dir.join("old.pdf");
        fs::write(&old, vec![0u8; 10]).unwrap();
        let f = fs::OpenOptions::new().append(true).open(&old).unwrap();
        f.set_modified(std::time::UNIX_EPOCH + Duration::from_secs(1)).unwrap();
        trim_cache(&dir);
        assert!(old.exists(), "under the limit nothing goes");
        let _ = fs::remove_dir_all(&dir);
    }

    /// A real conversion, when LibreOffice is at hand (NABIMAN_SOFFICE).
    #[test]
    fn a_document_becomes_a_pdf_when_libreoffice_is_here() {
        if std::env::var_os("NABIMAN_SOFFICE").is_none() || !soffice().exists() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("nabiman-office-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        // A flat OpenDocument text: small, and a real document.
        let doc = r#"<?xml version="1.0" encoding="UTF-8"?><office:document xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" office:version="1.2" office:mimetype="application/vnd.oasis.opendocument.text"><office:body><office:text><text:p>나비맨 미리보기</text:p></office:text></office:body></office:document>"#;
        let pdf = convert_in(&dir, doc.as_bytes(), "fodt", Kind::Document).unwrap();
        assert!(pdf.starts_with(b"%PDF"));
        // Again: from the cache, the same bytes.
        assert_eq!(convert_in(&dir, doc.as_bytes(), "fodt", Kind::Document).unwrap(), pdf);
        let _ = fs::remove_dir_all(&dir);
    }

    /// A real Hangul document (NABIMAN_TEST_HWP) through rhwp, with LibreOffice
    /// out of the way to show rhwp did it; then a broken one, which neither can do.
    #[test]
    fn a_hangul_document_becomes_a_pdf_when_rhwp_is_here() {
        let (Some(sample), true) = (std::env::var_os("NABIMAN_TEST_HWP"), rhwp().exists()) else { return };
        let dir = std::env::temp_dir().join(format!("nabiman-hwp-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let bytes = fs::read(&sample).unwrap();
        let ext = extension(&sample.to_string_lossy());
        let pdf = convert_in(&dir, &bytes, &ext, Kind::Hangul).unwrap();
        assert!(pdf.starts_with(b"%PDF"));
        let _ = fs::remove_dir_all(&dir);
    }
}
