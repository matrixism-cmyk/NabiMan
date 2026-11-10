use super::*;

fn start(user: &str, dir: &str, skip: bool, resume: bool) -> Start {
    Start { user: user.into(), dir: dir.into(), skip_permissions: skip, resume, title: String::new(), prompt: None, root_shell_after: true, attach: None }
}

#[test]
fn a_background_session_opens_with_claude_attach_as_its_owner() {
    let st = Start { attach: Some("cd2cae1c".into()), skip_permissions: true, resume: true, ..start("nabi", "/workspace/a b", false, false) };
    assert_eq!(
        command(&st),
        r"su --pty -l nabi -c 'cd '\''/workspace/a b'\'' && claude attach '\''cd2cae1c'\''; exec bash -l'; exec bash -l"
    );
    // Only a live background session's card opens one; anything else is refused.
    assert!(from_query("claude_attach=lc%3Ano-such-session").is_err());
    assert!(from_query("claude_attach=pty_123").is_err());
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
    assert_eq!(got, Start { user: "root".into(), dir: "/tmp".into(), skip_permissions: true, resume: false, title: "나비".into(), prompt: None, root_shell_after: true, attach: None });
}
