use super::tmux_operations::classify_tmux_probe;
use std::os::unix::process::ExitStatusExt;
#[test]
fn lookup_failure_is_not_session_absence() {
    let result = |code, error: &str| std::process::Output {
        status: std::process::ExitStatus::from_raw(code << 8),
        stdout: vec![],
        stderr: error.as_bytes().to_vec(),
    };
    assert!(classify_tmux_probe(&result(0, ""), "nb_one").unwrap());
    assert!(!classify_tmux_probe(
        &result(
            1,
            "can't find session: nb_one
"
        ),
        "nb_one"
    )
    .unwrap());
    for error in [
        "no server running on /tmp/tmux/default",
        "error connecting to /tmp/tmux/default (No such file or directory)",
        "Permission denied",
        "can't find session: nb_other",
    ] {
        assert!(classify_tmux_probe(&result(1, error), "nb_one").is_err());
    }
}
