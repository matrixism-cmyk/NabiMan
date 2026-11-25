use super::*;
use crate::terminal::session::TmuxSession;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

fn users() -> crate::users::UserStore {
    let user = |id: &str, role: &str| -> crate::models::NabimanUser {
        serde_json::from_value(serde_json::json!({"id":id,"username":id,"password_hash":"x","role":role,"created_at":"now"})).unwrap()
    };
    Arc::new(Mutex::new(vec![user("admin-id", "admin"), user("op-id", "admin"), user("member-id", "remote")]))
}

fn session(name: &str, owner_id: &str, shared: bool) -> TmuxSession {
    serde_json::from_value(serde_json::json!({"tmux_name":name,"owner":owner_id,"owner_id":owner_id,"label":"local","shared":shared})).unwrap()
}

fn registry(sessions: Vec<(&str, TmuxSession)>) -> PtyStore {
    Arc::new(Mutex::new(sessions.into_iter().map(|(id, s)| (id.to_string(), s)).collect::<HashMap<_, _>>()))
}

#[test]
fn only_a_session_the_account_sees_is_located() {
    let users = users();
    let store = registry(vec![
        ("pty_mine", session("nb_mine", "admin-id", false)),
        ("pty_theirs", session("nb_theirs", "op-id", false)),
        ("pty_shared", session("nb_shared", "op-id", true)),
        ("pty_member", session("nb_member", "member-id", false)),
    ]);
    assert_eq!(locate(&store, "pty_mine", "admin-id", &users), Some(Target::Local("nb_mine".into())));
    assert_eq!(locate(&store, "pty_theirs", "admin-id", &users), None, "another account's session");
    // A session its owner shared shows the other account its screen and its
    // Claude sessions' folders already (`…/scrollback`, `…/claude`).
    assert_eq!(locate(&store, "pty_shared", "admin-id", &users), Some(Target::Local("nb_shared".into())));
    assert_eq!(locate(&store, "pty_shared", "member-id", &users), None, "a remote-only member sees no shell of this machine");
    assert_eq!(locate(&store, "pty_member", "member-id", &users), None, "not even their own from before");
    assert_eq!(locate(&store, "pty_none", "admin-id", &users), None);
    assert_eq!(locate(&store, "pty_mine", "", &users), None, "no verified account");
    assert_eq!(locate(&store, "pty_mine", "gone-id", &users), None);
}

#[test]
fn a_session_to_another_machine_has_no_folder_and_a_legacy_one_is_not_found() {
    let users = users();
    let mut telnet = session("nb_telnet", "admin-id", false);
    telnet.label = "telnet:admin@192.0.2.1".into();
    telnet.ssh_cmd = Some("telnet 192.0.2.1".into());
    telnet.remote_binding = Some(crate::terminal::remote_access::RemoteBinding::adhoc_telnet("192.0.2.1", 23, "admin"));
    // Remote by an old label or command alone: never trusted, as elsewhere.
    let mut legacy = session("nb_legacy", "admin-id", false);
    legacy.ssh_cmd = Some("ssh old".into());
    let store = registry(vec![("pty_telnet", telnet), ("pty_legacy", legacy)]);
    assert_eq!(locate(&store, "pty_telnet", "admin-id", &users), Some(Target::Remote));
    assert_eq!(locate(&store, "pty_legacy", "admin-id", &users), None);
}

#[actix_web::test]
async fn unknown_and_unseen_sessions_are_404_and_a_remote_one_answers_null() {
    let missing = answer(None).await;
    assert_eq!(missing.status(), actix_web::http::StatusCode::NOT_FOUND);
    let remote = answer(Some(Target::Remote)).await;
    assert_eq!(remote.status(), actix_web::http::StatusCode::OK);
    let body = actix_web::body::to_bytes(remote.into_body()).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["success"], true);
    assert!(json["data"]["cwd"].is_null(), "{json}");
}

#[test]
fn only_absolute_clean_paths_are_handed_out() {
    assert_eq!(clean("/srv/app\n").as_deref(), Some("/srv/app"));
    assert_eq!(clean("/").as_deref(), Some("/"));
    assert_eq!(clean("/한글 폴더/a b").as_deref(), Some("/한글 폴더/a b"));
    for bad in ["", "\n", "relative/path", "~", "/a\tb", "/a\x1b[31m", "/tmp/nabiman-no-such-folder (deleted)"] {
        assert_eq!(clean(bad), None, "{bad:?}");
    }
    assert_eq!(clean(&format!("/{}", "a".repeat(MAX_PATH))), None);
}

fn proc(pid: i64, ppid: i64, pgrp: i64, sid: i64, tty: i64, tpgid: i64) -> Proc {
    Proc { pid, ppid, pgrp, sid, tty, tpgid }
}

#[test]
fn a_stat_line_is_read_past_an_odd_command_name() {
    let stat = "77 (my (odd) cmd) S 70 77 60 34816 91 4194560 1 2 3";
    assert_eq!(parse_stat(77, stat), Some(proc(77, 70, 77, 60, 34816, 91)));
    assert_eq!(parse_stat(1, "1 (short) S 0"), None);
    assert_eq!(parse_stat(1, "no parenthesis"), None);
}

#[test]
fn the_foreground_is_followed_into_a_terminal_inside_the_pane() {
    // The pane's shell (10) on tty 1, with `vim` (20) in the foreground.
    let plain = [proc(10, 1, 10, 10, 1, 20), proc(20, 10, 20, 10, 1, 20)];
    assert_eq!(foreground(10, &plain), Some((20, 0)));
    // bash 10 → su 20 (foreground of tty 1) → its child 30 leads tty 2, where
    // bash 30 started claude 40, now in the foreground.
    let nested = [
        proc(10, 1, 10, 10, 1, 20),
        proc(20, 10, 20, 10, 1, 20),
        proc(30, 20, 30, 30, 2, 40),
        proc(40, 30, 40, 30, 2, 40),
        // Somebody else's session leader on a third terminal is not followed.
        proc(50, 1, 50, 50, 3, 50),
    ];
    assert_eq!(foreground(10, &nested), Some((40, 1)));
    // Two terminals opened by the foreground program: which one is not known.
    let mut two = nested.to_vec();
    two.push(proc(60, 20, 60, 60, 4, 60));
    assert_eq!(foreground(10, &two), Some((20, 0)));
    // A background job's inner terminal is left alone.
    let background = [proc(10, 1, 10, 10, 1, 10), proc(20, 10, 20, 10, 1, 10), proc(30, 20, 30, 30, 2, 30)];
    assert_eq!(foreground(10, &background), Some((10, 0)));
    assert_eq!(foreground(99, &nested), None, "the pane's process is gone");
    assert_eq!(foreground(10, &[proc(10, 1, 10, 10, 1, -1)]), None, "no foreground group");
    // A chain longer than is followed stops instead of looping.
    let chain: Vec<Proc> = (0..10).flat_map(|i| [proc(100 + i, 99 + i, 100 + i, 100 + i, 1 + i, 100 + i + 1)]).collect();
    assert!(foreground(100, &chain).is_some_and(|(_, depth)| depth <= MAX_NESTED));
}

/// A saved server whose host key an administrator approved.
fn approved_ssh_server() -> crate::models::RemoteServer {
    use russh::keys::PublicKeyBase64;
    let key = russh::keys::PrivateKey::random(&mut rand10::rng(), russh::keys::Algorithm::Ed25519).unwrap();
    let public = key.public_key();
    let (public_key, fingerprint) =
        crate::remote_host_trust::parse_key(&format!("{} {}", public.algorithm().as_str(), public.public_key_base64())).unwrap();
    serde_json::from_value(serde_json::json!({"id":"remote","name":"remote","host":"localhost","port":22,"user":"root","auth_method":"key","tags":[],"memo":"","created_at":"now","host_trust_version":uuid::Uuid::new_v4().to_string(),"host_trust":{"public_key":public_key,"fingerprint":fingerprint,"verified_by":"admin","verified_at":"now","reason":"console"}})).unwrap()
}

/// Runs one test of this file again in a child process with `envs` set, and
/// checks it really ran: a name that matches nothing would pass quietly.
fn run_isolated(test: &str, envs: &[(&str, &std::ffi::OsStr)]) {
    let module = module_path!().split_once("::").map_or(module_path!(), |(_, rest)| rest);
    let out = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", &format!("{module}::{test}"), "--nocapture", "--test-threads=1"])
        .envs(envs.iter().copied())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success() && stdout.contains("1 passed"), "{stdout}\n{}", String::from_utf8_lossy(&out.stderr));
}

/// Waits for `want` to be the folder of tmux session `name`: its program takes a moment to start.
fn settles(name: &str, want: &str) -> Option<String> {
    let mut got = None;
    for _ in 0..100 {
        got = pane_cwd(name);
        if got.as_deref() == Some(want) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    got
}

#[test]
fn a_local_session_reports_the_folder_its_shell_is_in() {
    const CHILD: &str = "NABIMAN_TEST_CWD_CHILD";
    if let Some(dir) = std::env::var_os(CHILD) {
        let dir = std::path::PathBuf::from(dir);
        let (start, sub) = (dir.join("start"), dir.join("start/sub dir"));
        std::fs::create_dir_all(&sub).unwrap();
        let (start, sub) = (start.to_str().unwrap(), sub.to_str().unwrap());
        let new = |name: &str, command: &str| {
            assert!(tmux().args(["new-session", "-d", "-s", name, "-x", "80", "-y", "24", "-c", start, command]).output().unwrap().status.success());
        };
        // A shell that changed folder: where it is now, not where it started.
        new("nb_cwd", "cd 'sub dir' && exec sleep 60");
        assert_eq!(settles("nb_cwd", sub).as_deref(), Some(sub));
        // A similarly named session is not answered for another.
        assert_eq!(pane_cwd("nb_cw"), None);
        assert_eq!(pane_cwd("nb_missing"), None);
        // Through the endpoint's own steps, for the account that owns it.
        let store = registry(vec![("pty_cwd", session("nb_cwd", "admin-id", false))]);
        let target = locate(&store, "pty_cwd", "admin-id", &users());
        assert_eq!(target, Some(Target::Local("nb_cwd".into())));
        let body = actix_web::rt::System::new().block_on(async { actix_web::body::to_bytes(answer(target).await.into_body()).await.unwrap() });
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["data"]["cwd"], sub, "{json}");
        // An SSH session answers null even when its pane has a folder here:
        // that folder is the ssh client's, on this machine.
        let server = approved_ssh_server();
        crate::remote_servers::save_servers(&mut [server.clone()]).unwrap();
        let mut ssh = session("nb_cwd", "admin-id", false);
        ssh.label = "ssh:root@localhost".into();
        ssh.ssh_cmd = Some("ssh root@localhost".into());
        ssh.remote_binding = Some(crate::terminal::remote_access::RemoteBinding::from_server(&server).unwrap());
        let store = registry(vec![("pty_ssh", ssh)]);
        let target = locate(&store, "pty_ssh", "admin-id", &users());
        assert_eq!(target, Some(Target::Remote));
        let body = actix_web::rt::System::new().block_on(async { actix_web::body::to_bytes(answer(target).await.into_body()).await.unwrap() });
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json["success"] == true && json["data"]["cwd"].is_null(), "{json}");
        // A terminal inside the pane (as `su --pty` makes): the shell in it counts.
        if std::path::Path::new("/usr/bin/script").exists() {
            new("nb_inner", "script -qfc \"cd 'sub dir' && exec sleep 60\" /dev/null");
            assert_eq!(settles("nb_inner", sub).as_deref(), Some(sub));
        }
        return;
    }
    // A private tmux server: never the running one. A short folder name, for the socket's path.
    let dir = std::env::temp_dir().join(format!("nb-cwd-{}", &uuid::Uuid::new_v4().simple().to_string()[..8]));
    std::fs::create_dir_all(&dir).unwrap();
    let dir = dir.canonicalize().unwrap();
    let result = std::panic::catch_unwind(|| {
        run_isolated(
            "a_local_session_reports_the_folder_its_shell_is_in",
            &[(CHILD, dir.as_os_str()), ("TMUX_TMPDIR", dir.as_os_str()), ("NABIMAN_DATA_DIR", dir.as_os_str())],
        )
    });
    let _ = tmux().env("TMUX_TMPDIR", &dir).args(["kill-server"]).output();
    std::fs::remove_dir_all(dir).unwrap();
    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}
