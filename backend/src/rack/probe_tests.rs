use std::net::Ipv4Addr;

use super::probe::*;
use super::reach::{Method, Outcome, Prober};

/// The only address a test ever really pings: this machine.
#[test]
fn the_system_ping_is_asked_once_and_read() {
    match ping(Ipv4Addr::LOCALHOST) {
        Outcome::Reply { ms } => assert!(ms.is_some_and(|ms| (0.0..1000.0).contains(&ms)), "{ms:?}"),
        // A build machine without ping, or one that may not open the socket, says so.
        Outcome::Unavailable(why) => assert!(["ping_missing", "ping_denied"].contains(&why), "{why}"),
        Outcome::NoReply => panic!("127.0.0.1 did not answer"),
    }
}

#[test]
fn what_ping_says_is_told_apart() {
    let reply = "PING 10.0.0.1 (10.0.0.1) 56(84) bytes of data.\n64 bytes from 10.0.0.1: icmp_seq=1 ttl=64 time=0.612 ms\n\n--- 10.0.0.1 ping statistics ---\n1 packets transmitted, 1 received, 0% packet loss, time 0ms\nrtt min/avg/max/mdev = 0.612/0.612/0.612/0.000 ms\n";
    assert_eq!(judge_ping(Some(0), reply, ""), Outcome::Reply { ms: Some(0.612) });
    assert_eq!(judge_ping(Some(0), "64 bytes from 10.0.0.1: icmp_seq=1 ttl=64 time=12 ms", ""), Outcome::Reply { ms: Some(12.0) });
    assert_eq!(judge_ping(Some(0), "an answer in words this code does not know", ""), Outcome::Reply { ms: None });
    assert_eq!(judge_ping(Some(0), "time=abc ms", ""), Outcome::Reply { ms: None });
    assert_eq!(judge_ping(Some(1), "1 packets transmitted, 0 received, 100% packet loss", ""), Outcome::NoReply);
    assert_eq!(judge_ping(Some(1), "From 10.0.0.9 icmp_seq=1 Destination Host Unreachable", ""), Outcome::NoReply);
    assert_eq!(judge_ping(Some(2), "", "ping: socket: Operation not permitted\n"), Outcome::Unavailable("ping_denied"));
    assert_eq!(judge_ping(Some(2), "", "ping: socktype: SOCK_RAW\nping: socket: Permission denied\n"), Outcome::Unavailable("ping_denied"));
    assert_eq!(judge_ping(Some(2), "", "ping: connect: Network is unreachable\n"), Outcome::Unavailable("no_route"));
    assert_eq!(judge_ping(Some(2), "", "ping: invalid option -- 'n'\n"), Outcome::Unavailable("ping_failed"));
    assert_eq!(judge_ping(None, "", ""), Outcome::Unavailable("ping_failed"), "stopped by a signal");
}

#[actix_web::test]
async fn a_tcp_check_is_one_connect_to_the_port() {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let open = SystemProber.probe((Ipv4Addr::LOCALHOST, Method::Tcp(port))).await;
    assert!(matches!(open, Outcome::Reply { ms: Some(ms) } if (0.0..1000.0).contains(&ms)), "{open:?}");
    let (_stream, _) = listener.accept().await.unwrap();
    assert!(tokio::time::timeout(std::time::Duration::from_millis(100), listener.accept()).await.is_err(), "one connect, not two");
    drop(listener);
    // Nothing listens there any more: refused is no answer, not an error.
    assert_eq!(SystemProber.probe((Ipv4Addr::LOCALHOST, Method::Tcp(port))).await, Outcome::NoReply);
}

#[actix_web::test]
async fn the_prober_pings_through_the_blocking_pool() {
    let outcome = SystemProber.probe((Ipv4Addr::LOCALHOST, Method::Ping)).await;
    assert!(!matches!(outcome, Outcome::NoReply), "{outcome:?}");
}
