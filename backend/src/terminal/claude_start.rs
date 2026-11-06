//! Starting a local session straight into Claude Code: as one of this
//! machine's users, in a chosen directory, with the options picked in the
//! browser. It is what an operator does by hand — a root shell, `su <user>`,
//! `cd`, `claude …` — done for them, and the shells stay behind when Claude
//! Code exits, just as they would.

use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use actix_web::{web, HttpRequest, HttpResponse};
use serde::Serialize;

use crate::models::ApiResponse;

const MAX_DIR_LEN: usize = 1024;
const MAX_TITLE_LEN: usize = 64;
/// Claude Code's own list of projects; larger files are not read.
const MAX_CONFIG_BYTES: u64 = 64 << 20;
const MAX_DIRS: usize = 40;

/// What the browser asked a new session to start with.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Start {
    pub user: String,
    pub dir: String,
    /// `--dangerously-skip-permissions`
    pub skip_permissions: bool,
    /// `-c`: carry on with the last conversation in that directory.
    pub resume: bool,
    pub title: String,
    /// A first message for Claude Code (`claude "<prompt>"`).
    pub prompt: Option<String>,
    /// Drop to root's shell when the user's shell ends, as `su` by hand does.
    /// Sessions opened by a program end with the user's shell instead.
    pub root_shell_after: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct Account {
    pub name: String,
    pub home: String,
}

/// A user who may run a shell here: root, and people (uid 1000 and up) with a
/// real login shell and a home that exists.
fn accounts_in(passwd: &str, home_exists: impl Fn(&str) -> bool) -> Vec<Account> {
    passwd
        .lines()
        .filter_map(|line| {
            let f: Vec<&str> = line.split(':').collect();
            if f.len() < 7 {
                return None;
            }
            let uid: u32 = f[2].parse().ok()?;
            let shell = f[6];
            let person = (1000..65534).contains(&uid);
            if !(uid == 0 || person) || shell.ends_with("nologin") || shell.ends_with("false") || !valid_user(f[0]) {
                return None;
            }
            home_exists(f[5]).then(|| Account { name: f[0].to_string(), home: f[5].to_string() })
        })
        .collect()
}

pub(crate) fn accounts() -> Vec<Account> {
    let passwd = fs::read_to_string("/etc/passwd").unwrap_or_default();
    accounts_in(&passwd, |home| Path::new(home).is_dir())
}

pub(crate) fn valid_user(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_lowercase() || c == '_')
        && name.len() <= 32
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

/// A query value, percent-decoded as UTF-8 (a Korean directory or name comes
/// through whole); `None` when absent or not valid UTF-8.
fn param(query: &str, key: &str) -> Option<String> {
    let raw = query.split('&').find_map(|p| p.split_once('=').filter(|(k, _)| *k == key).map(|(_, v)| v))?;
    let mut bytes = Vec::with_capacity(raw.len());
    let mut it = raw.bytes();
    while let Some(b) = it.next() {
        match b {
            b'%' => {
                let hex = [it.next()?, it.next()?];
                bytes.push(u8::from_str_radix(std::str::from_utf8(&hex).ok()?, 16).ok()?);
            }
            b'+' => bytes.push(b' '),
            _ => bytes.push(b),
        }
    }
    String::from_utf8(bytes).ok()
}

/// The start the query asks for, checked; `None` when it asks for none.
pub(crate) fn from_query(query: &str) -> Result<Option<Start>, String> {
    let Some(user) = param(query, "start_user") else {
        return Ok(None);
    };
    let known = accounts();
    if !valid_user(&user) || !known.iter().any(|a| a.name == user) {
        return Err("이 서버에 없는 사용자입니다 / unknown user".into());
    }
    let dir = param(query, "start_dir").unwrap_or_default();
    if dir.is_empty() || dir.len() > MAX_DIR_LEN || !dir.starts_with('/') || dir.chars().any(char::is_control) {
        return Err("디렉토리는 /로 시작하는 절대 경로여야 합니다 / the directory must be an absolute path".into());
    }
    if !Path::new(&dir).is_dir() {
        return Err(format!("디렉토리가 없습니다 / no such directory: {dir}"));
    }
    let on = |key: &str| param(query, key).is_some_and(|v| v == "1");
    let title = param(query, "start_title")
        .unwrap_or_default()
        .trim()
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_TITLE_LEN)
        .collect();
    Ok(Some(Start { user, dir, skip_permissions: on("claude_skip"), resume: on("claude_continue"), title, prompt: None, root_shell_after: true }))
}

/// One word for a POSIX shell, whatever it holds.
fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// The command tmux runs for the session (as root, like every local shell).
///
/// The user's part runs in a login shell, so `/etc/profile.d` and their own
/// profile set things up as when they sign in (PATH to `claude`, HOME,
/// `GIT_CONFIG_GLOBAL`, cargo…): a plain `bash -i` misses profile.d, and an
/// operator's `su` from root's login shell would have inherited it. When
/// Claude Code exits they are left in their shell; leaving that comes back
/// to root's, the way `su` does by hand (unless a program opened it).
///
/// `--pty`: `su -c` alone starts the command in a new session with no
/// controlling terminal ("no job control"); a pty of its own keeps Ctrl+C,
/// job control, `/dev/tty` and window resizes working for Claude Code.
pub(crate) fn command(start: &Start) -> String {
    let mut claude = String::from("claude");
    if start.skip_permissions {
        claude.push_str(" --dangerously-skip-permissions");
    }
    if start.resume {
        claude.push_str(" -c");
    }
    if let Some(prompt) = start.prompt.as_deref().filter(|p| !p.is_empty()) {
        claude.push_str(" -- ");
        claude.push_str(&quote(prompt));
    }
    let inner = format!("cd {} && {}; exec bash -l", quote(&start.dir), claude);
    let run = if start.user == "root" {
        format!("bash -l -c {}", quote(&inner))
    } else {
        format!("su --pty -l {} -c {}", start.user, quote(&inner))
    };
    if start.root_shell_after {
        format!("{run}; exec bash -l")
    } else {
        run
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct Place {
    pub user: String,
    pub dir: String,
    /// When Claude Code last had a conversation there (ms since 1970), if known.
    pub last_used: Option<u64>,
}

/// Directories Claude Code has been used in, from each account's own project
/// list (`~/.claude.json`), most recent first. Only paths and times: nothing
/// else in that file leaves it.
fn places_in(user: &str, config: &str, is_dir: impl Fn(&str) -> bool, used: impl Fn(&str) -> Option<u64>) -> Vec<Place> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(config) else {
        return Vec::new();
    };
    let Some(projects) = value.get("projects").and_then(|p| p.as_object()) else {
        return Vec::new();
    };
    projects
        .iter()
        .filter(|(dir, _)| dir.starts_with('/') && dir.len() <= MAX_DIR_LEN && !dir.chars().any(char::is_control) && is_dir(dir))
        .map(|(dir, p)| Place {
            user: user.to_string(),
            dir: dir.clone(),
            // Claude Code seldom fills its own time in; its conversations show it.
            last_used: used(dir).or_else(|| p.get("lastSessionModified").and_then(|v| v.as_u64())),
        })
        .collect()
}

fn places(accounts: &[Account]) -> Vec<Place> {
    let mut all: Vec<Place> = accounts
        .iter()
        .flat_map(|a| {
            let path = Path::new(&a.home).join(".claude.json");
            // Only that account's own file, and not an outsized one.
            let fits = fs::metadata(&path).is_ok_and(|m| m.is_file() && m.len() <= MAX_CONFIG_BYTES && owned_by(&m, &a.name));
            let text = if fits { fs::read_to_string(&path).unwrap_or_default() } else { String::new() };
            let projects = Path::new(&a.home).join(".claude").join("projects");
            places_in(&a.name, &text, |d| Path::new(d).is_dir(), |d| last_conversation(&projects.join(super::claude_links::folder_name(d))))
        })
        .collect();
    all.sort_by(|a, b| b.last_used.cmp(&a.last_used).then_with(|| a.dir.cmp(&b.dir)));
    all.truncate(MAX_DIRS);
    all
}

/// When the newest conversation in a project folder last changed (ms since 1970).
fn last_conversation(folder: &Path) -> Option<u64> {
    fs::read_dir(folder)
        .ok()?
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().ends_with(".jsonl"))
        .filter_map(|e| e.metadata().ok()?.modified().ok())
        .max()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
}

fn owned_by(meta: &fs::Metadata, name: &str) -> bool {
    let passwd = fs::read_to_string("/etc/passwd").unwrap_or_default();
    passwd.lines().any(|l| {
        let f: Vec<&str> = l.split(':').collect();
        f.len() > 2 && f[0] == name && f[2].parse::<u32>().is_ok_and(|uid| uid == meta.uid())
    })
}

#[derive(Serialize)]
struct Options {
    users: Vec<Account>,
    /// The account to suggest: the one Claude Code was used by most recently.
    default_user: Option<String>,
    dirs: Vec<Place>,
}

/// Who a session can be started as, and where Claude Code has been used.
/// Local shells only: a remote-only member gets nothing here.
async fn start_options(req: HttpRequest, users: web::Data<crate::users::UserStore>) -> HttpResponse {
    let Some(id) = crate::auth::verified_user_id(&req).filter(|u| !u.is_empty()) else {
        return HttpResponse::Unauthorized().json(ApiResponse::<()>::error("Not signed in"));
    };
    let member = users
        .lock()
        .unwrap()
        .iter()
        .find(|u| u.id == id)
        .is_none_or(crate::member_servers::is_member);
    if member {
        return HttpResponse::Forbidden().json(ApiResponse::<()>::error("Local sessions are not available to this account"));
    }
    let list = accounts();
    let dirs = places(&list);
    let default_user = dirs.first().map(|p| p.user.clone());
    HttpResponse::Ok().json(ApiResponse::ok(Options { users: list, default_user, dirs }))
}

pub(crate) fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/api/terminal/start-options", web::get().to(start_options));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn start(user: &str, dir: &str, skip: bool, resume: bool) -> Start {
        Start { user: user.into(), dir: dir.into(), skip_permissions: skip, resume, title: String::new(), prompt: None, root_shell_after: true }
    }

    #[test]
    fn the_command_is_what_an_operator_types_by_hand() {
        assert_eq!(
            command(&start("nabi", "/workspace/nabiman", true, true)),
            r#"su --pty -l nabi -c 'cd '\''/workspace/nabiman'\'' && claude --dangerously-skip-permissions -c; exec bash -l'; exec bash -l"#
        );
        assert_eq!(command(&start("root", "/srv", false, false)), r#"bash -l -c 'cd '\''/srv'\'' && claude; exec bash -l'; exec bash -l"#);
    }

    /// Runs `command` with stand-ins for su, bash and claude on PATH, and
    /// gives back the arguments claude got and the directory it ran in.
    fn run_with_stand_ins(start: &Start) -> String {
        use std::os::unix::fs::PermissionsExt;
        let bin = std::env::temp_dir().join(format!("nabiman-start-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&bin).unwrap();
        let tool = |name: &str, body: &str| {
            let path = bin.join(name);
            fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        };
        // su --pty -l <user> -c <cmd>; bash -l [-c <cmd>]; claude prints what it got.
        tool("su", r#"[ "$1" = --pty ] && [ "$2" = -l ] && [ "$4" = -c ] && exec sh -c "$5""#);
        tool("bash", r#"[ "$1" = -l ] && [ "$2" = -c ] && exec sh -c "$3"; exit 0"#);
        tool("claude", r#"printf 'dir=%s\n' "$PWD"; for a in "$@"; do printf '[%s]' "$a"; done"#);
        let out = std::process::Command::new("sh")
            .args(["-c", &command(start)])
            .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
            .output()
            .unwrap();
        let _ = fs::remove_dir_all(&bin);
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    #[test]
    fn claude_gets_its_options_and_a_first_message_as_one_argument() {
        let dir = std::env::temp_dir().join(format!("it's a dir {}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let mut s = start("nabi", dir.to_str().unwrap(), true, true);
        s.prompt = Some("it's -rf; $(touch /tmp/never) `x`".into());
        let got = run_with_stand_ins(&s);
        let _ = fs::remove_dir_all(&dir);
        assert!(got.contains(&format!("dir={}", dir.display())), "{got}");
        assert!(got.contains("[--dangerously-skip-permissions][-c][--][it's -rf; $(touch /tmp/never) `x`]"), "{got}");
    }

    #[test]
    fn a_program_s_session_ends_with_the_user_s_shell() {
        let mut s = start("nabi", "/w", true, false);
        s.root_shell_after = false;
        assert_eq!(command(&s), r#"su --pty -l nabi -c 'cd '\''/w'\'' && claude --dangerously-skip-permissions; exec bash -l'"#);
        s.root_shell_after = true;
        assert!(command(&s).ends_with("'; exec bash -l"));
    }

    #[test]
    fn a_quote_in_the_directory_stays_inside_it() {
        // Run the produced words through sh and see the directory come back whole.
        let dir = "/tmp/it's here; rm -rf /";
        let out = std::process::Command::new("sh")
            .args(["-c", &format!("printf %s {}", quote(dir))])
            .output()
            .unwrap();
        assert_eq!(String::from_utf8(out.stdout).unwrap(), dir);
        let inner = format!("printf %s {}", quote(dir));
        let out = std::process::Command::new("sh").args(["-c", &format!("sh -c {}", quote(&inner))]).output().unwrap();
        assert_eq!(String::from_utf8(out.stdout).unwrap(), dir);
    }

    #[test]
    fn only_root_and_people_with_a_shell_and_a_home() {
        let passwd = "root:x:0:0:root:/root:/bin/bash\n\
                      daemon:x:1:1:daemon:/usr/sbin:/usr/sbin/nologin\n\
                      nabi:x:1000:1000::/workspace/nabi:/bin/bash\n\
                      gone:x:1001:1001::/nowhere:/bin/bash\n\
                      svc:x:1002:1002::/srv/svc:/bin/false\n\
                      Bad:x:1003:1003::/home/bad:/bin/bash\n\
                      nobody:x:65534:65534::/nonexistent:/bin/bash";
        let names: Vec<String> = accounts_in(passwd, |h| h != "/nowhere").into_iter().map(|a| a.name).collect();
        assert_eq!(names, vec!["root", "nabi"]);
    }

    #[test]
    fn places_are_paths_and_times_only() {
        let config = r#"{"projects":{
            "/workspace/nabiman":{"lastSessionModified":1793890000000,"lastSessionFirstPrompt":"secret"},
            "/gone":{"lastSessionModified":1},
            "relative":{},
            "/workspace":{}
        }}"#;
        let got = places_in("nabi", config, |d| d != "/gone", |d| (d == "/workspace").then_some(5));
        assert_eq!(got.len(), 2);
        assert!(got.iter().any(|p| p.dir == "/workspace/nabiman" && p.last_used == Some(1793890000000)));
        assert!(got.iter().any(|p| p.dir == "/workspace" && p.last_used == Some(5)), "the conversations' own time wins");
        assert!(!serde_json::to_string(&got).unwrap().contains("secret"));
        assert!(places_in("nabi", "not json", |_| true, |_| None).is_empty());
    }

    #[test]
    fn the_query_picks_the_start() {
        assert_eq!(from_query("cols=80").unwrap(), None);
        assert!(from_query("start_user=Root&start_dir=%2F").is_err());
        assert!(from_query("start_user=root&start_dir=relative").is_err());
        assert!(from_query("start_user=root&start_dir=%2Fno%2Fsuch%2Fdir").is_err());
        assert!(from_query("start_user=root&start_dir=%2Ftmp%ZZ").is_err(), "a broken escape is no directory");
        let got = from_query("start_user=root&start_dir=%2Ftmp&claude_skip=1&claude_continue=0&start_title=%EB%82%98%EB%B9%84").unwrap().unwrap();
        assert_eq!(got, Start { user: "root".into(), dir: "/tmp".into(), skip_permissions: true, resume: false, title: "나비".into(), prompt: None, root_shell_after: true });
    }
}
