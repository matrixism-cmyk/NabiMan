//! Offline only. Passphrase is read from stdin, never argv or an environment variable.
use age::secrecy::SecretString;
use nabiman_server::{backup_crypto as crypto, recovery_bundle as bundle};
use std::{
    io::{self, Read},
    path::PathBuf,
};
fn execute() -> Result<(), String> {
    if unsafe { libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0) } != 0 {
        return Err("Cannot protect recovery process".into());
    }
    let args: Vec<_> = std::env::args().collect();
    if args.len() == 2 && args[1] == "--help" {
        println!("nabiman-recovery pack TREE OUTPUT_DIRECTORY | restore ARCHIVE NEW_DIRECTORY | verify TREE\npack/restore read a passphrase from stdin; never start the recovered server automatically.\n\nnabiman-recovery snapshot DATA_DIR OUTPUT_DIRECTORY [--with-secret-key] [--mec-db FILE]\nnabiman-recovery restore-state ARCHIVE NEW_DIRECTORY [--secret-key FILE]\nsnapshot is safe while the server runs (databases are copied consistently) and leaves secret.key out unless asked;\nrestore-state writes only a new directory and checks the key against the stored credentials.");
        return Ok(());
    }
    if matches!(args.get(1).map(String::as_str), Some("snapshot" | "restore-state")) {
        return state(&args[1..]);
    }
    if args.len() == 3 && args[1] == "verify" {
        let result = bundle::verify(&PathBuf::from(&args[2]))?;
        println!(
            "{}",
            serde_json::to_string(&result).map_err(|_| "Cannot encode verification")?
        );
        return Ok(());
    }
    if args.len() != 4 || !matches!(args[1].as_str(), "pack" | "restore") {
        return Err("Use --help for recovery command syntax".into());
    }
    let mut bytes = String::new();
    io::stdin()
        .take(1026)
        .read_to_string(&mut bytes)
        .map_err(|_| "Cannot read passphrase")?;
    if bytes.ends_with('\n') {
        bytes.pop();
        if bytes.ends_with('\r') {
            bytes.pop();
        }
    }
    let password = SecretString::from(bytes);
    if !crypto::password_valid(&password) {
        return Err("Invalid recovery passphrase (12 to 1024 characters/bytes)".into());
    }
    let input = PathBuf::from(&args[2]);
    let output = PathBuf::from(&args[3]);
    if args[1] == "pack" {
        let manifest_hash = bundle::digest(&input.join("manifest.json"))?;
        bundle::verify(&input)?;
        let file = crypto::encrypt_tree(&output, &input, password)?;
        if bundle::verify(&input).is_err()
            || bundle::digest(&input.join("manifest.json"))? != manifest_hash
        {
            let _ = std::fs::remove_file(output.join(&file));
            return Err("Bundle changed while encrypting".into());
        }
        println!("{}", serde_json::json!({"filename":file}));
    } else {
        let dir = input.parent().ok_or("Invalid archive path")?;
        let name = input
            .file_name()
            .and_then(|v| v.to_str())
            .ok_or("Invalid archive name")?;
        crypto::decrypt_validated(
            dir,
            name,
            output.to_str().ok_or("Invalid recovery target")?,
            password,
            bundle::prepare,
        )?;
        let result = bundle::verify(&output)?;
        println!(
            "{}",
            serde_json::to_string(&result).map_err(|_| "Cannot encode verification")?
        );
    }
    Ok(())
}
/// Data directory snapshots (nabiman_server::state_snapshot). No passphrase: the archive
/// is as private as the data directory itself (0600) and holds no key unless asked to.
/// Takes no data directory lock: a snapshot only reads, and a restore only creates a new directory.
fn state(args: &[String]) -> Result<(), String> {
    use nabiman_server::state_snapshot as snapshot;
    let usage = || "Use --help for snapshot / restore-state syntax".to_string();
    let (command, positional, flags) = (args[0].as_str(), args.get(1..3).ok_or_else(usage)?, &args[3..]);
    let mut options = snapshot::Options::default();
    let mut key = None;
    let mut rest = flags.iter();
    while let Some(flag) = rest.next() {
        match (command, flag.as_str()) {
            ("snapshot", "--with-secret-key") => options.include_secret_key = true,
            ("snapshot", "--mec-db") => options.mec_db = Some(PathBuf::from(rest.next().ok_or_else(usage)?)),
            ("restore-state", "--secret-key") => key = Some(PathBuf::from(rest.next().ok_or_else(usage)?)),
            _ => return Err(usage()),
        }
    }
    let result = if command == "snapshot" {
        let name = snapshot::create(&PathBuf::from(&positional[0]), &PathBuf::from(&positional[1]), "nabiman-state", &options)?;
        serde_json::json!({ "filename": name, "secret_key_included": options.include_secret_key })
    } else {
        let restored = snapshot::restore(&PathBuf::from(&positional[0]), &positional[1], key.as_deref())?;
        serde_json::to_value(restored).map_err(|_| "Cannot encode restore summary")?
    };
    println!("{result}");
    Ok(())
}
fn main() {
    if let Err(error) = execute() {
        eprintln!("Recovery failed: {error}");
        std::process::exit(1);
    }
}
