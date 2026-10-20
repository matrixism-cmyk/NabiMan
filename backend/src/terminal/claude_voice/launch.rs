//! Finding Claude Code and starting it as the account it is signed in under.

use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;

use super::Settings;

/// Where Claude Code is usually installed, besides `$PATH`.
const PLACES: [&str; 4] = ["/workspace/.npm-global/bin/claude", "/usr/local/bin/claude", "/usr/bin/claude", "/opt/claude/bin/claude"];
/// Per-user installs, under each account's home.
const HOME_PLACES: [&str; 3] = [".local/bin/claude", ".npm-global/bin/claude", ".claude/local/claude"];

pub(crate) fn valid_user(name: &str) -> bool {
    !name.is_empty() && name.len() <= 32
        && name.chars().next().is_some_and(|c| c.is_ascii_lowercase() || c == '_')
        && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

struct Account { name: String, uid: u32, home: String }

fn accounts() -> Vec<Account> {
    std::fs::read_to_string("/etc/passwd").unwrap_or_default().lines().filter_map(|line| {
        let f: Vec<&str> = line.split(':').collect();
        if f.len() < 7 {
            return None;
        }
        Some(Account { name: f[0].into(), uid: f[2].parse().ok()?, home: f[5].into() })
    }).collect()
}

fn runnable(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.mode() & 0o111 != 0)
}

/// The `claude` program: the one set, or the first found.
fn find(set: &str) -> Result<PathBuf, String> {
    if !set.is_empty() {
        let p = PathBuf::from(set);
        return if runnable(&p) { Ok(p) } else { Err(format!("{set} is not a program that can be run")) };
    }
    let from_path = std::env::var("PATH").unwrap_or_default().split(':').map(|d| Path::new(d).join("claude")).collect::<Vec<_>>();
    let homes = accounts().into_iter().filter(|a| a.uid == 0 || a.uid >= 1000)
        .flat_map(|a| HOME_PLACES.iter().map(move |p| Path::new(&a.home).join(p))).collect::<Vec<_>>();
    from_path.into_iter().chain(PLACES.iter().map(PathBuf::from)).chain(homes)
        .find(|p| runnable(p))
        .ok_or_else(|| "Claude Code (claude) was not found on this server; set where it is".into())
}

/// How to start Claude: which program, as whom, from where.
pub(crate) struct Launch {
    pub program: String,
    pub user: String,
    pub home: String,
    /// NabiMan is root and Claude runs as someone else: through `runuser`.
    switch: bool,
}

impl Launch {
    pub(crate) fn resolve(s: &Settings) -> Result<Launch, String> {
        let program = find(&s.claude_path)?;
        let all = accounts();
        // By default, the account that installed it: that is the one signed in.
        let owner_uid = std::fs::metadata(std::fs::canonicalize(&program).unwrap_or(program.clone())).map(|m| m.uid()).unwrap_or(0);
        let wanted = if s.run_as.is_empty() {
            all.iter().find(|a| a.uid == owner_uid).ok_or("The owner of claude has no account here")?
        } else {
            all.iter().find(|a| a.name == s.run_as).ok_or_else(|| format!("There is no account {}", s.run_as))?
        };
        let me = unsafe { libc::geteuid() };
        if me != 0 && wanted.uid != me {
            let name = all.iter().find(|a| a.uid == me).map(|a| a.name.clone()).unwrap_or_else(|| me.to_string());
            return Err(format!("NabiMan runs as {name}, so it cannot start Claude as {}", wanted.name));
        }
        Ok(Launch {
            program: program.to_string_lossy().into_owned(),
            user: wanted.name.clone(),
            home: wanted.home.clone(),
            switch: me == 0 && wanted.uid != 0,
        })
    }

    /// `claude` with `args`, as the account, in its home, with a clean environment.
    pub(crate) fn command(&self, args: &[String]) -> Command {
        let mut cmd = if self.switch {
            // Not on the clean PATH below: by its full name.
            let runuser = ["/usr/sbin/runuser", "/sbin/runuser", "/usr/bin/runuser"].into_iter()
                .find(|p| Path::new(p).exists()).unwrap_or("runuser");
            let mut c = Command::new(runuser);
            c.args(["-u", &self.user, "--", &self.program]);
            c
        } else {
            Command::new(&self.program)
        };
        let bin = Path::new(&self.program).parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
        cmd.args(args)
            .env_clear()
            .env("HOME", &self.home)
            .env("USER", &self.user)
            .env("LOGNAME", &self.user)
            .env("LANG", "C.UTF-8")
            .env("PATH", format!("{bin}:/usr/local/bin:/usr/bin:/bin"))
            .current_dir(&self.home)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        cmd
    }

    pub(crate) async fn version(&self) -> Result<String, String> {
        let mut cmd = self.command(&["--version".into()]);
        cmd.stdin(Stdio::null());
        let out = tokio::time::timeout(Duration::from_secs(30), cmd.output()).await
            .map_err(|_| "Claude did not answer in 30 seconds".to_string())?
            .map_err(|e| format!("Cannot start Claude: {e}"))?;
        if !out.status.success() {
            return Err(format!("Claude failed: {}", String::from_utf8_lossy(&out.stderr).chars().take(300).collect::<String>()));
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_names_are_plain() {
        assert!(valid_user("nabi") && valid_user("_svc-1"));
        assert!(!valid_user("Root") && !valid_user("a b") && !valid_user("-x") && !valid_user("../x") && !valid_user(""));
    }

    #[test]
    fn a_missing_program_is_said_so() {
        let s = Settings { claude_path: "/nonexistent/claude".into(), ..Settings::default() };
        assert!(Launch::resolve(&s).is_err());
    }
}
