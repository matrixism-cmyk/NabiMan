//! Remote paths and listings, kept free of I/O so they can be tested directly.

use russh_sftp::client::fs::Metadata;
use serde::Serialize;

/// Longest remote path accepted from the browser.
const MAX_PATH: usize = 4096;

/// A path the browser sent: something, without NUL bytes, of a sane length.
/// Where it points is the remote server's business; its own permissions decide.
pub(crate) fn valid_path(path: &str) -> bool {
    !path.is_empty() && path.len() <= MAX_PATH && !path.contains('\0')
}

/// A single name to create inside a folder: no separators, not `.` or `..`.
pub(crate) fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && name != "."
        && name != ".."
        && !name.contains('/')
        && !name.contains('\0')
}

pub(crate) fn join(dir: &str, name: &str) -> String {
    if dir.ends_with('/') {
        format!("{dir}{name}")
    } else {
        format!("{dir}/{name}")
    }
}

/// The folder above `path`, or `None` at the root.
pub(crate) fn parent(path: &str) -> Option<String> {
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        return None;
    }
    match trimmed.rfind('/') {
        Some(0) => Some("/".into()),
        Some(i) => Some(trimmed[..i].to_string()),
        None => None,
    }
}

/// The last part of a path, used as the downloaded file's name.
pub(crate) fn base_name(path: &str) -> &str {
    path.trim_end_matches('/')
        .rsplit('/')
        .next()
        .filter(|n| !n.is_empty())
        .unwrap_or("download")
}

/// `path` as a URL query value.
pub(crate) fn query_escape(path: &str) -> String {
    let mut out = String::new();
    for b in path.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~/".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// `Content-Disposition` for a download, with the name encoded per RFC 5987
/// so Korean and other non-ASCII names survive, and an ASCII fallback.
pub(crate) fn disposition(name: &str) -> String {
    let fallback: String = name
        .chars()
        .map(|c| if c.is_ascii_graphic() && c != '"' && c != '\\' { c } else { '_' })
        .collect();
    let mut encoded = String::new();
    for b in name.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
            encoded.push(b as char);
        } else {
            encoded.push_str(&format!("%{b:02X}"));
        }
    }
    format!("attachment; filename=\"{fallback}\"; filename*=UTF-8''{encoded}")
}

#[derive(Serialize, Debug, PartialEq)]
pub(crate) struct Entry {
    pub name: String,
    /// `dir`, `file`, `link` or `other`.
    pub kind: &'static str,
    /// For a link: whether it leads to a folder, so the browser can open it.
    pub to_dir: bool,
    pub size: u64,
    pub mtime: Option<u32>,
    pub mode: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gid: Option<u32>,
    /// Owner and group names, where they can be told.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

pub(crate) fn kind_of(meta: &Metadata) -> &'static str {
    if meta.is_symlink() {
        "link"
    } else if meta.is_dir() {
        "dir"
    } else if meta.is_regular() {
        "file"
    } else {
        "other"
    }
}

pub(crate) fn entry(name: String, meta: &super::fs::Meta) -> Entry {
    Entry {
        name,
        kind: meta.kind,
        to_dir: false,
        size: meta.size,
        mtime: meta.mtime,
        mode: meta.mode,
        uid: meta.uid,
        gid: meta.gid,
        owner: meta.owner.clone(),
        group: meta.group.clone(),
    }
}

/// Folders (and links to folders) first, then everything else, by name.
pub(crate) fn sort(entries: &mut [Entry]) {
    entries.sort_by(|a, b| {
        let folder = |e: &Entry| e.kind == "dir" || e.to_dir;
        folder(b)
            .cmp(&folder(a))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then_with(|| a.name.cmp(&b.name))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_cannot_leave_their_folder() {
        assert!(valid_name("report.txt"));
        assert!(valid_name("보고서 최종.pdf"));
        for bad in ["", ".", "..", "a/b", "../x", "a\0b"] {
            assert!(!valid_name(bad), "{bad:?}");
        }
        assert!(!valid_name(&"x".repeat(256)));
    }

    #[test]
    fn paths_are_bounded() {
        assert!(valid_path("/home/me"));
        assert!(!valid_path(""));
        assert!(!valid_path("/a\0b"));
        assert!(!valid_path(&"/x".repeat(3000)));
    }

    #[test]
    fn joins_and_walks_up() {
        assert_eq!(join("/home/me", "a"), "/home/me/a");
        assert_eq!(join("/", "etc"), "/etc");
        assert_eq!(parent("/home/me"), Some("/home".into()));
        assert_eq!(parent("/home/me/"), Some("/home".into()));
        assert_eq!(parent("/home"), Some("/".into()));
        assert_eq!(parent("/"), None);
        assert_eq!(base_name("/var/log/syslog"), "syslog");
        assert_eq!(base_name("/"), "download");
    }

    #[test]
    fn link_paths_round_trip_through_a_query() {
        let path = "/srv/보고서 & 결과?.txt";
        let query = format!("path={}&ticket=x", query_escape(path));
        let parsed = actix_web::web::Query::<std::collections::HashMap<String, String>>::from_query(&query).unwrap();
        assert_eq!(parsed.get("path").map(String::as_str), Some(path));
    }

    #[test]
    fn download_names_survive_non_ascii() {
        let header = disposition("보고서 \"v2\".txt");
        assert!(header.starts_with("attachment; filename=\""));
        assert!(header.contains("filename*=UTF-8''%EB%B3%B4"));
        assert!(header.contains("%22v2%22.txt"));
        // The quoted fallback cannot be broken out of.
        let fallback = header.split('"').nth(1).unwrap();
        assert!(!fallback.contains('\\'));
    }

    #[test]
    fn folders_come_first() {
        let e = |name: &str, kind: &'static str, to_dir: bool| Entry {
            name: name.into(), kind, to_dir, size: 0, mtime: None, mode: None, uid: None, gid: None, owner: None, group: None,
        };
        let mut list = vec![e("b.txt", "file", false), e("Zeta", "dir", false),
            e("alpha", "dir", false), e("link", "link", true), e("A.txt", "file", false)];
        sort(&mut list);
        let names: Vec<_> = list.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["alpha", "link", "Zeta", "A.txt", "b.txt"]);
    }
}
