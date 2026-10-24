//! NabiMan as an SSH certificate authority, so a server can be reached
//! without a password stored here: install NabiMan's CA public key on the
//! server (`TrustedUserCAKeys`) and NabiMan signs a short-lived certificate
//! for each connection. Opt-in per server (`auth_method = "cert"`); nothing
//! changes for password- or key-authenticated servers.

use std::path::PathBuf;
use std::process::Command;

use crate::models::RemoteServer;

/// How long a signed certificate is valid — long enough to connect, short
/// enough that a leaked one is useless within minutes.
const CERT_VALIDITY: &str = "+5m";
/// How long the certificate file lingers on disk so a detached `ssh` (inside
/// tmux) can read it, before it is swept.
const CERT_FILE_TTL_SECS: u64 = 60;

fn ca_dir() -> PathBuf {
    nabiman_server::data_dir().join("ssh-ca")
}
fn ca_key() -> PathBuf {
    ca_dir().join("ca")
}
fn ca_pub() -> PathBuf {
    ca_dir().join("ca.pub")
}
fn cert_dir() -> PathBuf {
    nabiman_server::data_dir().join("ssh").join("certs")
}

/// Makes the CA key pair if it is not there yet.
pub fn ensure_ca() -> Result<(), String> {
    if ca_key().exists() && ca_pub().exists() {
        return Ok(());
    }
    std::fs::create_dir_all(ca_dir()).map_err(|e| format!("CA dir: {e}"))?;
    let _ = std::fs::set_permissions(ca_dir(), std::os::unix::fs::PermissionsExt::from_mode(0o700));
    // A fresh CA; -N '' = no passphrase so the server can sign unattended.
    let out = Command::new("ssh-keygen")
        .args(["-t", "ed25519", "-f", &ca_key().to_string_lossy(), "-N", "", "-C", "nabiman-ssh-ca", "-q"])
        .output()
        .map_err(|e| format!("ssh-keygen: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(())
}

/// The CA public key line, for the administrator to install on servers.
pub fn ca_public_key() -> Result<String, String> {
    ensure_ca()?;
    Ok(std::fs::read_to_string(ca_pub()).map_err(|e| format!("CA public key: {e}"))?.trim().to_string())
}

/// A signed certificate and the key it certifies, with the files removed
/// shortly after so a leaked copy cannot be reused.
pub struct CertAuth {
    pub identity: String,
    pub cert: String,
}

/// Signs a short-lived certificate for reaching `server` as its user.
pub fn sign_for(server: &RemoteServer) -> Result<CertAuth, String> {
    ensure_ca()?;
    let identity = crate::ssh_keys::nabiman_key_path();
    let pubkey = crate::ssh_keys::nabiman_pubkey_path();
    crate::ssh_keys::ensure_ssh_key()?;
    std::fs::create_dir_all(cert_dir()).map_err(|e| format!("cert dir: {e}"))?;
    let _ = std::fs::set_permissions(cert_dir(), std::os::unix::fs::PermissionsExt::from_mode(0o700));

    // Sign a copy of the managed public key, so concurrent connections never
    // overwrite one another's certificate (ssh-keygen writes <name>-cert.pub).
    let stem = cert_dir().join(uuid::Uuid::new_v4().simple().to_string());
    let pub_copy = format!("{}.pub", stem.display());
    std::fs::copy(&pubkey, &pub_copy).map_err(|e| format!("cert: {e}"))?;
    let principal = &server.user;
    let id = format!("nabiman-{}", uuid::Uuid::new_v4().simple());
    let out = Command::new("ssh-keygen")
        .args([
            "-s", &ca_key().to_string_lossy(),
            "-I", &id,
            "-n", principal,
            "-V", CERT_VALIDITY,
            &pub_copy,
        ])
        .output()
        .map_err(|e| format!("ssh-keygen sign: {e}"))?;
    if !out.status.success() {
        let _ = std::fs::remove_file(&pub_copy);
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    let cert = format!("{}-cert.pub", stem.display());
    sweep_after(vec![pub_copy, cert.clone()], CERT_FILE_TTL_SECS);
    Ok(CertAuth { identity, cert })
}

/// Removes the files after `secs`, so a detached ssh has read them by then.
fn sweep_after(paths: Vec<String>, secs: u64) {
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(secs));
        for p in paths {
            let _ = std::fs::remove_file(p);
        }
    });
}

/// The ssh options that use a certificate and nothing else.
pub fn ssh_opts(auth: &CertAuth) -> Vec<String> {
    vec![
        "-i".into(),
        auth.identity.clone(),
        "-o".into(),
        format!("CertificateFile={}", auth.cert),
        "-o".into(),
        "IdentitiesOnly=yes".into(),
        "-o".into(),
        "PreferredAuthentications=publickey".into(),
        "-o".into(),
        "PubkeyAuthentication=yes".into(),
    ]
}

/// Whether a server is set to authenticate with a NabiMan certificate.
pub fn uses_cert(server: &RemoteServer) -> bool {
    server.auth_method == "cert" && !server.member_owned()
}
