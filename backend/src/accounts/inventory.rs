use crate::models::UserAccount;
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::Read,
    os::unix::fs::OpenOptionsExt,
    path::Path,
};
pub(crate) fn read(path: &Path, limit: u64) -> Result<String, String> {
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|_| "Local account configuration is unreadable")?;
    if !file
        .metadata()
        .map_err(|_| "Cannot inspect local account configuration")?
        .is_file()
    {
        return Err("Local account configuration is not a regular file".into());
    }
    let mut bytes = vec![];
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Cannot read local account configuration")?;
    if bytes.len() as u64 > limit {
        return Err("Local account configuration exceeds its read limit".into());
    }
    String::from_utf8(bytes).map_err(|_| "Local account configuration is not valid UTF-8".into())
}
pub(crate) fn name(value: &str) -> bool {
    (1..=32).contains(&value.len())
        && value
            .bytes()
            .next()
            .is_some_and(|c| c.is_ascii_lowercase() || c == b'_')
        && value
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_' || c == b'-')
}
pub(crate) fn password(value: &str) -> bool {
    (12..=256).contains(&value.len()) && !value.chars().any(char::is_control)
}
pub(crate) fn parse(content: &str) -> Result<Vec<UserAccount>, String> {
    let mut rows = vec![];
    let mut names = HashSet::new();
    for line in content
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
    {
        let p: Vec<_> = line.split(':').collect();
        if p.len() != 7 || p[0].is_empty() || !names.insert(p[0]) {
            return Err("Malformed or duplicate local account records; inspect /etc/passwd".into());
        }
        let uid = p[2].parse().map_err(|_| "Invalid local account UID")?;
        let gid = p[3].parse().map_err(|_| "Invalid local account GID")?;
        rows.push(UserAccount {
            username: p[0].into(),
            uid,
            gid,
            home: p[5].into(),
            shell: p[6].into(),
            is_logged_in: None,
            can_manage: false,
        });
    }
    if rows.is_empty() {
        return Err("Local account inventory is empty; refusing to infer a valid system".into());
    }
    let mut counts = HashMap::new();
    for row in &rows {
        *counts.entry(row.uid).or_insert(0usize) += 1;
    }
    for i in 0..rows.len() {
        rows[i].can_manage = name(&rows[i].username)
            && (1000..65534).contains(&rows[i].uid)
            && counts.get(&rows[i].uid) == Some(&1);
    }
    Ok(rows)
}
pub(crate) fn shells(path: &Path) -> Result<Vec<String>, String> {
    let content = read(path, 65536)?;
    let mut values = vec![];
    for line in content.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if !line.starts_with('/')
            || line.chars().any(|c| c.is_control() || c.is_whitespace())
            || line.len() > 512
        {
            return Err("Invalid /etc/shells entry".into());
        }
        if !values.iter().any(|v| v == line) {
            values.push(line.into());
        }
    }
    if values.is_empty() {
        return Err("No approved login shells are configured".into());
    }
    Ok(values)
}

/// Require the operating system's NSS identity to agree with the local-file target.
/// Password policy and site hooks remain the operator's PAM/shadow configuration.
pub(crate) fn nss_matches(row: &UserAccount) -> bool {
    let Ok(name) = std::ffi::CString::new(row.username.as_bytes()) else {
        return false;
    };
    let mut buffer = vec![0u8; 65536];
    let mut record = std::mem::MaybeUninit::<libc::passwd>::uninit();
    let mut found = std::ptr::null_mut();
    let result = unsafe {
        libc::getpwnam_r(
            name.as_ptr(),
            record.as_mut_ptr(),
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            &mut found,
        )
    };
    if result != 0 || found.is_null() {
        return false;
    }
    let record = unsafe { record.assume_init() };
    if record.pw_name.is_null() || record.pw_dir.is_null() || record.pw_shell.is_null() {
        return false;
    }
    record.pw_uid == row.uid
        && record.pw_gid == row.gid
        && unsafe { std::ffi::CStr::from_ptr(record.pw_name) }.to_bytes() == row.username.as_bytes()
        && unsafe { std::ffi::CStr::from_ptr(record.pw_dir) }.to_bytes() == row.home.as_bytes()
        && unsafe { std::ffi::CStr::from_ptr(record.pw_shell) }.to_bytes() == row.shell.as_bytes()
}
