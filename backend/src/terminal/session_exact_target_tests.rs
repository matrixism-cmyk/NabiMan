use super::*;
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
    let result=std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact","terminal::session::exact_target_tests::missing_session_does_not_match_capture_or_kill_similarly_named_session","--nocapture"]).env(CHILD,&dir).output().unwrap();
    let _ = tmux()
        .env("TMUX_TMPDIR", &dir)
        .args(["kill-session", "-t", "=nb_missing_extra"])
        .output();
    std::fs::remove_dir_all(dir).unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}
