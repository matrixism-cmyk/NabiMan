use super::client::upstream;
use crate::mec::services::ServiceResult;
use async_trait::async_trait;
use russh::client::{self, Config, Handle, Handler};
use russh::{kex, Channel, ChannelMsg, Preferred};
use russh_keys::key;
use std::borrow::Cow;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{timeout, Instant};

/// AXGATE's SSH stack only negotiates legacy algorithms — we extend russh's
/// safe defaults so modern endpoints still work elsewhere.
fn legacy_preferred() -> Preferred {
    let mut kex_list = Preferred::DEFAULT.kex.to_vec();
    for name in [kex::DH_G14_SHA1, kex::DH_G1_SHA1] {
        if !kex_list.contains(&name) {
            kex_list.push(name);
        }
    }
    let mut key_list = Preferred::DEFAULT.key.to_vec();
    if !key_list.contains(&key::SSH_RSA) {
        key_list.push(key::SSH_RSA);
    }
    Preferred {
        kex: Cow::Owned(kex_list),
        key: Cow::Owned(key_list),
        cipher: Preferred::DEFAULT.cipher.clone(),
        mac: Preferred::DEFAULT.mac.clone(),
        compression: Preferred::DEFAULT.compression.clone(),
    }
}

pub struct AxgateSession {
    handle: Handle<SshClient>,
    password: String,
}

struct SshClient;

#[async_trait]
impl Handler for SshClient {
    type Error = russh::Error;
    async fn check_server_key(
        &mut self,
        _server_public_key: &russh_keys::key::PublicKey,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

impl AxgateSession {
    pub async fn connect(
        host: &str,
        port: u16,
        username: &str,
        password: &str,
        timeout_dur: Duration,
    ) -> ServiceResult<Self> {
        let config = Arc::new(Config {
            preferred: legacy_preferred(),
            ..Default::default()
        });
        let mut handle = timeout(
            timeout_dur,
            client::connect(config, (host, port), SshClient),
        )
        .await
        .map_err(|_| upstream(format!("connect timeout: {}:{}", host, port)))?
        .map_err(upstream)?;

        let ok = timeout(
            timeout_dur,
            handle.authenticate_password(username, password),
        )
        .await
        .map_err(|_| upstream("auth timeout"))?
        .map_err(upstream)?;
        if !ok {
            return Err(upstream("authentication failed"));
        }
        Ok(Self {
            handle,
            password: password.to_string(),
        })
    }

    /// AXGATE CLI flow (verified 2026-04-21):
    ///  1. SSH auth succeeds → server immediately emits `Password: ` prompt
    ///  2. Client must send CLI password (same value). There is NO `enable`
    ///     step — the session is already privileged after this second auth.
    ///  3. Send `terminal length 0`, then `show running-config`.
    ///
    /// **Important**: sending anything other than the password at step 2
    /// triggers AXGATE's lockout (`% You are blocked for 600 seconds!`).
    pub async fn fetch_running_config(&mut self) -> ServiceResult<String> {
        self.run_commands(&["show running-config"]).await
    }

    pub async fn run_commands(&mut self, commands: &[&str]) -> ServiceResult<String> {
        let mut channel = self
            .handle
            .channel_open_session()
            .await
            .map_err(upstream)?;
        channel
            .request_pty(true, "vt100", 200, 50, 0, 0, &[])
            .await
            .map_err(upstream)?;
        channel.request_shell(true).await.map_err(upstream)?;

        let idle = Duration::from_secs(6);
        let overall = Duration::from_secs(120);
        let start = Instant::now();
        let debug = std::env::var("NABIMAN_MEC_AXGATE_DEBUG").is_ok();

        // Step 1: wait for the CLI-level Password prompt.
        let greet = read_until_idle(&mut channel, idle, start, overall).await?;
        if debug {
            eprintln!("[axgate:greet] {:?}", greet.chars().take(200).collect::<String>());
        }

        if greet.contains("blocked for") {
            return Err(upstream(format!(
                "AXGATE has rate-locked this client: {}",
                greet.trim()
            )));
        }
        if !greet.contains("Password") {
            return Err(upstream(format!(
                "unexpected AXGATE greeting (no Password prompt): {:?}",
                greet
            )));
        }

        // Step 2: CLI password (same as SSH password per handoff §2-3).
        write_line(&mut channel, &self.password).await?;
        let after_pw = read_until_idle(&mut channel, idle, start, overall).await?;
        if debug {
            eprintln!(
                "[axgate:after_pw] {:?}",
                after_pw.chars().take(200).collect::<String>()
            );
        }
        if after_pw.contains("blocked for") || after_pw.contains("Username:") {
            return Err(upstream(format!(
                "AXGATE CLI authentication failed: {}",
                after_pw.trim()
            )));
        }

        // Step 3: pagination off.
        write_line(&mut channel, "terminal length 0").await?;
        let _ = read_until_idle(&mut channel, idle, start, overall).await?;

        // Step 4: run commands.
        let mut combined = String::new();
        for c in commands {
            write_line(&mut channel, c).await?;
            combined.push_str(&read_until_idle(&mut channel, idle, start, overall).await?);
        }
        write_line(&mut channel, "exit").await?;
        let _ = channel.close().await;
        Ok(combined)
    }

    #[allow(dead_code)]
    pub async fn run_command(&mut self, command: &str) -> ServiceResult<String> {
        self.run_commands(&[command]).await
    }
}

async fn write_line(channel: &mut Channel<client::Msg>, line: &str) -> ServiceResult<()> {
    let mut buf = Vec::with_capacity(line.len() + 1);
    buf.extend_from_slice(line.as_bytes());
    buf.push(b'\n');
    channel.data(&buf[..]).await.map_err(upstream)?;
    Ok(())
}

/// Reads from the channel, returning once no new data arrives for `idle`,
/// or the overall `total` elapses. AXGATE shells don't expose a consistent
/// end-of-command marker, so we rely on idle timeout as the boundary.
///
/// Also auto-advances the `--More--` pagination prompt by sending SPACE,
/// in case `terminal length 0` wasn't honored.
async fn read_until_idle(
    channel: &mut Channel<client::Msg>,
    idle: Duration,
    start: Instant,
    total: Duration,
) -> ServiceResult<String> {
    let mut buf = String::new();
    loop {
        let remaining_total = total
            .checked_sub(start.elapsed())
            .unwrap_or(Duration::from_millis(0));
        if remaining_total.is_zero() {
            return Err(upstream("AXGATE session overall timeout"));
        }
        let wait = idle.min(remaining_total);
        match timeout(wait, channel.wait()).await {
            Ok(Some(ChannelMsg::Data { data })) => {
                let chunk = String::from_utf8_lossy(&data);
                buf.push_str(&chunk);
                let tail = buf
                    .trim_end_matches(|c: char| c.is_whitespace() || c.is_control());
                if tail.ends_with("--More--") || tail.ends_with("---- More ----") {
                    // advance paginator
                    let _ = channel.data(&b" "[..]).await;
                }
            }
            Ok(Some(ChannelMsg::ExtendedData { data, .. })) => {
                buf.push_str(&String::from_utf8_lossy(&data));
            }
            Ok(Some(ChannelMsg::Eof)) | Ok(Some(ChannelMsg::Close)) | Ok(None) => {
                return Ok(buf);
            }
            Ok(Some(_)) => {}
            Err(_) => return Ok(buf), // idle timeout → command complete
        }
    }
}
