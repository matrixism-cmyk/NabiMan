use super::inventory;
use crate::models::{
    ChangePasswordRequest, CreateAccountRequest, DeleteAccountRequest, UserAccount,
};
use std::{
    fs,
    io::Write,
    os::unix::process::CommandExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};
/// Fixed production paths. Alternative paths exist only in internal tests, never HTTP/config input.
pub(super) struct Operations {
    pub passwd: PathBuf,
    pub shells: PathBuf,
    pub home: PathBuf,
    pub useradd: PathBuf,
    pub userdel: PathBuf,
    pub chpasswd: PathBuf,
    pub timeout: Duration,
    pub authorize: Box<dyn Fn() -> bool + Send>,
    pub verify_identity: Box<dyn Fn(&UserAccount) -> bool + Send>,
}
impl Operations {
    pub fn system() -> Self {
        Self {
            passwd: "/etc/passwd".into(),
            shells: "/etc/shells".into(),
            home: "/home".into(),
            useradd: "/usr/sbin/useradd".into(),
            userdel: "/usr/sbin/userdel".into(),
            chpasswd: "/usr/sbin/chpasswd".into(),
            timeout: Duration::from_secs(30),
            authorize: Box::new(|| false),
            verify_identity: Box::new(inventory::nss_matches),
        }
    }
    fn execute(&self, program: &Path, args: &[&str], input: Option<&str>) -> Result<(), String> {
        if !(self.authorize)() {
            return Err(
                "Authorization changed before dispatch; inspect any earlier partial effects".into(),
            );
        }
        run(program, args, input, self.timeout)
    }
    pub fn inventory(&self) -> Result<Vec<UserAccount>, String> {
        inventory::parse(&inventory::read(&self.passwd, 1024 * 1024)?)
    }
    fn target(&self, name: &str, uid: u32) -> Result<UserAccount, String> {
        if !inventory::name(name) {
            return Err("Invalid local account name".into());
        }
        self.inventory()?
            .into_iter()
            .find(|r| r.username == name && r.uid == uid && r.can_manage && (self.verify_identity)(r))
            .ok_or_else(|| {
                "Account missing, protected, shared UID, changed or inconsistent with NSS; reload the account inventory"
                    .into()
            })
    }
    pub fn create(&self, body: CreateAccountRequest) -> Result<String, String> {
        if !inventory::name(&body.username) || !inventory::password(&body.password) {
            return Err("Use a lowercase POSIX account name (1–32 characters) and a 12–256 byte password without control characters".into());
        }
        let shell = body.shell.as_deref().unwrap_or("/bin/bash");
        if !inventory::shells(&self.shells)?.iter().any(|s| s == shell) {
            return Err("Choose a shell explicitly listed in /etc/shells".into());
        }
        if self
            .inventory()?
            .iter()
            .any(|r| r.username == body.username)
        {
            return Err("Local account already exists; no change dispatched".into());
        }
        let home = self.home.join(&body.username);
        match fs::symlink_metadata(&home) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            _ => {
                return Err(
                    "Home path exists or cannot be inspected; existing data will not be adopted"
                        .into(),
                )
            }
        }
        self.execute(
            &self.useradd,
            &[
                "-K",
                "UID_MIN=1000",
                "-K",
                "UID_MAX=65533",
                "-m",
                "-d",
                home.to_str().ok_or("Invalid home path")?,
                "-s",
                shell,
                "--",
                &body.username,
            ],
            None,
        )?;
        let row = self
            .inventory()?
            .into_iter()
            .find(|r| {
                r.username == body.username && r.can_manage && r.home == home.to_string_lossy()
            })
            .ok_or("Account creation could not be verified; inspect local state before retrying")?;
        self.target(&row.username, row.uid)?;
        self.execute(&self.chpasswd,&[],Some(&format!("{}:{}\n",body.username,body.password)))
            .map_err(|_| "Account may have been created but password setup was not confirmed; no account or home was automatically removed. Inspect local state before retrying".to_string())?;
        self.target(&row.username, row.uid)?;
        Ok(
            "Local account created; account identity verified and password command completed"
                .into(),
        )
    }
    pub fn password(&self, body: ChangePasswordRequest) -> Result<String, String> {
        if !inventory::password(&body.new_password) {
            return Err("Password must be 12–256 bytes without control characters".into());
        }
        self.target(&body.username, body.expected_uid)?;
        self.execute(
            &self.chpasswd,
            &[],
            Some(&format!("{}:{}\n", body.username, body.new_password)),
        )?;
        self.target(&body.username, body.expected_uid)?;
        Ok("Password command completed for the verified local account; existing sessions are not revoked".into())
    }
    pub fn delete(&self, body: DeleteAccountRequest) -> Result<String, String> {
        let row = self.target(&body.username, body.expected_uid)?;
        let mut args = vec![];
        if body.remove_home {
            let expected = self.home.join(&body.username);
            if row.home != expected.to_string_lossy()
                || body.confirm_home.as_deref() != Some(row.home.as_str())
                || self
                    .inventory()?
                    .iter()
                    .any(|r| r.username != row.username && r.home == row.home)
                || match fs::symlink_metadata(&expected) {
                    Ok(m) => m.file_type().is_symlink(),
                    Err(e) => e.kind() != std::io::ErrorKind::NotFound,
                }
            {
                return Err("Home removal requires the exact dedicated /home/username path and explicit confirmation; shared or symbolic-link homes are refused".into());
            }
            args.push("-r");
        }
        args.extend(["--", &body.username]);
        self.target(&body.username, body.expected_uid)?;
        self.execute(&self.userdel, &args, None)?;
        if self
            .inventory()?
            .iter()
            .any(|r| r.username == body.username)
        {
            return Err("Deletion was not confirmed; inspect local state before retrying".into());
        }
        if body.remove_home && fs::symlink_metadata(&row.home).is_ok() {
            return Err(
                "Account removed but home removal was not confirmed; inspect retained data".into(),
            );
        }
        Ok(if body.remove_home { "Local account removed; home removal confirmed. Other owned files are outside this operation" } else { "Local account removed without requesting home/mail-spool removal; OS-configured deletion hooks may have additional effects" }.into())
    }
}
pub(super) fn run(
    program: &Path,
    args: &[&str],
    input: Option<&str>,
    deadline: Duration,
) -> Result<(), String> {
    let mut command = Command::new(program);
    command
        .args(args)
        .env_clear()
        .env("PATH", "/usr/sbin:/usr/bin:/sbin:/bin")
        .env("LANG", "C")
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0);
    let mut child = command
        .spawn()
        .map_err(|_| "Account tool could not start; no command dispatched")?;
    if let Some(input) = input {
        if child
            .stdin
            .take()
            .ok_or("Account tool input unavailable")?
            .write_all(input.as_bytes())
            .is_err()
        {
            unsafe {
                libc::kill(-(child.id() as i32), libc::SIGKILL);
            }
            let _ = child.wait();
            return Err("Password command input failed; outcome unknown, inspect local state before retrying".into());
        }
    }
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return if status.success() {
                    Ok(())
                } else {
                    Err("Account tool reported failure; partial effects are possible. Inspect local state before retrying".into())
                }
            }
            Ok(None) if started.elapsed() < deadline => {
                std::thread::sleep(Duration::from_millis(20))
            }
            _ => {
                unsafe {
                    libc::kill(-(child.id() as i32), libc::SIGKILL);
                }
                let _ = child.wait();
                return Err("Account tool deadline or wait failed; outcome unknown. Inspect local state before retrying".into());
            }
        }
    }
}
