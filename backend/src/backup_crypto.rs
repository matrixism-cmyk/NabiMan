//! Shared authenticated file encryption for the HTTP API and offline recovery tool.
use age::secrecy::{ExposeSecret, SecretString};
use std::os::unix::fs::OpenOptionsExt;
use std::{
    fs,
    io::{self, Read, Seek},
    path::Path,
};
#[path = "backup_restore.rs"]
mod restore;
#[path = "backup_encrypted_source.rs"]
mod source;
use restore::{Stage, MAX_BYTES};
pub fn validate(name: &str, paths: &[String]) -> Result<(), String> {
    if name.trim().is_empty()
        || name.len() > 120
        || name
            .chars()
            .any(|c| c.is_control() || c == '/' || c == '\\')
    {
        return Err("Invalid backup name".into());
    }
    if paths.is_empty()
        || paths.len() > 64
        || paths.iter().any(|p| {
            let path = Path::new(p);
            !path.is_absolute()
                || p.len() > 4096
                || p.chars().any(char::is_control)
                || path
                    .components()
                    .any(|c| matches!(c, std::path::Component::ParentDir))
        })
    {
        return Err("Use 1 to 64 absolute paths without parent traversal".into());
    }
    Ok(())
}
pub fn password_valid(password: &SecretString) -> bool {
    let p = password.expose_secret();
    p.chars().count() >= 12 && p.len() <= 1024 && !p.chars().any(char::is_control)
}
pub fn encrypt(
    dir: &Path,
    name: &str,
    paths: &[String],
    password: SecretString,
) -> Result<String, String> {
    validate(name, paths)?;
    encrypt_with(dir, password, |writer| source::write(writer, paths, dir))
}
pub fn encrypt_tree(dir: &Path, root: &Path, password: SecretString) -> Result<String, String> {
    encrypt_with(dir, password, |writer| {
        source::write_tree(writer, root, dir)
    })
}
fn encrypt_with(
    dir: &Path,
    password: SecretString,
    write: impl FnOnce(
        age::stream::StreamWriter<fs::File>,
    ) -> Result<age::stream::StreamWriter<fs::File>, String>,
) -> Result<String, String> {
    fs::create_dir_all(dir).map_err(|_| "Backup storage unavailable")?;
    let stage = Stage::new(dir)?;
    let output = stage.0.join("encrypted.age");
    let mut recipient = age::scrypt::Recipient::new(password);
    recipient.set_work_factor(18);
    let encryptor =
        age::Encryptor::with_recipients(std::iter::once(&recipient as &dyn age::Recipient))
            .map_err(|_| "Cannot initialize encryption")?;
    let writer = encryptor
        .wrap_output(restore::private_file(&output).map_err(|_| "Cannot stage encrypted backup")?)
        .map_err(|_| "Cannot initialize encryption")?;
    let writer = write(writer)?;
    writer
        .finish()
        .map_err(|_| "Cannot finish authenticated encryption")?
        .sync_all()
        .map_err(|_| "Cannot persist encrypted backup")?;
    let filename = format!("backup-{}.tar.gz.age", uuid::Uuid::new_v4());
    fs::hard_link(&output, dir.join(&filename)).map_err(|_| "Cannot publish encrypted backup")?;
    fs::File::open(dir)
        .and_then(|f| f.sync_all())
        .map_err(|_| "Encrypted backup published but durability could not be confirmed")?;
    Ok(filename)
}
pub fn decrypt(
    dir: &Path,
    filename: &str,
    target_dir: &str,
    password: SecretString,
) -> Result<String, String> {
    decrypt_validated(dir, filename, target_dir, password, |_| Ok(()))
}
pub fn decrypt_validated(
    dir: &Path,
    filename: &str,
    target_dir: &str,
    password: SecretString,
    check: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<String, String> {
    if !filename.ends_with(".tar.gz.age")
        || filename.contains('/')
        || filename.contains('\\')
        || filename.starts_with('.')
        || filename.len() > 160
    {
        return Err("Invalid age backup filename".into());
    }
    let target = restore::target(target_dir)?;
    let mut file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(dir.join(filename))
        .map_err(|_| "Backup unavailable")?;
    let metadata = file.metadata().map_err(|_| "Backup unavailable")?;
    if !metadata.is_file() || metadata.len() > MAX_BYTES + 2 * 1024 * 1024 {
        return Err("Backup is not a regular file or exceeds size limit".into());
    }
    // Our password-only format has a small header. Reject excessive/foreign headers before KDF work.
    let mut header = [0u8; 1024];
    let count = file.read(&mut header).map_err(|_| "Cannot read backup")?;
    let header = String::from_utf8_lossy(&header[..count]);
    if !header.starts_with("age-encryption.org/v1\n-> scrypt ")
        || !header.contains("\n--- ")
        || header
            .split("\n--- ")
            .next()
            .is_none_or(|prefix| prefix.matches("\n-> ").count() != 1)
    {
        return Err("Unsupported age password header".into());
    }
    file.rewind().map_err(|_| "Cannot read backup")?;
    let stage = Stage::new(target.parent().ok_or("Missing restore parent")?)?;
    let plain = stage.0.join("authenticated.tar.gz");
    let mut output = restore::private_file(&plain).map_err(|_| "Cannot stage restore")?;
    let decryptor = age::Decryptor::new(file).map_err(|_| "Invalid encrypted backup")?;
    let mut identity = age::scrypt::Identity::new(password);
    identity.set_max_work_factor(18);
    let mut reader = decryptor
        .decrypt(std::iter::once(&identity as &dyn age::Identity))
        .map_err(|_| "Wrong passphrase, invalid backup or unsupported work factor")?
        .take(MAX_BYTES + 1024 * 1024 + 1);
    let bytes = io::copy(&mut reader, &mut output)
        .map_err(|_| "Backup authentication failed; no restore was published")?;
    if bytes > MAX_BYTES + 1024 * 1024 {
        return Err("Decrypted archive exceeds 1 GiB".into());
    }
    output
        .sync_all()
        .map_err(|_| "Cannot persist staged restore")?;
    drop(output);
    let tree = stage.0.join("tree");
    restore::directory(&tree).map_err(|_| "Cannot stage restore tree")?;
    restore::extract(
        fs::File::open(plain).map_err(|_| "Cannot read authenticated archive")?,
        &tree,
    )?;
    check(&tree)?;
    restore::publish(&tree, &target)?;
    Ok("Authenticated backup restored to new directory; review contents before applying".into())
}

#[cfg(test)]
#[path = "backup_encrypted_tests.rs"]
mod tests;
