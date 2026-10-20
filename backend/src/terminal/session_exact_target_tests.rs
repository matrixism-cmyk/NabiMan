use super::*;

/// Runs one test of this file again in a child process with `envs` set, and
/// checks it really ran: a name that matches nothing would pass quietly.
fn run_isolated(test: &str, envs: &[(&str, &std::ffi::OsStr)]) {
    let module = module_path!()
        .split_once("::")
        .map_or(module_path!(), |(_, rest)| rest);
    let out = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            &format!("{module}::{test}"),
            "--nocapture",
            "--test-threads=1",
        ])
        .envs(envs.iter().copied())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success() && stdout.contains("1 passed"),
        "{stdout}\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn missing_session_does_not_match_capture_or_kill_similarly_named_session() {
    const CHILD: &str = "NABIMAN_TEST_EXACT_TMUX_CHILD";
    if let Some(directory) = std::env::var_os(CHILD) {
        std::env::set_var("TMUX_TMPDIR", directory);
        assert!(tmux()
            .args(["new-session", "-d", "-s", "nb_missing_extra", "sleep 30"])
            .output()
            .unwrap()
            .status
            .success());
        assert!(!tmux_session_state("nb_missing").unwrap());
        assert!(capture_scrollback("nb_missing", 10).is_err());
        assert!(close_tmux_session("nb_missing").is_ok());
        assert!(tmux_session_state("nb_missing_extra").unwrap());
        return;
    }
    let dir = std::env::temp_dir().join(format!("nabiman-tmux-exact-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let result = std::panic::catch_unwind(|| {
        run_isolated(
            "missing_session_does_not_match_capture_or_kill_similarly_named_session",
            &[(CHILD, dir.as_os_str())],
        )
    });
    let _ = tmux()
        .env("TMUX_TMPDIR", &dir)
        .args(["kill-server"])
        .output();
    std::fs::remove_dir_all(dir).unwrap();
    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}

/// The per-session options must land on the session itself. `set-option -t`
/// reads its target as a pane, so `=name` alone matched nothing and all three
/// were dropped without a word.
#[test]
fn new_session_carries_its_own_options() {
    const CHILD: &str = "NABIMAN_TEST_TMUX_OPTIONS_CHILD";
    if std::env::var_os(CHILD).is_some() {
        create_tmux_session("nb_opts", Some("sleep 30"), 4321, (80, 24)).unwrap();
        let local = |opt: &str| {
            let out = tmux()
                .args(["show-options", "-v", "-t", "=nb_opts:", opt])
                .output()
                .unwrap();
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        };
        assert_eq!(local("mouse"), "on");
        assert_eq!(local("history-limit"), "4321");
        assert_eq!(local("destroy-unattached"), "off");
        return;
    }
    // A private tmux server and data folder: never the running ones.
    let dir = std::env::temp_dir().join(format!("nabiman-tmux-opts-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let result = std::panic::catch_unwind(|| {
        run_isolated(
            "new_session_carries_its_own_options",
            &[
                (CHILD, std::ffi::OsStr::new("1")),
                ("TMUX_TMPDIR", dir.as_os_str()),
                ("NABIMAN_DATA_DIR", dir.as_os_str()),
            ],
        )
    });
    let _ = tmux()
        .env("TMUX_TMPDIR", &dir)
        .args(["kill-server"])
        .output();
    std::fs::remove_dir_all(dir).unwrap();
    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}
