use super::{
    client::{Client, ClientError},
    executor,
    policy::{canonical_unit, Policy, ServiceRule},
    protocol::*,
    server,
    store::Store,
};
use std::{
    os::unix::net::{UnixListener, UnixStream},
    process::Command,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
fn api_identity() -> super::identity::ProcessIdentity {
    super::identity::capture(std::process::id(), unsafe { libc::geteuid() }).unwrap()
}
fn policy() -> Policy {
    Policy {
        api_uid: unsafe { libc::geteuid() },
        services: vec![ServiceRule {
            unit: "fixture.service".into(),
            actions: vec![ServiceAction::Restart],
        }],
    }
}
fn request() -> Request {
    Request {
        version: VERSION,
        request_id: uuid::Uuid::new_v4().to_string(),
        actor_id: "actor-1".into(),
        expires_at: chrono::Utc::now().timestamp() + 3,
        operation: Operation::Execute {
            unit: "fixture.service".into(),
            action: ServiceAction::Restart,
        },
    }
}
fn roundtrip(store: &mut Store, policy: &Policy, request: &Request, count: &AtomicUsize) -> Reply {
    let (mut client, helper) = UnixStream::pair().unwrap();
    write_frame(&mut client, request).unwrap();
    server::handle(
        helper,
        policy,
        Some(&api_identity()),
        store,
        |unit, action| {
            assert_eq!(unit, "fixture.service");
            assert_eq!(action, ServiceAction::Restart);
            count.fetch_add(1, Ordering::SeqCst);
            (State::CommandSucceeded, Some(0))
        },
    )
    .unwrap();
    read_frame(&mut client).unwrap()
}
#[test]
fn names_cannot_select_options_globs_paths_unicode_or_templates() {
    for name in [
        "",
        "--all",
        "*.service",
        "../../root",
        "a/b",
        "foo@",
        "한글",
        "foo;reboot",
        "a b",
        "a..b",
    ] {
        assert!(canonical_unit(name).is_none(), "{name}");
    }
    assert_eq!(canonical_unit("nginx").unwrap(), "nginx.service");
    assert_eq!(
        canonical_unit("worker@one.service").unwrap(),
        "worker@one.service"
    );
}
#[test]
fn root_self_management_duplicate_rules_actions_and_unknown_fields_are_rejected() {
    let mut value = policy();
    value.api_uid = 0;
    assert!(value.validate().is_err());
    value.api_uid = 1000;
    value.services[0].unit = "nabiman.service".into();
    assert!(value.validate().is_err());
    value.services[0].unit = "fixture.service".into();
    value.services.push(value.services[0].clone());
    assert!(value.validate().is_err());
    value.services.pop();
    value.services[0].actions.push(ServiceAction::Restart);
    assert!(value.validate().is_err());
    assert!(serde_json::from_str::<Policy>(
        r#"{"api_uid":1000,"services":[],"program":"/bin/sh"}"#
    )
    .is_err());
    assert!(serde_json::from_str::<Request>(r#"{"version":1,"request_id":"x","actor_id":"a","operation":{"kind":"execute","unit":"fixture.service","action":"restart","args":["--all"]}}"#).is_err());
}
#[test]
fn framed_input_is_bounded_and_trailing_json_and_unknown_actions_are_rejected() {
    assert!(read_frame::<Request>(&mut &(50000u32.to_be_bytes())[..]).is_err());
    let bytes = b"{}{}";
    let mut frame = (bytes.len() as u32).to_be_bytes().to_vec();
    frame.extend(bytes);
    assert!(read_frame::<Request>(&mut &frame[..]).is_err());
    assert!(serde_json::from_str::<ServiceAction>("\"daemon-reexec\"").is_err());
}
#[test]
fn peer_uid_is_kernel_identity_and_unapproved_peer_never_dispatches() {
    let mut store = Store::memory();
    let mut policy = policy();
    policy.api_uid += 1;
    let (mut client, helper) = UnixStream::pair().unwrap();
    assert_eq!(server::peer_uid(&helper).unwrap(), unsafe {
        libc::geteuid()
    });
    server::handle(
        helper,
        &policy,
        Some(&api_identity()),
        &mut store,
        |_, _| panic!("unapproved peer dispatched"),
    )
    .unwrap();
    let reply: Reply = read_frame(&mut client).unwrap();
    assert_eq!(reply.status, Status::Rejected);
}
#[test]
fn exact_policy_denies_other_unit_and_other_action_before_intent() {
    let mut store = Store::memory();
    let count = AtomicUsize::new(0);
    for operation in [
        Operation::Execute {
            unit: "foreign.service".into(),
            action: ServiceAction::Restart,
        },
        Operation::Execute {
            unit: "fixture.service".into(),
            action: ServiceAction::Stop,
        },
    ] {
        let mut request = request();
        request.operation = operation;
        assert_eq!(
            roundtrip(&mut store, &policy(), &request, &count).status,
            Status::Rejected
        );
        assert!(store.get(&request.request_id).unwrap().is_none());
    }
    assert_eq!(count.load(Ordering::SeqCst), 0);
}
#[test]
fn durable_intent_completes_once_and_duplicate_or_other_actor_does_not_replay() {
    let mut store = Store::memory();
    let request = request();
    let count = AtomicUsize::new(0);
    let reply = roundtrip(&mut store, &policy(), &request, &count);
    assert_eq!(reply.receipt.unwrap().state, State::CommandSucceeded);
    assert_eq!(
        roundtrip(&mut store, &policy(), &request, &count).status,
        Status::Conflict
    );
    let mut stolen = request.clone();
    stolen.actor_id = "other".into();
    let reply = roundtrip(&mut store, &policy(), &stolen, &count);
    assert_eq!(reply.status, Status::Rejected);
    assert!(reply.receipt.is_none());
    assert_eq!(count.load(Ordering::SeqCst), 1);
}
#[test]
fn restart_recovers_running_as_unknown_and_inspection_never_executes() {
    let mut store = Store::memory();
    let request = request();
    store
        .begin(
            &request,
            "fixture.service",
            ServiceAction::Restart,
            &policy(),
        )
        .unwrap();
    store.recover().unwrap();
    assert_eq!(
        store.get(&request.request_id).unwrap().unwrap().state,
        State::UnknownResult
    );
    let mut inspect = request.clone();
    inspect.request_id = uuid::Uuid::new_v4().to_string();
    inspect.operation = Operation::Inspect {
        operation_id: request.request_id,
    };
    let count = AtomicUsize::new(0);
    let reply = roundtrip(&mut store, &policy(), &inspect, &count);
    assert_eq!(reply.receipt.unwrap().state, State::UnknownResult);
    assert_eq!(count.load(Ordering::SeqCst), 0);
    inspect.actor_id = "other".into();
    let reply = roundtrip(&mut store, &policy(), &inspect, &count);
    assert_eq!(reply.status, Status::NotFound);
    assert!(reply.receipt.is_none());
}
#[test]
fn intent_failure_prevents_dispatch_and_confirmation_failure_leaves_unknown_recovery() {
    let mut store = Store::memory();
    store.inject_failure(false);
    let request = request();
    let (mut client, helper) = UnixStream::pair().unwrap();
    write_frame(&mut client, &request).unwrap();
    assert!(server::handle(
        helper,
        &policy(),
        Some(&api_identity()),
        &mut store,
        |_, _| panic!("No committed intent")
    )
    .is_err());
    let mut store = Store::memory();
    store.inject_failure(true);
    let (mut client, helper) = UnixStream::pair().unwrap();
    write_frame(&mut client, &request).unwrap();
    let count = AtomicUsize::new(0);
    assert!(server::handle(
        helper,
        &policy(),
        Some(&api_identity()),
        &mut store,
        |_, _| {
            count.fetch_add(1, Ordering::SeqCst);
            (State::CommandSucceeded, Some(0))
        }
    )
    .is_err());
    assert_eq!(count.load(Ordering::SeqCst), 1);
    assert_eq!(
        store.get(&request.request_id).unwrap().unwrap().state,
        State::Running
    );
    store.clear_failure();
    store.recover().unwrap();
    assert_eq!(
        store.get(&request.request_id).unwrap().unwrap().state,
        State::UnknownResult
    );
}
#[test]
fn command_deadline_has_unknown_result_and_missing_program_never_dispatches() {
    let start = Instant::now();
    let (state, code) = executor::execute(
        Command::new("/bin/sleep").arg("5"),
        Duration::from_millis(30),
    );
    assert_eq!((state, code), (State::UnknownResult, None));
    assert!(start.elapsed() < Duration::from_secs(2));
    assert_eq!(
        executor::execute(
            &mut Command::new("/nonexistent-nabiman-fixture"),
            Duration::from_secs(1)
        ),
        (State::NotDispatched, None)
    );
    assert_eq!(
        executor::execute(&mut Command::new("/bin/true"), Duration::from_secs(1)),
        (State::CommandSucceeded, Some(0))
    );
    assert_eq!(
        executor::execute(&mut Command::new("/bin/false"), Duration::from_secs(1)),
        (State::CommandFailed, Some(1))
    );
}
#[test]
fn client_rejects_wrong_peer_and_uncorrelated_response_without_retry() {
    let path =
        std::env::temp_dir().join(format!("nabiman-helper-test-{}.sock", uuid::Uuid::new_v4()));
    let listener = UnixListener::bind(&path).unwrap();
    let client = Client::new(path.clone(), unsafe { libc::geteuid() } + 1);
    let wrong = std::thread::spawn(move || {
        let (socket, _) = listener.accept().unwrap();
        drop(socket);
    });
    assert!(matches!(
        client.request(&request()),
        Err(ClientError::Unavailable)
    ));
    wrong.join().unwrap();
    std::fs::remove_file(&path).unwrap();
    let listener = UnixListener::bind(&path).unwrap();
    let count = Arc::new(AtomicUsize::new(0));
    let seen = count.clone();
    let server = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let _: Request = read_frame(&mut socket).unwrap();
        seen.fetch_add(1, Ordering::SeqCst);
        write_frame(
            &mut socket,
            &Reply {
                version: VERSION,
                request_id: uuid::Uuid::new_v4().to_string(),
                status: Status::Completed,
                receipt: None,
            },
        )
        .unwrap();
    });
    let client = Client::new(path.clone(), unsafe { libc::geteuid() });
    assert!(matches!(
        client.request(&request()),
        Err(ClientError::UnknownResult)
    ));
    server.join().unwrap();
    assert_eq!(count.load(Ordering::SeqCst), 1);
    std::fs::remove_file(&path).unwrap();
}

#[test]
fn expired_or_overflow_deadline_never_creates_intent_or_dispatches() {
    let mut store = Store::memory();
    let count = AtomicUsize::new(0);
    for expires_at in [i64::MIN, 0, chrono::Utc::now().timestamp() + 300] {
        let mut request = request();
        request.expires_at = expires_at;
        assert_eq!(
            roundtrip(&mut store, &policy(), &request, &count).status,
            Status::Expired
        );
        assert!(store.get(&request.request_id).unwrap().is_none());
    }
    assert_eq!(count.load(Ordering::SeqCst), 0);
}
#[test]
fn unresolved_unit_blocks_a_new_operation_key_after_restart_without_replay() {
    let mut store = Store::memory();
    let first = request();
    store
        .begin(&first, "fixture.service", ServiceAction::Restart, &policy())
        .unwrap();
    store.recover().unwrap();
    let next = request();
    let count = AtomicUsize::new(0);
    let reply = roundtrip(&mut store, &policy(), &next, &count);
    assert_eq!(reply.status, Status::Conflict);
    assert_eq!(reply.receipt.unwrap().operation_id, first.request_id);
    assert!(store.get(&next.request_id).unwrap().is_none());
    assert_eq!(count.load(Ordering::SeqCst), 0);
}

#[test]
fn full_unix_accept_queue_fails_without_a_blocking_connect_or_retry() {
    use std::os::fd::AsRawFd;
    let path =
        std::env::temp_dir().join(format!("nabiman-helper-full-{}.sock", uuid::Uuid::new_v4()));
    let listener = UnixListener::bind(&path).unwrap();
    assert_eq!(unsafe { libc::listen(listener.as_raw_fd(), 0) }, 0);
    let held = UnixStream::connect(&path).unwrap();
    let client = Client::new(path.clone(), unsafe { libc::geteuid() });
    let start = Instant::now();
    assert!(matches!(
        client.request(&request()),
        Err(ClientError::Unavailable)
    ));
    assert!(start.elapsed() < Duration::from_secs(1));
    drop(held);
    drop(listener);
    std::fs::remove_file(path).unwrap();
}
