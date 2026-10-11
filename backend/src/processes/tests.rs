use super::{identity, signal};
use std::{
    process::{Child, Command, Stdio},
    time::Duration,
};
struct Fixture(Child);
impl Fixture {
    fn new() -> Self {
        let mut f = Self(
            Command::new("/bin/sleep")
                .arg("60")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        );
        for _ in 0..100 {
            if identity::inspect(f.0.id()).is_ok_and(|i| i.command == "/bin/sleep 60") {
                return f;
            }
            assert!(f.0.try_wait().unwrap().is_none());
            std::thread::sleep(Duration::from_millis(5));
        }
        panic!("Fixture did not finish exec")
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
#[test]
fn stat_parser_handles_spaces_parentheses_and_rejects_incomplete_fields() {
    let mut rest = vec!["0"; 20];
    rest[0] = "S";
    rest[1] = "42";
    rest[19] = "12345";
    let value = format!("123 (space ) inside) {}", rest.join(" "));
    let (p, s, n) = identity::stat(&value).unwrap();
    assert_eq!((p, s, n), (42, 12345, "space ) inside".into()));
    assert!(identity::stat("123 (short) S 1").is_err());
}
#[test]
fn wrong_identity_revocation_invalid_signal_and_protected_pids_never_signal() {
    let mut f = Fixture::new();
    let pid = f.0.id();
    let id = identity::inspect(pid).unwrap().value;
    assert!(signal::send(pid, &"0".repeat(64), "KILL", || true).is_err());
    assert!(signal::send(pid, &id, "KILL", || false).is_err());
    assert!(signal::send(pid, &id, "-9", || true).is_err());
    for target in [0, 1, std::process::id()] {
        assert!(signal::send(target, &id, "KILL", || true).is_err());
    }
    assert!(f.0.try_wait().unwrap().is_none());
}
#[test]
fn pidfd_signal_observes_exit_of_owned_fixture_only() {
    let mut f = Fixture::new();
    let pid = f.0.id();
    let id = identity::inspect(pid).unwrap().value;
    let result = signal::send(pid, &id, "TERM", || true).unwrap();
    assert!(result.accepted);
    assert_eq!(result.observation, "exited");
    assert!(f.0.wait().unwrap().code().is_none());
    assert!(signal::send(pid, &id, "KILL", || true).is_err());
}
#[test]
fn ignored_term_is_not_reported_as_exited() {
    let mut child=Command::new("/usr/bin/python3").args(["-c","import signal,time,sys;signal.signal(signal.SIGTERM,signal.SIG_IGN);print('ready',flush=True);time.sleep(60)"]).stdout(Stdio::piped()).spawn().unwrap();
    let mut line = String::new();
    use std::io::BufRead;
    std::io::BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    assert_eq!(line, "ready\n");
    let mut f = Fixture(child);
    let pid = f.0.id();
    let id = identity::inspect(pid).unwrap().value;
    let result = signal::send(pid, &id, "TERM", || true).unwrap();
    assert_eq!(result.observation, "still_present");
    assert!(f.0.try_wait().unwrap().is_none());
}
#[test]
fn stop_and_continue_are_accepted_without_claiming_exit() {
    let mut f = Fixture::new();
    std::thread::sleep(Duration::from_millis(20));
    let pid = f.0.id();
    let id = identity::inspect(pid).unwrap().value;
    assert_eq!(
        signal::send(pid, &id, "STOP", || true).unwrap().observation,
        "still_present"
    );
    assert_eq!(
        signal::send(pid, &id, "CONT", || true).unwrap().observation,
        "still_present"
    );
    assert!(f.0.try_wait().unwrap().is_none());
}
