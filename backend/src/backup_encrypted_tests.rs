use super::*;
use std::{io::Write, path::PathBuf};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!("nb-age-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn secret() -> SecretString {
    SecretString::from("fixture long random passphrase".to_string())
}
fn archive(entries: Vec<(&str, tar::EntryType, &[u8])>) -> Vec<u8> {
    let gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    let mut builder = tar::Builder::new(gzip);
    for (path, kind, data) in entries {
        let mut h = tar::Header::new_gnu();
        h.set_entry_type(kind);
        h.set_size(data.len() as u64);
        h.set_mode(0o7777);
        if kind.is_symlink() || kind.is_hard_link() {
            h.set_link_name("/etc/passwd").unwrap();
        }
        h.set_cksum();
        builder.append_data(&mut h, path, data).unwrap();
    }
    builder.into_inner().unwrap().finish().unwrap()
}
#[test]
fn strict_extract_rejects_links_special_duplicates_and_truncation() {
    let f = Fixture::new();
    for (i, kind) in [
        tar::EntryType::Symlink,
        tar::EntryType::Link,
        tar::EntryType::Fifo,
        tar::EntryType::Char,
    ]
    .into_iter()
    .enumerate()
    {
        let root = f.0.join(i.to_string());
        restore::directory(&root).unwrap();
        assert!(restore::extract(&archive(vec![("bad", kind, &[])])[..], &root).is_err());
    }
    let tree = f.0.join("duplicate");
    restore::directory(&tree).unwrap();
    assert!(restore::extract(
        &archive(vec![
            ("same", tar::EntryType::Regular, b"one"),
            ("same", tar::EntryType::Regular, b"two")
        ])[..],
        &tree
    )
    .is_err());
    let tree = f.0.join("truncated");
    restore::directory(&tree).unwrap();
    let mut bytes = archive(vec![("file", tar::EntryType::Regular, b"contents")]);
    bytes.truncate(bytes.len() - 5);
    assert!(restore::extract(&bytes[..], &tree).is_err());
}
#[test]
fn unsafe_paths_and_publish_race_cannot_overwrite_existing_target() {
    let f = Fixture::new();
    assert!(restore::target("/tmp/../etc").is_err());
    assert!(restore::target("relative").is_err());
    let stage = Stage::new(&f.0).unwrap();
    let tree = stage.0.join("tree");
    restore::directory(&tree).unwrap();
    let target = f.0.join("target");
    assert!(restore::target(target.to_str().unwrap()).is_ok());
    fs::create_dir(&target).unwrap();
    fs::write(target.join("canary"), b"preserve").unwrap();
    assert!(restore::publish(&tree, &target).is_err());
    assert_eq!(fs::read(target.join("canary")).unwrap(), b"preserve");
    // Tar builder rejects traversal; manually mutate a valid header and recompute checksum.
    let mut data = Vec::new();
    {
        let mut b = tar::Builder::new(&mut data);
        let mut h = tar::Header::new_gnu();
        h.set_size(1);
        h.set_mode(0o600);
        h.set_cksum();
        b.append_data(&mut h, "normal", &b"x"[..]).unwrap();
        b.finish().unwrap();
    }
    data[..100].fill(0);
    data[..9].copy_from_slice(b"../escape");
    data[148..156].fill(b' ');
    let sum: u32 = data[..512].iter().map(|b| *b as u32).sum();
    data[148..156].copy_from_slice(format!("{sum:06o}\0 ").as_bytes());
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    gz.write_all(&data).unwrap();
    assert!(restore::extract(&gz.finish().unwrap()[..], &tree).is_err());
    assert!(!stage.0.join("escape").exists());
}
#[test]
fn age_roundtrip_and_tamper_never_publish_target() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    let source = f.0.join("source");
    fs::write(&source, b"secret fixture contents").unwrap();
    let dir = f.0.join("backups");
    let filename = encrypt(
        &dir,
        "fixture",
        &[source.to_str().unwrap().into()],
        secret(),
    )
    .unwrap();
    let original = fs::read(dir.join(&filename)).unwrap();
    assert!(original.starts_with(b"age-encryption.org/v1\n"));
    assert!(!original
        .windows(23)
        .any(|w| w == b"secret fixture contents"));
    assert_eq!(
        fs::metadata(dir.join(&filename))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    let target = f.0.join("restored");
    decrypt(&dir, &filename, target.to_str().unwrap(), secret()).unwrap();
    assert_eq!(
        fs::read(target.join(source.strip_prefix("/").unwrap())).unwrap(),
        b"secret fixture contents"
    );
    assert_eq!(
        fs::metadata(&target).unwrap().permissions().mode() & 0o777,
        0o700
    );
    let wrong = f.0.join("wrong");
    assert!(decrypt(
        &dir,
        &filename,
        wrong.to_str().unwrap(),
        SecretString::from("a completely wrong passphrase".to_string())
    )
    .is_err());
    assert!(!wrong.exists());
    for (i, mut data) in [original[..original.len() - 1].to_vec(), original.clone()]
        .into_iter()
        .enumerate()
    {
        if i == 1 {
            let last = data.len() - 1;
            data[last] ^= 1;
        }
        fs::write(dir.join(&filename), data).unwrap();
        let target = f.0.join(format!("tampered-{i}"));
        assert!(decrypt(&dir, &filename, target.to_str().unwrap(), secret()).is_err());
        assert!(!target.exists());
    }
    assert!(fs::read_dir(&dir).unwrap().all(|e| !e
        .unwrap()
        .file_name()
        .to_string_lossy()
        .starts_with('.')));
    assert!(fs::read_dir(&f.0).unwrap().all(|e| !e
        .unwrap()
        .file_name()
        .to_string_lossy()
        .starts_with('.')));
}
#[test]
fn excessive_kdf_header_rejected_before_work_and_source_links_not_created() {
    let f = Fixture::new();
    let dir = f.0.join("backups");
    fs::create_dir(&dir).unwrap();
    fs::write(dir.join("bad.tar.gz.age"),b"age-encryption.org/v1\n-> scrypt c2FsdHNhbHRzYWx0c2FsdA 99\nAA\n--- AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\n").unwrap();
    assert!(decrypt(
        &dir,
        "bad.tar.gz.age",
        f.0.join("out").to_str().unwrap(),
        secret()
    )
    .is_err());
    assert!(!f.0.join("out").exists());
    let source = f.0.join("links");
    fs::create_dir(&source).unwrap();
    std::os::unix::fs::symlink("/etc/passwd", source.join("link")).unwrap();
    assert!(encrypt(&dir, "links", &[source.to_str().unwrap().into()], secret()).is_err());
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
}
