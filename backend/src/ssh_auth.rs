//! Shared SSH invocation helpers.
//!
//! Passwords are never passed on the command line (`sshpass -p` would expose
//! them in `ps`) nor through the environment. They are written to a 0600 file
//! under `$NABIMAN_DATA_DIR/ssh/pw/` and handed to `sshpass -f`, then removed
//! as soon as the connection attempt is over.

use std::fs;
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

fn data_dir() -> String {
    std::env::var("NABIMAN_DATA_DIR").unwrap_or_else(|_| "/var/lib/nabiman".to_string())
}

fn pw_dir() -> String {
    format!("{}/ssh/pw", data_dir())
}

/// A temporary password file. Removed on drop unless [`PwFile::delete_after`]
/// hands it over to a timer (needed when ssh runs detached inside tmux).
pub struct PwFile {
    path: String,
    keep: bool,
}

impl PwFile {
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Keep the file around for `secs` so a detached `ssh` can still read it,
    /// then delete it in the background.
    pub fn delete_after(mut self, secs: u64) -> String {
        self.keep = true;
        let path = self.path.clone();
        let spawned = path.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(secs));
            let _ = fs::remove_file(&spawned);
        });
        path
    }
}

impl Drop for PwFile {
    fn drop(&mut self) {
        if !self.keep {
            let _ = fs::remove_file(&self.path);
        }
    }
}

pub fn write_password_file(password: &str) -> Result<PwFile, String> {
    let dir = pw_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("pw dir: {}", e))?;
    let _ = fs::set_permissions(&dir, fs::Permissions::from_mode(0o700));
    let name = {
        use rand::Rng;
        format!("{}/pw_{:016x}", dir, rand::thread_rng().gen::<u64>())
    };
    let mut f = fs::OpenOptions::new()
        .write(true).create(true).truncate(true).mode(0o600)
        .open(&name).map_err(|e| format!("pw file: {}", e))?;
    // sshpass -f reads the first line, so a trailing newline is expected.
    writeln!(f, "{}", password).map_err(|e| format!("pw write: {}", e))?;
    Ok(PwFile { path: name, keep: false })
}

/// Remove password files left behind by a crash/restart.
pub fn cleanup_password_files() {
    if let Ok(entries) = fs::read_dir(pw_dir()) {
        for e in entries.flatten() {
            let _ = fs::remove_file(e.path());
        }
    }
}

/// Options that force password authentication (so ssh does not silently fall
/// back to a key and then hang on an unanswerable prompt).
pub fn password_auth_opts() -> Vec<&'static str> {
    vec![
        "-o", "PreferredAuthentications=password,keyboard-interactive",
        "-o", "PubkeyAuthentication=no",
        "-o", "NumberOfPasswordPrompts=1",
    ]
}

/// TCP/SSH level keep-alive: the server side never lets an idle session die.
pub fn keepalive_opts(interval_secs: u64) -> Vec<String> {
    vec![
        "-o".into(), format!("ServerAliveInterval={}", interval_secs.clamp(5, 3600)),
        "-o".into(), "ServerAliveCountMax=1000000".into(),
        "-o".into(), "TCPKeepAlive=yes".into(),
    ]
}
