use super::*;
use chrono::{Duration as ChronoDuration, Utc};
use russh::server;
use std::sync::atomic::{AtomicUsize, Ordering};

fn pin(key: &russh::keys::PrivateKey) -> String {
    key.public_key()
        .fingerprint(russh::keys::HashAlg::Sha256)
        .to_string()
}

#[test]
fn host_trust_requires_canonical_pin_and_time_bounded_exception() {
    let key = russh::keys::PrivateKey::random(&mut rand10::rng(), russh::keys::Algorithm::Ed25519)
        .unwrap();
    let fingerprint = pin(&key);
    let now = Utc::now();
    let strict = HostTrust::parse(&fingerprint, None, now).unwrap();
    assert!(strict.matches(&key.public_key()));
    assert!(!strict.legacy);
    for invalid in [
        "",
        "MD5:abc",
        "SHA256:abc",
        &format!("{}=", fingerprint),
        &format!("{}\n", fingerprint),
        &format!("SHA256:{}B", "A".repeat(42)),
    ] {
        assert!(HostTrust::parse(invalid, None, now).is_err());
    }
    for expiry in [
        (now - ChronoDuration::seconds(1)).to_rfc3339(),
        now.to_rfc3339(),
        (now + ChronoDuration::days(31)).to_rfc3339(),
        "invalid".into(),
    ] {
        assert!(HostTrust::parse(&fingerprint, Some(&expiry), now).is_err());
    }
    let expiry = (now + ChronoDuration::days(1)).to_rfc3339();
    assert!(
        HostTrust::parse(&fingerprint, Some(&expiry), now)
            .unwrap()
            .legacy
    );
    assert!(HostTrust::parse(&fingerprint, Some(&expiry), now + ChronoDuration::days(2)).is_err());
}

struct AuthObserver(Arc<AtomicUsize>);

impl server::Handler for AuthObserver {
    type Error = russh::Error;
    async fn auth_password(
        &mut self,
        _user: &str,
        _password: &str,
    ) -> Result<server::Auth, Self::Error> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(server::Auth::Accept)
    }
}

async fn ssh_fixture(expected_matches: bool, legacy_server: bool, allow_legacy: bool) {
    ssh_fixture_algorithm(
        expected_matches,
        legacy_server,
        allow_legacy,
        russh::keys::Algorithm::Ed25519,
    )
    .await;
}

async fn ssh_fixture_algorithm(
    expected_matches: bool,
    legacy_server: bool,
    allow_legacy: bool,
    algorithm: russh::keys::Algorithm,
) {
    let key = russh::keys::PrivateKey::random(&mut rand10::rng(), algorithm).unwrap();
    let trusted = if expected_matches {
        pin(&key)
    } else {
        pin(
            &russh::keys::PrivateKey::random(&mut rand10::rng(), russh::keys::Algorithm::Ed25519)
                .unwrap(),
        )
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let auth_count = Arc::new(AtomicUsize::new(0));
    let observed = auth_count.clone();
    let config = Arc::new(server::Config {
        keys: vec![key],
        preferred: if legacy_server {
            Preferred {
                kex: Cow::Owned(vec![kex::DH_G14_SHA1]),
                ..Preferred::DEFAULT
            }
        } else {
            Preferred::DEFAULT
        },
        auth_rejection_time: Duration::ZERO,
        auth_rejection_time_initial: Some(Duration::ZERO),
        ..Default::default()
    });
    let task = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        if let Ok(session) = server::run_stream(config, stream, AuthObserver(observed)).await {
            let _ = session.await;
        }
    });
    let result = AxgateSession::connect(&AxgateRealConfig {
        host: "127.0.0.1".into(),
        port,
        username: "test".into(),
        password: "test-password".into(),
        // Protocol fixtures are not latency benchmarks: debug DH on shared CI is slow.
        timeout_secs: 15,
        host_key_sha256: trusted,
        legacy_until: allow_legacy.then(|| (Utc::now() + ChronoDuration::hours(1)).to_rfc3339()),
    })
    .await;
    if expected_matches && (!legacy_server || allow_legacy) {
        assert!(
            result.is_ok(),
            "{}",
            result.err().map(|e| e.to_string()).unwrap_or_default()
        );
        assert_eq!(auth_count.load(Ordering::SeqCst), 1);
    } else {
        let err = match result {
            Ok(_) => panic!("Changed host key was accepted"),
            Err(err) => err,
        };
        if !expected_matches {
            assert!(err.to_string().contains("host key mismatch"), "{err}");
        }
        assert_eq!(
            auth_count.load(Ordering::SeqCst),
            0,
            "Password must never reach an untrusted server"
        );
    }
    task.abort();
    let _ = task.await;
}

#[tokio::test]
async fn matching_key_allows_password_authentication() {
    ssh_fixture(true, false, false).await;
}

#[tokio::test]
async fn changed_key_is_rejected_before_password_authentication() {
    ssh_fixture(false, false, false).await;
}

#[tokio::test]
async fn missing_trust_blocks_before_opening_a_socket() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let result = AxgateSession::connect(&AxgateRealConfig {
        host: "127.0.0.1".into(),
        port,
        username: "test".into(),
        password: "test-password".into(),
        // Protocol fixtures are not latency benchmarks: debug DH on shared CI is slow.
        timeout_secs: 15,
        host_key_sha256: String::new(),
        legacy_until: None,
    })
    .await;
    assert!(result.is_err());
    assert!(
        tokio::time::timeout(Duration::from_millis(100), listener.accept())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn legacy_server_is_rejected_without_exception() {
    ssh_fixture(true, true, false).await;
}

#[tokio::test]
async fn legacy_server_accepts_verified_key_with_unexpired_exception() {
    ssh_fixture(true, true, true).await;
}

#[tokio::test]
async fn rsa_host_key_remains_verified_before_password_authentication() {
    ssh_fixture_algorithm(
        true,
        false,
        false,
        russh::keys::Algorithm::Rsa { hash: None },
    )
    .await;
    ssh_fixture_algorithm(
        false,
        false,
        false,
        russh::keys::Algorithm::Rsa { hash: None },
    )
    .await;
}
