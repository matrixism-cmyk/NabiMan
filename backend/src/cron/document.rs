use crate::models::CronJob;
use sha2::{Digest, Sha256};
pub(super) fn revision(user: &str, uid: u32, content: &str) -> String {
    hex::encode(Sha256::digest(
        format!("nabiman-crontab-v1\0{user}\0{uid}\0{content}").as_bytes(),
    ))
}
fn env(line: &str) -> bool {
    line.split_once('=').is_some_and(|(key, _)| {
        let key = key.trim();
        !key.is_empty()
            && key
                .bytes()
                .enumerate()
                .all(|(i, c)| c == b'_' || c.is_ascii_alphabetic() || (i > 0 && c.is_ascii_digit()))
    })
}
pub(super) fn parse(user: &str, uid: u32, content: &str) -> Result<Vec<CronJob>, String> {
    let revision = revision(user, uid, content);
    let mut jobs = vec![];
    for (index, line) in content.split_inclusive('\n').enumerate() {
        let line = line.strip_suffix('\n').unwrap_or(line).trim_start();
        if line.is_empty() || line.starts_with('#') || env(line) {
            continue;
        }
        let count = if line.starts_with('@') { 1 } else { 5 };
        let mut remaining = line;
        let mut fields = vec![];
        for _ in 0..count {
            let end = remaining
                .find(char::is_whitespace)
                .ok_or("Unrecognized crontab entry; refusing to infer a safe editable list")?;
            fields.push(&remaining[..end]);
            remaining = remaining[end..].trim_start();
        }
        if remaining.is_empty() {
            return Err("Crontab entry has no command".into());
        }
        jobs.push(CronJob {
            can_manage: false,
            id: (index + 1) as u32,
            user: user.into(),
            schedule: fields.join(" "),
            command: remaining.into(),
            revision: revision.clone(),
        });
    }
    Ok(jobs)
}
pub(super) fn append(content: &str, schedule: &str, command: &str) -> Result<String, String> {
    if schedule.chars().any(|c| c.is_control())
        || command.trim().is_empty()
        || command.len() > 8192
        || command.chars().any(char::is_control)
    {
        return Err(
            "Use a single-line schedule and a nonempty command without control characters".into(),
        );
    }
    let fields: Vec<_> = schedule.split_whitespace().collect();
    let special = fields.len() == 1
        && [
            "@reboot",
            "@yearly",
            "@annually",
            "@monthly",
            "@weekly",
            "@daily",
            "@midnight",
            "@hourly",
        ]
        .contains(&fields[0]);
    if !special
        && (fields.len() != 5
            || fields.iter().any(|p| {
                !p.bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"*/-,".contains(&c))
            }))
    {
        return Err("Use five cron time fields or a supported @schedule; installed crontab validates the schedule".into());
    }
    let mut next = content.to_string();
    if !next.is_empty() && !next.ends_with('\n') {
        next.push('\n');
    }
    next.push_str(&format!("{} {}\n", fields.join(" "), command));
    if next.len() > 1024 * 1024 {
        return Err("Crontab exceeds 1 MiB".into());
    }
    Ok(next)
}
pub(super) fn remove(user: &str, uid: u32, content: &str, id: u32) -> Result<String, String> {
    if id == 0 || !parse(user, uid, content)?.iter().any(|r| r.id == id) {
        return Err("Selected job no longer exists; reload".into());
    }
    Ok(content
        .split_inclusive('\n')
        .enumerate()
        .filter_map(|(i, s)| if i + 1 == id as usize { None } else { Some(s) })
        .collect())
}

pub(super) fn update(
    user: &str,
    uid: u32,
    content: &str,
    id: u32,
    schedule: &str,
    command: &str,
) -> Result<String, String> {
    if id == 0 || !parse(user, uid, content)?.iter().any(|r| r.id == id) {
        return Err("Selected job no longer exists; reload".into());
    }
    let replacement = append("", schedule, command)?;
    let next: String = content
        .split_inclusive('\n')
        .enumerate()
        .map(|(i, line)| {
            if i + 1 == id as usize {
                replacement.as_str()
            } else {
                line
            }
        })
        .collect();
    if next.len() > 1024 * 1024 {
        return Err("Crontab exceeds 1 MiB".into());
    }
    Ok(next)
}
