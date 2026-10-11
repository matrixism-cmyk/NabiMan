use super::document;
use crate::bounded_command as runner;
use crate::{
    accounts::inventory,
    models::{CronJob, UserAccount},
};
use std::{collections::BTreeSet, path::PathBuf, time::Duration};
pub(super) struct Operations {
    pub program: PathBuf,
    pub passwd: PathBuf,
    pub spool: Vec<PathBuf>,
    pub authorize: Box<dyn Fn() -> bool + Send>,
    pub identity: Box<dyn Fn(&UserAccount) -> bool + Send>,
}
impl Operations {
    pub fn system() -> Self {
        Self {
            program: "/usr/bin/crontab".into(),
            passwd: "/etc/passwd".into(),
            spool: vec!["/var/spool/cron/crontabs".into(), "/var/spool/cron".into()],
            authorize: Box::new(|| false),
            identity: Box::new(inventory::nss_matches),
        }
    }
    pub fn account(&self, user: &str) -> Result<UserAccount, String> {
        if !inventory::name(user) {
            return Err("Invalid local account name".into());
        }
        let rows = inventory::parse(&inventory::read(&self.passwd, 1024 * 1024)?)?;
        let root_unique = rows.iter().filter(|r| r.uid == 0).count() == 1;
        rows.into_iter()
            .find(|r| r.username == user && (self.identity)(r))
            .map(|mut row| {
                if user == "root" && row.uid == 0 && root_unique {
                    row.can_manage = true;
                }
                row
            })
            .ok_or("Local account missing or inconsistent with NSS".into())
    }
    pub fn read(&self, user: &str) -> Result<String, String> {
        let out = runner::run(
            &self.program,
            &["-l", "-u", user],
            None,
            Duration::from_secs(5),
        )?;
        if out.status.success() {
            return Ok(out.out);
        }
        if out.status.code() == Some(1)
            && out.out.is_empty()
            && out.err.trim() == format!("no crontab for {user}")
        {
            return Ok(String::new());
        }
        Err("Cannot read the existing crontab; no empty-state fallback".into())
    }
    pub fn snapshot(&self, user: &str) -> Result<(UserAccount, String), String> {
        let row = self.account(user)?;
        let content = self.read(user)?;
        let after = self.account(user)?;
        if row.uid != after.uid || row.can_manage != after.can_manage {
            return Err("Account identity changed; reload".into());
        }
        document::parse(user, row.uid, &content)?;
        Ok((row, content))
    }
    pub fn list(&self) -> Result<Vec<CronJob>, String> {
        let mut names = BTreeSet::from(["root".to_string()]);
        let mut available = false;
        for dir in &self.spool {
            match std::fs::read_dir(dir) {
                Ok(entries) => {
                    available = true;
                    for entry in entries {
                        let entry = entry.map_err(|_| "Cannot read cron spool entry")?;
                        let name = entry
                            .file_name()
                            .into_string()
                            .map_err(|_| "Invalid cron spool name")?;
                        if !entry
                            .file_type()
                            .map_err(|_| "Cannot inspect cron spool entry")?
                            .is_file()
                            || !inventory::name(&name)
                        {
                            return Err(
                                "Unsupported cron spool entry; refusing incomplete inventory"
                                    .into(),
                            );
                        }
                        names.insert(name);
                        if names.len() > 128 {
                            return Err("Cron inventory exceeds 128 local accounts".into());
                        }
                    }
                    break;
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
                Err(_) => return Err("Cannot inspect cron spool; inventory unavailable".into()),
            }
        }
        if !available {
            return Err("Supported cron spool not found; inventory unavailable".into());
        }
        let mut jobs = vec![];
        for user in names {
            let (row, content) = self.snapshot(&user)?;
            let mut parsed = document::parse(&user, row.uid, &content)?;
            for job in &mut parsed {
                job.can_manage = row.can_manage;
            }
            jobs.extend(parsed);
        }
        Ok(jobs)
    }
    pub fn change(
        &self,
        user: &str,
        expected: &str,
        edit: impl FnOnce(&str, u32) -> Result<String, String>,
    ) -> Result<String, String> {
        let (row, before) = self.snapshot(user)?;
        if !(row.can_manage) {
            return Err("Protected local account; no change dispatched".into());
        }
        if expected != document::revision(user, row.uid, &before) {
            return Err(
                "Crontab or account changed since it was reviewed; reload before submitting".into(),
            );
        }
        let after = edit(&before, row.uid)?;
        let (current, recheck) = self.snapshot(user)?;
        if current.uid != row.uid || !current.can_manage || recheck != before {
            return Err("Crontab changed during planning; no change dispatched".into());
        }
        if !(self.authorize)() {
            return Err("Authorization changed before dispatch; no change dispatched".into());
        }
        let result = runner::run(
            &self.program,
            &["-u", user, "-"],
            Some(&after),
            Duration::from_secs(10),
        )?;
        if !result.status.success() {
            return Err(
                "Crontab installation failed; inspect current state before retrying".into(),
            );
        }
        let (current, observed) = self.snapshot(user).map_err(|_| {
            "Crontab was submitted but verification failed; outcome unknown. Reload before retrying"
        })?;
        if current.uid != row.uid || !current.can_manage || observed != after {
            return Err("Crontab was submitted but observed state differs; outcome unknown. Reload before retrying".into());
        }
        Ok("Crontab installed and read back; future command execution is not verified".into())
    }
}
