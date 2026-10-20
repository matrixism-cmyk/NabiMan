//! Reading what an FTP server says a folder holds. MLSD (RFC 3659) is
//! machine-readable; servers without it answer LIST in the style of `ls -l`
//! (Unix) or of `dir` (Windows/IIS), and both are understood here.

use super::super::fs::Meta;

/// One parsed line: its name and what the panel needs to know about it.
pub(crate) type Item = (String, Meta);

/// MLSD/MLST facts: `type=file;size=12;modify=20240102030405;UNIX.mode=0644; name`.
pub(crate) fn mlsd_line(line: &str) -> Option<Item> {
    let line = line.trim_end_matches(['\r', '\n']);
    let (facts, name) = line.split_once(' ')?;
    let meta = facts_meta(facts)?;
    // The folder itself and its parent are listed as cdir and pdir.
    if meta.kind.is_empty() || name.is_empty() {
        return None;
    }
    Some((name.to_string(), meta))
}

/// The facts of one MLST/MLSD entry; `kind` is empty for cdir and pdir.
pub(crate) fn facts_meta(facts: &str) -> Option<Meta> {
    let mut meta = Meta { kind: "other", size: 0, mtime: None, mode: None };
    let mut typed = false;
    for fact in facts.split(';').filter(|f| !f.is_empty()) {
        let (key, value) = fact.split_once('=')?;
        match key.to_ascii_lowercase().as_str() {
            "type" => {
                typed = true;
                meta.kind = match value.to_ascii_lowercase().as_str() {
                    "file" => "file",
                    "dir" => "dir",
                    "cdir" | "pdir" => "",
                    v if v.starts_with("os.unix=slink") || v.starts_with("os.unix=symlink") => "link",
                    _ => "other",
                };
            }
            "size" | "sizd" => meta.size = value.parse().unwrap_or(0),
            "modify" => meta.mtime = timestamp(value),
            "unix.mode" => meta.mode = u32::from_str_radix(value.trim_start_matches('0'), 8).ok().or(Some(0)),
            _ => {}
        }
    }
    typed.then_some(meta)
}

/// `YYYYMMDDHHMMSS[.sss]` in UTC, as MLSD and MDTM give it.
pub(crate) fn timestamp(value: &str) -> Option<u32> {
    let digits = value.split('.').next()?;
    let t = chrono::NaiveDateTime::parse_from_str(digits, "%Y%m%d%H%M%S").ok()?;
    u32::try_from(t.and_utc().timestamp()).ok()
}

/// One line of a LIST answer, Unix or Windows style. `total 12` and the
/// folder's own `.` and `..` give nothing.
pub(crate) fn list_line(line: &str, now: chrono::NaiveDate) -> Option<Item> {
    let line = line.trim_end_matches(['\r', '\n']);
    let item = unix_line(line, now).or_else(|| dos_line(line))?;
    (item.0 != "." && item.0 != "..").then_some(item)
}

/// The words of a line with where each one starts, so the name — the rest
/// of the line — keeps its spaces.
fn words(line: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut start = None;
    for (i, c) in line.char_indices() {
        match (c == ' ' || c == '\t', start) {
            (true, Some(s)) => {
                out.push((s, &line[s..i]));
                start = None;
            }
            (false, None) => start = Some(i),
            _ => {}
        }
    }
    if let Some(s) = start {
        out.push((s, &line[s..]));
    }
    out
}

const MONTHS: [&str; 12] = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];

/// `drwxr-xr-x  2 user group 4096 Oct 10 10:10 name`, or with a year in
/// place of the time, and with or without the group.
fn unix_line(line: &str, now: chrono::NaiveDate) -> Option<Item> {
    let w = words(line);
    let perms = w.first()?.1;
    if perms.len() < 10 || !"-dlbcps".contains(perms.chars().next()?) {
        return None;
    }
    // The date is "Mon DD HH:MM|YYYY"; the size comes right before it.
    let at = (2..w.len().saturating_sub(3)).find(|&i| {
        MONTHS.contains(&w[i].1.to_ascii_lowercase().as_str()) && w[i + 1].1.parse::<u32>().is_ok()
    })?;
    let size: u64 = w[at - 1].1.parse().ok()?;
    let name_at = w.get(at + 3)?.0;
    let mut name = &line[name_at..];
    let kind = match perms.as_bytes()[0] {
        b'd' => "dir",
        b'-' => "file",
        b'l' => {
            name = name.split(" -> ").next().unwrap_or(name);
            "link"
        }
        _ => "other",
    };
    let month = MONTHS.iter().position(|m| *m == w[at].1.to_ascii_lowercase())? as u32 + 1;
    let day: u32 = w[at + 1].1.parse().ok()?;
    let when = w[at + 2].1;
    let mtime = if let Some((h, m)) = when.split_once(':') {
        // No year means within the last six months or so.
        let (h, m): (u32, u32) = (h.parse().ok()?, m.parse().ok()?);
        let mut date = chrono::NaiveDate::from_ymd_opt(chrono::Datelike::year(&now), month, day)?;
        if date > now + chrono::Duration::days(1) {
            date = chrono::NaiveDate::from_ymd_opt(chrono::Datelike::year(&now) - 1, month, day)?;
        }
        date.and_hms_opt(h, m, 0)
    } else {
        chrono::NaiveDate::from_ymd_opt(when.parse().ok()?, month, day)?.and_hms_opt(0, 0, 0)
    };
    let mtime = mtime.and_then(|t| u32::try_from(t.and_utc().timestamp()).ok());
    Some((name.to_string(), Meta { kind, size, mtime, mode: Some(mode_bits(perms)) }))
}

fn mode_bits(perms: &str) -> u32 {
    let b = perms.as_bytes();
    let mut mode = 0;
    for (i, bit) in (1..10).zip((0..9).rev()) {
        match b.get(i) {
            Some(b'r' | b'w' | b'x') => mode |= 1 << bit,
            Some(b's' | b't') => mode |= 1 << bit,
            _ => {}
        }
    }
    mode
}

/// `10-25-23  02:15PM       <DIR>          Folder` or `… 1234 file.txt`.
fn dos_line(line: &str) -> Option<Item> {
    let w = words(line);
    if w.len() < 4 {
        return None;
    }
    let date = chrono::NaiveDate::parse_from_str(w[0].1, "%m-%d-%y")
        .or_else(|_| chrono::NaiveDate::parse_from_str(w[0].1, "%m-%d-%Y"))
        .ok()?;
    let time = chrono::NaiveTime::parse_from_str(w[1].1, "%I:%M%p")
        .or_else(|_| chrono::NaiveTime::parse_from_str(w[1].1, "%H:%M"))
        .ok()?;
    let mtime = u32::try_from(date.and_time(time).and_utc().timestamp()).ok();
    let name = line[w[3].0..].to_string();
    if w[2].1.eq_ignore_ascii_case("<DIR>") {
        Some((name, Meta { kind: "dir", size: 0, mtime, mode: None }))
    } else {
        let size = w[2].1.replace(',', "").parse().ok()?;
        Some((name, Meta { kind: "file", size, mtime, mode: None }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn today() -> chrono::NaiveDate {
        chrono::NaiveDate::from_ymd_opt(2026, 10, 20).unwrap()
    }

    #[test]
    fn mlsd_lines_give_kind_size_time_and_mode() {
        let (name, m) = mlsd_line("type=file;size=1234;modify=20261019120000;UNIX.mode=0644; my report.pdf").unwrap();
        assert_eq!((name.as_str(), m.kind, m.size, m.mode), ("my report.pdf", "file", 1234, Some(0o644)));
        assert_eq!(m.mtime, Some(1_792_411_200));
        assert_eq!(mlsd_line("type=dir;modify=20261019120000; sub").unwrap().1.kind, "dir");
        assert_eq!(mlsd_line("type=OS.unix=slink:/etc;modify=20261019120000; etc").unwrap().1.kind, "link");
        assert!(mlsd_line("type=cdir;modify=20261019120000; /home/u").is_none());
        assert!(mlsd_line("type=pdir;modify=20261019120000; ..").is_none());
    }

    #[test]
    fn unix_listings_keep_spaces_in_names() {
        let (name, m) = list_line("-rw-r--r--    1 1000     1000         5120 Oct 19 09:30 two  spaces.txt", today()).unwrap();
        assert_eq!((name.as_str(), m.kind, m.size, m.mode), ("two  spaces.txt", "file", 5120, Some(0o644)));
        assert_eq!(m.mtime, Some(1_792_402_200));
        let (name, m) = list_line("drwxr-xr-x 2 ftp ftp 4096 Jan  3  2024 old", today()).unwrap();
        assert_eq!((name.as_str(), m.kind, m.mode), ("old", "dir", Some(0o755)));
        // Without a group, and a link shows its own name.
        let (name, m) = list_line("lrwxrwxrwx 1 owner 7 Oct 01 10:00 latest -> releases/9", today()).unwrap();
        assert_eq!((name.as_str(), m.kind), ("latest", "link"));
        // A date "in the future" is from last year.
        let (_, m) = list_line("-rw-r--r-- 1 u g 1 Dec 24 18:00 gift", today()).unwrap();
        assert_eq!(m.mtime, Some(1_766_599_200));
        assert!(list_line("total 24", today()).is_none());
        assert!(list_line("drwxr-xr-x 2 u g 4096 Oct 19 09:30 ..", today()).is_none());
    }

    #[test]
    fn windows_listings_are_understood() {
        let (name, m) = list_line("10-19-26  02:15PM       <DIR>          Web Site", today()).unwrap();
        assert_eq!((name.as_str(), m.kind), ("Web Site", "dir"));
        let (name, m) = list_line("10-19-26  09:00AM              1,234 index.html", today()).unwrap();
        assert_eq!((name.as_str(), m.kind, m.size), ("index.html", "file", 1234));
        assert_eq!(m.mtime, Some(1_792_400_400));
    }

    #[test]
    fn timestamps_ignore_fractions() {
        assert_eq!(timestamp("20261019120000.123"), Some(1_792_411_200));
        assert_eq!(timestamp("garbage"), None);
    }
}
