use super::*;
use std::io::Write;

/// Writes `files` (name, content, deflate, big) and some folders into an archive.
fn archive(files: &[(&str, &[u8], bool, bool)], dirs: &[&str]) -> Vec<u8> {
    let mut zip = ZipWriter::default();
    let mut out = Vec::new();
    for d in dirs {
        out.extend(zip.dir(d, Some(1_700_000_000), 0o755));
    }
    for (name, content, deflate, big) in files {
        out.extend(zip.begin_file(name, Some(1_700_000_000), 0o644, *deflate, *big));
        let data = if *deflate {
            let mut enc = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::fast());
            enc.write_all(content).unwrap();
            enc.finish().unwrap()
        } else {
            content.to_vec()
        };
        zip.wrote(data.len());
        out.extend(&data);
        out.extend(zip.end_file(crc32fast::hash(content), data.len() as u64, content.len() as u64).unwrap());
    }
    out.extend(zip.finish());
    out
}

/// Another implementation's opinion: Python's zipfile, when it is installed.
fn python_reads(bytes: &[u8], script: &str) -> Option<String> {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let path = std::env::temp_dir().join(format!("nabiman-zip-test-{}-{nanos}.zip", std::process::id()));
    std::fs::write(&path, bytes).unwrap();
    let out = std::process::Command::new("python3").arg("-c").arg(script).arg(&path).output();
    let _ = std::fs::remove_file(&path);
    let out = out.ok()?;
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    Some(String::from_utf8(out.stdout).unwrap())
}

#[test]
fn another_reader_opens_it_with_names_contents_and_folders() {
    let text = "로그 한 줄\n".repeat(2000);
    let bytes = archive(
        &[
            ("폴더/로그.txt", text.as_bytes(), true, false),
            ("폴더/a.bin", &[0u8, 1, 2, 3, 255], false, false),
            ("폴더/큰파일.iso", b"flagged big, still small", false, true),
        ],
        &["폴더", "폴더/빈 폴더"],
    );
    let script = r#"
import sys, zipfile
z = zipfile.ZipFile(sys.argv[1])
assert z.testzip() is None
for i in z.infolist():
    print(i.filename, i.file_size, i.compress_type, oct(i.external_attr >> 16), i.is_dir())
print(z.read('폴더/로그.txt').decode().count('로그'))
"#;
    let Some(seen) = python_reads(&bytes, script) else { return };
    let lines: Vec<&str> = seen.lines().collect();
    assert_eq!(lines[0], "폴더/ 0 0 0o40755 True");
    assert_eq!(lines[1], "폴더/빈 폴더/ 0 0 0o40755 True");
    assert_eq!(lines[2], format!("폴더/로그.txt {} 8 0o100644 False", text.len()));
    assert_eq!(lines[3], "폴더/a.bin 5 0 0o100644 False");
    assert_eq!(lines[4], "폴더/큰파일.iso 24 0 0o100644 False");
    assert_eq!(lines[5], "2000");
}

#[test]
fn more_entries_than_classic_zip_can_count() {
    let dirs: Vec<String> = (0..70_000).map(|i| format!("d/{i}")).collect();
    let refs: Vec<&str> = dirs.iter().map(String::as_str).collect();
    let bytes = archive(&[("d/x.txt", b"x", false, false)], &refs);
    let script = "import sys, zipfile\nz = zipfile.ZipFile(sys.argv[1])\nprint(len(z.infolist()), z.read('d/x.txt'))";
    let Some(seen) = python_reads(&bytes, script) else { return };
    assert_eq!(seen.trim(), "70001 b'x'");
}

#[test]
fn a_small_entry_cannot_quietly_overflow() {
    let mut zip = ZipWriter::default();
    zip.begin_file("f", None, 0o644, false, false);
    assert!(zip.end_file(0, 5 << 30, 5 << 30).is_err());
}

#[test]
fn times_are_dos_times() {
    // Before 1980 there is no DOS date: it becomes 1980-01-01.
    assert_eq!(dos_time(Some(0)), (0, (1 << 5) | 1));
    let (_, date) = dos_time(Some(1_700_000_000));
    assert_eq!(date >> 9, 2023 - 1980);
}

#[test]
fn compressed_formats_are_stored() {
    assert!(worth_compressing("app.log"));
    assert!(worth_compressing("Makefile"));
    assert!(!worth_compressing("movie.MP4"));
    assert!(!worth_compressing("backup.tar.gz"));
}
