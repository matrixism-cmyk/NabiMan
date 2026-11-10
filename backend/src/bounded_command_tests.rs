use super::*;
use std::os::unix::fs::PermissionsExt;

fn sh(script: &str) -> Command {
    let mut c = Command::new("/bin/sh");
    c.args(["-c", script]);
    c
}

fn alive(pid: i32) -> bool {
    // SAFETY: signal 0 only checks that the process exists.
    unsafe { libc::kill(pid, 0) == 0 }
}

#[test]
fn a_hanging_child_and_its_children_are_stopped_at_the_deadline() {
    let dir = std::env::temp_dir().join(format!("nabiman-bounded-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let pidfile = dir.join("grandchild.pid");
    // Like sshpass → ssh: the child starts another process and both hang.
    let script = format!("sleep 600 & echo $! > {}; wait", pidfile.display());
    let started = Instant::now();
    let result = capture(&mut sh(&script), None, Duration::from_millis(300), MAX_OUTPUT);
    assert_eq!(result.err(), Some(RunError::TimedOut(Duration::from_millis(300))));
    assert!(started.elapsed() < Duration::from_secs(5), "took {:?}", started.elapsed());
    let grandchild: i32 = std::fs::read_to_string(&pidfile).unwrap().trim().parse().unwrap();
    let gone = Instant::now();
    while alive(grandchild) && gone.elapsed() < Duration::from_secs(2) {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!alive(grandchild), "the grandchild was stopped too");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_timeout_reads_as_one_to_the_client() {
    let err = sh("sleep 5").output_within(Duration::from_millis(100)).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::TimedOut);
    assert!(err.to_string().starts_with("Timed out after 0s"), "{err}");
}

#[test]
fn output_is_kept_up_to_the_limit_and_the_rest_dropped() {
    let c = capture(&mut sh("head -c 100000 /dev/zero; echo done >&2"), None, Duration::from_secs(10), 1000).unwrap();
    assert!(c.status.success());
    assert_eq!(c.stdout.len(), 1000);
    assert!(c.truncated);
    assert_eq!(c.stderr, b"done\n");
}

#[test]
fn input_reaches_the_child_and_a_background_holder_does_not_stall_the_result() {
    let c = capture(&mut sh("cat; (sleep 600 &)"), Some(b"hello"), Duration::from_secs(20), MAX_OUTPUT).unwrap();
    assert_eq!(c.stdout, b"hello");
    assert!(!c.truncated);
    let started = Instant::now();
    let out = sh("echo hi; sleep 600 >/dev/null &").output_within(Duration::from_secs(20)).unwrap();
    assert_eq!(out.stdout, b"hi\n");
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[test]
fn a_missing_program_is_a_start_error() {
    let dir = std::env::temp_dir().join(format!("nabiman-bounded-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("noexec");
    std::fs::write(&p, "#!/bin/sh\n").unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(capture(&mut Command::new(&p), None, Duration::from_secs(1), 10).err(), Some(RunError::Start));
    let _ = std::fs::remove_dir_all(dir);
}
