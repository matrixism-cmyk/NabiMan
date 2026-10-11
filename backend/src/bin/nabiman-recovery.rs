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
        println!("nabiman-recovery pack TREE OUTPUT_DIRECTORY | restore ARCHIVE NEW_DIRECTORY | verify TREE\npack/restore read a passphrase from stdin; never start the recovered server automatically.");
        return Ok(());
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
fn main() {
    if let Err(error) = execute() {
        eprintln!("Recovery failed: {error}");
        std::process::exit(1);
    }
}
