//! The helper's system operations: what it refuses (unknown operations,
//! bad arguments, the wrong peer, an operation its policy does not approve),
//! that refusals never dispatch, that each call is audited, and that the
//! commands are fixed argv with the bounded runner.
use super::{
    audit::Audit,
    client::{Client, ClientError},
    policy::{Policy, ServiceRule},
    protocol::*,
    server,
    store::Store,
    system_ops::{self, Plan, Step, SystemPolicy, Tools},
};
use std::{
    os::unix::net::{UnixListener, UnixStream},
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

fn uid() -> u32 {
    unsafe { libc::geteuid() }
}
fn policy() -> Policy {
    Policy { api_uid: uid(), services: vec![ServiceRule { unit: "fixture.service".into(), actions: vec![ServiceAction::Restart] }] }
}
fn approve(firewall: bool, package_upgrade: bool) -> SystemPolicy {
    SystemPolicy { api_uid: uid(), firewall, package_upgrade }
}
fn request(operation: Operation) -> Request {
    Request {
        version: VERSION,
        request_id: uuid::Uuid::new_v4().to_string(),
        actor_id: "actor-1".into(),
        expires_at: chrono::Utc::now().timestamp() + 3,
        operation,
    }
}
fn ok() -> Outcome {
    Outcome { state: State::CommandSucceeded, exit_code: Some(0), backend: Some("ufw".into()), output: "done".into(), truncated: false }
}

/// One call through the helper's handler; returns the raw reply and whether
/// the system closure (the only path to a command) was reached.
fn ask(policy: &Policy, system: &SystemPolicy, pinned: bool, request: &Request, audit: &Arc<Mutex<Audit>>) -> (serde_json::Value, bool) {
    let (mut client, helper) = UnixStream::pair().unwrap();
    write_frame(&mut client, request).unwrap();
    let identity = super::identity::capture(std::process::id(), uid()).unwrap();
    let mut store = Store::memory();
    let mut reached = false;
    server::handle_with(helper, policy, system, pinned.then_some(&identity), &mut store, audit, |_, _| panic!("no service command"), |job| {
        reached = true;
        job.finish(ok());
    })
    .unwrap();
    (read_frame_within(&mut client, MAX_REPLY).unwrap(), reached)
}
fn memory() -> Arc<Mutex<Audit>> {
    Arc::new(Mutex::new(Audit::Memory(Vec::new())))
}
fn decisions(audit: &Arc<Mutex<Audit>>) -> Vec<&'static str> {
    audit.lock().unwrap().entries().iter().map(|e| e.decision).collect()
}

#[test]
fn unknown_operations_and_unknown_fields_never_parse() {
    for text in [
        r#"{"kind":"shell","command":"id"}"#,
        r#"{"kind":"run","program":"/bin/sh","args":["-c","id"]}"#,
        r#"{"kind":"firewall_add","verdict":"allow","protocol":"tcp","port":22,"port_end":null,"args":["-F"]}"#,
        r#"{"kind":"firewall_add","verdict":"accept","protocol":"tcp","port":22,"port_end":null}"#,
        r#"{"kind":"firewall_add","verdict":"allow","protocol":"icmp","port":22,"port_end":null}"#,
        r#"{"kind":"firewall_add","verdict":"allow","protocol":"tcp","port":70000,"port_end":null}"#,
        r#"{"kind":"firewall_delete","number":-1}"#,
        r#"{"kind":"package_upgrade","package":"nginx","env":{"LD_PRELOAD":"/tmp/x.so"}}"#,
        r#"{"kind":"package_refresh","manager":"/tmp/apt"}"#,
    ] {
        assert!(serde_json::from_str::<Operation>(text).is_err(), "{text}");
    }
    assert!(serde_json::from_str::<Operation>(r#"{"kind":"package_upgrade","package":null}"#).is_ok());
    assert!(serde_json::from_str::<Operation>(r#"{"kind":"firewall_status"}"#).is_ok());
}

#[test]
fn an_operation_the_policy_does_not_approve_is_refused_and_audited_without_dispatch() {
    let audit = memory();
    let (reply, reached) = ask(&policy(), &SystemPolicy::none(uid()), true, &request(Operation::FirewallStatus {}), &audit);
    assert!(!reached);
    assert_eq!(reply["status"], "rejected");
    assert!(reply["reason"].as_str().unwrap().contains("system policy"));
    // Approving packages does not approve the firewall, and the other way round.
    let (_, reached) = ask(&policy(), &approve(false, true), true, &request(Operation::FirewallDelete { number: 1 }), &audit);
    assert!(!reached);
    let (_, reached) = ask(&policy(), &approve(true, false), true, &request(Operation::PackageUpgrade { package: None }), &audit);
    assert!(!reached);
    assert_eq!(decisions(&audit), ["denied", "denied", "denied"]);
}

#[test]
fn bad_arguments_are_refused_even_when_the_family_is_approved() {
    let audit = memory();
    let all = approve(true, true);
    for operation in [
        Operation::FirewallAdd { verdict: Verdict::Allow, protocol: Protocol::Tcp, port: 0, port_end: None },
        Operation::FirewallAdd { verdict: Verdict::Allow, protocol: Protocol::Tcp, port: 9000, port_end: Some(8000) },
        Operation::FirewallAdd { verdict: Verdict::Allow, protocol: Protocol::Any, port: 8000, port_end: Some(9000) },
        Operation::FirewallDelete { number: 0 },
        Operation::FirewallDelete { number: 1_000_000 },
        Operation::PackageUpgrade { package: Some("-oAPT::Get::AllowUnauthenticated=1".into()) },
        Operation::PackageUpgrade { package: Some("--allow-unauthenticated".into()) },
        Operation::PackageUpgrade { package: Some("./evil.deb".into()) },
        Operation::PackageUpgrade { package: Some("nginx curl".into()) },
        Operation::PackageUpgrade { package: Some("nginx;id".into()) },
        Operation::PackageUpgrade { package: Some(String::new()) },
        Operation::PackageUpgrade { package: Some("a".repeat(129)) },
    ] {
        let (reply, reached) = ask(&policy(), &all, true, &request(operation.clone()), &audit);
        assert!(!reached, "{operation:?}");
        assert_eq!(reply["status"], "rejected", "{operation:?}");
    }
    assert!(system_ops::valid_package("libssl3:amd64") && system_ops::valid_package("g++-12"));
}

#[test]
fn the_wrong_peer_uid_or_an_unpinned_process_is_refused_before_reading() {
    let audit = memory();
    let other = Policy { api_uid: uid() + 1, ..policy() };
    let system = SystemPolicy { api_uid: uid() + 1, firewall: true, package_upgrade: true };
    let (reply, reached) = ask(&other, &system, true, &request(Operation::FirewallStatus {}), &audit);
    assert!(!reached);
    // The service format the client reads as "helper unavailable".
    assert_eq!((reply["status"].as_str(), reply["request_id"].as_str()), (Some("rejected"), Some("")));
    let (reply, reached) = ask(&policy(), &approve(true, true), false, &request(Operation::FirewallStatus {}), &audit);
    assert!(!reached);
    assert_eq!(reply["request_id"], "");
    assert_eq!(decisions(&audit), ["peer_rejected", "peer_rejected"]);
}

#[test]
fn expired_bad_version_and_bad_actor_never_dispatch() {
    let audit = memory();
    let mut expired = request(Operation::FirewallStatus {});
    expired.expires_at -= 10;
    let mut far = request(Operation::FirewallStatus {});
    far.expires_at += 60;
    let mut version = request(Operation::FirewallStatus {});
    version.version = 2;
    let mut actor = request(Operation::FirewallStatus {});
    actor.actor_id = "a b".into();
    for (r, status) in [(expired, "expired"), (far, "expired"), (version, "rejected"), (actor, "rejected")] {
        let (reply, reached) = ask(&policy(), &approve(true, true), true, &r, &audit);
        assert!(!reached);
        assert_eq!(reply["status"], status);
    }
}

#[test]
fn an_approved_operation_is_audited_before_and_after_and_answered_once() {
    let audit = memory();
    let (reply, reached) = ask(&policy(), &approve(true, false), true, &request(Operation::FirewallStatus {}), &audit);
    assert!(reached);
    let reply: SystemReply = serde_json::from_value(reply).unwrap();
    assert_eq!((reply.status, reply.outcome), (Status::Completed, Some(ok())));
    assert_eq!(decisions(&audit), ["dispatched", "completed"]);
    let entries = audit.lock().unwrap().entries().to_vec();
    assert_eq!(entries[0].actor_id.as_deref(), Some("actor-1"));
    assert_eq!(entries[0].operation.as_ref().unwrap()["kind"], "firewall_status");
    assert_eq!(entries[1].state, Some("command_succeeded"));
    assert_eq!(entries[0].peer_uid, uid());
}

fn tools() -> Tools {
    Tools { firewall: Some(("ufw", PathBuf::from("/usr/sbin/ufw"))), packages: Some(("apt", PathBuf::from("/usr/bin/apt-get"))) }
}
fn argv(plan: &Plan) -> Vec<Vec<String>> {
    plan.steps.iter().map(|s| std::iter::once(s.program.display().to_string()).chain(s.args.clone()).collect()).collect()
}

#[test]
fn each_operation_is_a_fixed_argv_for_the_backend_found() {
    let add = Operation::FirewallAdd { verdict: Verdict::Allow, protocol: Protocol::Tcp, port: 22, port_end: None };
    assert_eq!(argv(&system_ops::plan(&add, &tools()).unwrap()), [["/usr/sbin/ufw", "allow", "22/tcp"]]);
    let iptables = Tools { firewall: Some(("iptables", PathBuf::from("/usr/sbin/iptables"))), packages: None };
    let deny = Operation::FirewallAdd { verdict: Verdict::Deny, protocol: Protocol::Udp, port: 8000, port_end: Some(8100) };
    assert_eq!(
        argv(&system_ops::plan(&deny, &iptables).unwrap()),
        [["/usr/sbin/iptables", "-A", "INPUT", "-p", "udp", "--dport", "8000:8100", "-j", "DROP"]]
    );
    let firewalld = Tools { firewall: Some(("firewalld", PathBuf::from("/usr/bin/firewall-cmd"))), packages: None };
    assert_eq!(
        argv(&system_ops::plan(&deny, &firewalld).unwrap()),
        [
            vec!["/usr/bin/firewall-cmd", "--permanent", r#"--add-rich-rule=rule port port="8000-8100" protocol="udp" drop"#],
            vec!["/usr/bin/firewall-cmd", "--reload"],
        ]
    );
    let deny_any = Operation::FirewallAdd { verdict: Verdict::Deny, protocol: Protocol::Any, port: 22, port_end: None };
    assert_eq!(
        system_ops::firewalld_drop_args("22", Protocol::Any),
        ["--permanent", r#"--add-rich-rule=rule port port="22" protocol="tcp" drop"#, r#"--add-rich-rule=rule port port="22" protocol="udp" drop"#]
    );
    assert_eq!(argv(&system_ops::plan(&deny_any, &firewalld).unwrap())[0].len(), 4);
    // Allowing still opens the port.
    let allow = Operation::FirewallAdd { verdict: Verdict::Allow, protocol: Protocol::Tcp, port: 443, port_end: None };
    assert_eq!(argv(&system_ops::plan(&allow, &firewalld).unwrap())[0], ["/usr/bin/firewall-cmd", "--add-port", "443/tcp", "--permanent"]);
    assert!(system_ops::plan(&Operation::FirewallDelete { number: 3 }, &firewalld).is_err());
    let one = Operation::PackageUpgrade { package: Some("nginx".into()) };
    let plan = system_ops::plan(&one, &tools()).unwrap();
    assert_eq!(argv(&plan), [["/usr/bin/apt-get", "install", "--only-upgrade", "-y", "--", "nginx"]]);
    assert!(plan.tail && plan.deadline == crate::bounded_command::deadline::PACKAGE);
    let none = Tools { firewall: None, packages: None };
    assert!(system_ops::plan(&Operation::FirewallStatus {}, &none).is_err());
    assert!(system_ops::plan(&Operation::PackageRefresh {}, &none).is_err());
    // Service operations are not system operations.
    let service = Operation::Execute { unit: "fixture.service".into(), action: ServiceAction::Restart };
    assert!(system_ops::plan(&service, &tools()).is_err() && !system_ops::is_system(&service));
}

#[test]
fn commands_run_bounded_and_report_what_happened() {
    let plan = |program: &str, args: &[&str], ms: u64| Plan {
        backend: "fixture",
        steps: vec![Step { program: program.into(), args: args.iter().map(|a| a.to_string()).collect() }],
        deadline: Duration::from_millis(ms),
        tail: false,
    };
    let started = std::time::Instant::now();
    assert_eq!(system_ops::execute(&plan("/bin/sleep", &["30"], 300)).state, State::UnknownResult);
    assert!(started.elapsed() < Duration::from_secs(10), "the deadline stops the command");
    assert_eq!(system_ops::execute(&plan("/nonexistent-nabiman-fixture", &[], 1000)).state, State::NotDispatched);
    let failed = system_ops::execute(&plan("/bin/false", &[], 5000));
    assert_eq!((failed.state, failed.exit_code), (State::CommandFailed, Some(1)));
    // The environment is fixed: nothing of the helper's own reaches the command.
    std::env::set_var("NABIMAN_FIXTURE_LEAK", "leaked");
    let env = system_ops::execute(&plan("/usr/bin/env", &[], 5000));
    assert!(env.output.contains("PATH=/usr/sbin:/usr/bin:/sbin:/bin") && !env.output.contains("leaked"));
}

#[test]
fn output_is_printable_and_bounded() {
    let (text, cut) = system_ops::bounded_text(b"ok\x1b[31m red\x07\n", false);
    assert_eq!((text.as_str(), cut), ("ok [31m red \n", false));
    let long = "가".repeat(system_ops::KEEP);
    let (head, cut) = system_ops::bounded_text(long.as_bytes(), false);
    assert!(cut && head.len() <= system_ops::KEEP && head.chars().all(|c| c == '가'));
    let (tail, _) = system_ops::bounded_text(format!("{long}END").as_bytes(), true);
    assert!(tail.ends_with("END") && tail.len() <= system_ops::KEEP);
}

#[test]
fn the_system_policy_file_is_optional_and_an_unsafe_one_is_refused() {
    let missing = std::env::temp_dir().join(format!("nabiman-system-{}.json", uuid::Uuid::new_v4()));
    assert_eq!(SystemPolicy::load(&missing, uid()).unwrap(), SystemPolicy::none(uid()));
    std::fs::write(&missing, format!(r#"{{"api_uid":{},"firewall":true}}"#, uid())).unwrap();
    // Not root-owned, in a world-writable directory: refused, not trusted.
    assert!(SystemPolicy::load(&missing, uid()).is_err());
    std::fs::remove_file(&missing).unwrap();
}

#[test]
fn the_client_reads_refusals_and_uncorrelated_answers_without_retrying() {
    let serve = |answer: serde_json::Value| {
        let path = std::env::temp_dir().join(format!("nabiman-system-{}.sock", uuid::Uuid::new_v4()));
        let listener = UnixListener::bind(&path).unwrap();
        let thread = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let asked: Request = read_frame(&mut socket).unwrap();
            let mut answer = answer;
            if answer["request_id"] == "same" {
                answer["request_id"] = asked.request_id.clone().into();
            }
            write_frame_within(&mut socket, &answer, MAX_REPLY).unwrap();
        });
        (path, thread)
    };
    let client = |path: &PathBuf| Client::new(path.clone(), uid());
    let cases = [
        (serde_json::json!({"version":1,"request_id":"","status":"rejected","receipt":null}), "unavailable"),
        (serde_json::json!({"version":1,"request_id":"other","status":"completed","outcome":ok(),"reason":null}), "unknown"),
        (serde_json::json!({"version":1,"request_id":"same","status":"completed","outcome":null,"reason":null}), "unknown"),
        (serde_json::json!({"version":1,"request_id":"same","status":"completed","outcome":ok(),"reason":null}), "ok"),
    ];
    for (answer, expect) in cases {
        let (path, thread) = serve(answer);
        let got = client(&path).request_system(&request(Operation::FirewallStatus {}));
        thread.join().unwrap();
        std::fs::remove_file(&path).unwrap();
        let seen = match got {
            Ok(_) => "ok",
            Err(ClientError::Unavailable) => "unavailable",
            Err(ClientError::UnknownResult) => "unknown",
        };
        assert_eq!(seen, expect);
    }
    // A service request never goes down the system path.
    let nowhere = Client::new("/nonexistent/helper.sock".into(), 0);
    let service = request(Operation::Execute { unit: "fixture.service".into(), action: ServiceAction::Restart });
    assert!(matches!(nowhere.request_system(&service), Err(ClientError::UnknownResult)));
}
