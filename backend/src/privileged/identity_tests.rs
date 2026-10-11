use super::{
    identity,
    policy::{Policy, ServiceRule},
    protocol::*,
    server,
    store::Store,
};
use std::os::unix::net::UnixStream;
#[test]
fn same_uid_wrong_process_or_reused_pid_or_previous_boot_cannot_dispatch() {
    let uid = unsafe { libc::geteuid() };
    let identity = identity::capture(std::process::id(), uid).unwrap();
    assert!(identity::capture(std::process::id(), uid + 1).is_err());
    let policy = Policy {
        api_uid: uid,
        services: vec![ServiceRule {
            unit: "fixture.service".into(),
            actions: vec![ServiceAction::Restart],
        }],
    };
    for kind in 0..4 {
        let mut approved = identity.clone();
        match kind {
            0 => approved.pid += 1,
            1 => approved.start_ticks += 1,
            2 => approved.boot_id = uuid::Uuid::new_v4().to_string(),
            _ => {}
        }
        let expected = if kind == 3 { None } else { Some(&approved) };
        let (mut client, helper) = UnixStream::pair().unwrap();
        let mut store = Store::memory();
        server::handle(helper, &policy, expected, &mut store, |_, _| {
            panic!("Unpinned process dispatched")
        })
        .unwrap();
        let reply: Reply = read_frame(&mut client).unwrap();
        assert_eq!(reply.status, Status::Rejected);
        assert!(reply.receipt.is_none());
    }
}
