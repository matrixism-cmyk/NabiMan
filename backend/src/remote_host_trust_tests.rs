use super::*;
use russh::server;
use std::os::unix::fs::PermissionsExt;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

fn server_record(public_key: String, fingerprint: String, port: u16) -> RemoteServer {
    RemoteServer {
        id: "srv_test".into(),
        name: "Test".into(),
        host: "127.0.0.1".into(),
        port,
        user: "test".into(),
        auth_method: "key".into(),
        tags: vec![],
        memo: String::new(),
        created_at: String::new(),
        last_checked: String::new(),
        status: String::new(),
        password_enc: String::new(),
        has_password: false,
        host_trust_version: uuid::Uuid::new_v4().to_string(),
        host_trust: Some(HostKeyTrust {
            public_key,
            fingerprint,
            verified_by: "admin".into(),
            verified_at: String::new(),
            reason: "Console verification".into(),
        }),
    }
}

fn public(key: &russh::keys::PrivateKey) -> (String, String) {
    let key = key.public_key();
    let line = format!("{} {}", key.algorithm().as_str(), key.public_key_base64());
    parse_key(&line).unwrap()
}

#[test]
fn key_parser_rejects_fingerprints_multiline_and_wrong_type() {
    let (key, _) = public(
        &russh::keys::PrivateKey::random(&mut rand10::rng(), russh::keys::Algorithm::Ed25519)
            .unwrap(),
    );
    assert!(parse_key("SHA256:abc").is_err());
    assert!(parse_key(&format!("{key}\n{key}")).is_err());
    assert!(parse_key(&key.replace("ssh-ed25519", "ssh-rsa")).is_err());
    assert!(parse_key("ssh-ed25519 not-base64").is_err());
}

#[test]
fn trust_files_are_private_and_versioned_and_missing_trust_fails() {
    let directory = std::env::temp_dir().join(format!("nabiman-trust-{}", uuid::Uuid::new_v4()));
    let (key, fingerprint) = public(
        &russh::keys::PrivateKey::random(&mut rand10::rng(), russh::keys::Algorithm::Ed25519)
            .unwrap(),
    );
    let mut record = server_record(key, fingerprint, 2222);
    let options = options_at(&record, &directory).unwrap();
    let file = options
        .iter()
        .find_map(|s| s.strip_prefix("UserKnownHostsFile="))
        .unwrap();
    assert_eq!(
        std::fs::metadata(file).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert!(options.contains(&"StrictHostKeyChecking=yes".into()));
    record.port = 2223;
    let next = options_at(&record, &directory).unwrap();
    assert_ne!(options, next);
    record.host_trust = None;
    assert!(options_at(&record, &directory).is_err());
    std::fs::remove_dir_all(directory).unwrap();
}

struct Observer(Arc<AtomicUsize>);
impl server::Handler for Observer {
    type Error = russh::Error;
    async fn auth_none(&mut self, _user: &str) -> Result<server::Auth, Self::Error> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(server::Auth::Accept)
    }
    async fn channel_open_session(
        &mut self,
        _channel: russh::Channel<server::Msg>,
        reply: server::ChannelOpenHandle,
        _session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        reply.accept().await;
        Ok(())
    }
    async fn exec_request(
        &mut self,
        channel: russh::ChannelId,
        _data: &[u8],
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        session.channel_success(channel)?;
        session.exit_status_request(channel, 0)?;
        session.eof(channel)?;
        session.close(channel)?;
        Ok(())
    }
}

async fn openssh_fixture(matches: bool) {
    let directory = std::env::temp_dir().join(format!("nabiman-openssh-{}", uuid::Uuid::new_v4()));
    let key = russh::keys::PrivateKey::random(&mut rand10::rng(), russh::keys::Algorithm::Ed25519)
        .unwrap();
    let (trusted, fingerprint) = if matches {
        public(&key)
    } else {
        public(
            &russh::keys::PrivateKey::random(&mut rand10::rng(), russh::keys::Algorithm::Ed25519)
                .unwrap(),
        )
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let observed = Arc::new(AtomicUsize::new(0));
    let counter = observed.clone();
    let task = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let config = Arc::new(server::Config {
            keys: vec![key],
            ..Default::default()
        });
        if let Ok(session) = server::run_stream(config, socket, Observer(counter)).await {
            let _ = session.await;
        }
    });
    let options = options_at(&server_record(trusted, fingerprint, port), &directory).unwrap();
    let result = tokio::task::spawn_blocking(move || {
        std::process::Command::new("ssh")
            .args([
                "-F",
                "/dev/null",
                "-o",
                "BatchMode=yes",
                "-o",
                "ConnectTimeout=5",
                "-o",
                "PreferredAuthentications=none",
            ])
            .args(options)
            .args(["-p", &port.to_string(), "test@127.0.0.1", "true"])
            .output()
            .unwrap()
    })
    .await
    .unwrap();
    assert_eq!(
        result.status.success(),
        matches,
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    if matches {
        assert!(observed.load(Ordering::SeqCst) > 0);
    } else {
        assert_eq!(observed.load(Ordering::SeqCst), 0);
    }
    task.abort();
    let _ = task.await;
    std::fs::remove_dir_all(directory).unwrap();
}

#[tokio::test]
async fn openssh_uses_verified_key_at_nonstandard_port() {
    openssh_fixture(true).await;
}

#[tokio::test]
async fn openssh_rejects_changed_key_before_authentication() {
    openssh_fixture(false).await;
}

#[test]
fn rsa_ecdsa_and_ed25519_pins_match_openssh_fingerprints() {
    let directory =
        std::env::temp_dir().join(format!("nabiman-key-interop-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&directory).unwrap();
    for algorithm in ["rsa", "ecdsa", "ed25519"] {
        let path = directory.join(algorithm);
        let generated = std::process::Command::new("ssh-keygen")
            .args(["-q", "-t", algorithm, "-N", "", "-f"])
            .arg(&path)
            .output()
            .unwrap();
        assert!(generated.status.success());
        let public = std::fs::read_to_string(path.with_extension("pub")).unwrap();
        let (canonical, fingerprint) = parse_key(public.trim()).unwrap();
        let reference = std::process::Command::new("ssh-keygen")
            .args(["-l", "-E", "sha256", "-f"])
            .arg(path.with_extension("pub"))
            .output()
            .unwrap();
        assert!(reference.status.success());
        let output = String::from_utf8(reference.stdout).unwrap();
        assert_eq!(output.split_whitespace().nth(1).unwrap(), fingerprint);
        assert_eq!(parse_key(&canonical).unwrap().1, fingerprint);
    }
    std::fs::remove_dir_all(directory).unwrap();
}
