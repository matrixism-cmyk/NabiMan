use super::*;
use actix_web::{test as atest, App};
use std::os::unix::process::ExitStatusExt;

/// What the fake tmux says (a static: the probe runs on the blocking pool).
pub(crate) static FAKE_TMUX: Mutex<Tmux> = Mutex::new(Tmux::Reachable);

fn output(code: i32, stderr: &str) -> std::io::Result<std::process::Output> {
    Ok(std::process::Output { status: std::process::ExitStatus::from_raw(code << 8), stdout: vec![], stderr: stderr.into() })
}

#[test]
fn tmux_answers_are_read_without_starting_a_server() {
    assert_eq!(classify(output(1, "can't find session: =nabiman-readiness-probe\n")), Tmux::Reachable);
    assert_eq!(classify(output(0, "")), Tmux::Reachable);
    assert_eq!(classify(output(1, "no server running on /tmp/tmux-1000/default\n")), Tmux::NoServer);
    assert_eq!(classify(output(1, "error connecting to /tmp/tmux-1000/default (No such file or directory)\n")), Tmux::NoServer);
    assert_eq!(classify(output(1, "lost server\n")), Tmux::Unreachable);
    assert_eq!(classify(Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "Timed out"))), Tmux::Unreachable);
}

#[test]
fn readiness_names_each_problem() {
    assert!(reasons(true, true, Tmux::Reachable, 3).is_empty());
    assert!(reasons(true, true, Tmux::NoServer, 0).is_empty(), "no sessions, no tmux server: ready");
    assert_eq!(reasons(true, true, Tmux::NoServer, 2), ["tmux_unreachable"]);
    assert_eq!(
        reasons(false, false, Tmux::Unreachable, 0),
        ["data_dir_not_writable", "audit_journal_unavailable", "tmux_unreachable"]
    );
}

#[test]
fn writable_leaves_nothing_behind() {
    let dir = std::env::temp_dir().join(format!("nabiman-readyz-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    assert!(writable(&dir));
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);
    assert!(!writable(&dir.join("missing")));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[actix_web::test]
async fn liveness_is_a_bare_ok_and_readiness_a_short_reason_list() {
    *FAKE_TMUX.lock_safe() = Tmux::Unreachable;
    *LAST_TMUX.lock_safe() = None;
    *LAST_ANSWER.lock_safe() = None;
    let dir = std::env::temp_dir().join(format!("nabiman-readyz-audit-{}", uuid::Uuid::new_v4()));
    let audit = crate::audit::log_at(dir.join("audit.jsonl"));
    let sessions: crate::terminal::PtyStore = Default::default();
    let app = atest::init_service(
        App::new()
            .app_data(web::Data::new(audit))
            .app_data(web::Data::new(sessions))
            .configure(super::super::config),
    )
    .await;
    let res = atest::call_service(&app, atest::TestRequest::get().uri("/healthz").to_request()).await;
    assert_eq!(res.status(), 200);
    assert_eq!(atest::read_body(res).await, "ok");

    let res = atest::call_service(&app, atest::TestRequest::get().uri("/readyz").to_request()).await;
    assert_eq!(res.status(), 503);
    let body: serde_json::Value = atest::read_body_json(res).await;
    assert_eq!(body["status"], "not_ready");
    let reasons: Vec<&str> = body["reasons"].as_array().unwrap().iter().map(|r| r.as_str().unwrap()).collect();
    assert!(reasons.contains(&"tmux_unreachable"), "{reasons:?}");
    assert!(!body.to_string().contains('/'), "no paths in the answer: {body}");
    *LAST_ANSWER.lock_safe() = None;
    *FAKE_TMUX.lock_safe() = Tmux::Reachable;
}
