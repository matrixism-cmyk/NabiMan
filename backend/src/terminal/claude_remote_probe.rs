//! The probe itself: the script sent over a tile's shared connection, run
//! there, and what it sends back read into sessions and messages.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use super::{mux_path, RemoteClaude};

const LINUX_PROBE: &str = include_str!("claude_remote_probe.sh");
const WINDOWS_PROBE: &str = include_str!("claude_remote_probe.ps1");
const PROBE_TIMEOUT: Duration = Duration::from_secs(25);
const MAX_OUTPUT: usize = 4 << 20;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Os {
    Unix,
    Windows,
}

pub(super) fn run(session_id: &str, target: &str, os: Os, offsets: &HashMap<String, u64>) -> Option<String> {
    let (remote, input) = match os {
        Os::Unix => {
            let lines: String = offsets
                .iter()
                .filter(|(k, _)| k.chars().all(|c| c.is_ascii_hexdigit() || c == '-'))
                .map(|(k, v)| format!("{k} {v}\n"))
                .collect();
            ("sh -s".to_string(), format!("NABIMAN_OFFSETS='{lines}'\n{LINUX_PROBE}"))
        }
        Os::Windows => {
            let table: String = offsets
                .iter()
                .filter(|(k, _)| k.chars().all(|c| c.is_ascii_hexdigit() || c == '-'))
                .map(|(k, v)| format!("'{k}'={v};"))
                .collect();
            // A tiny loader on the command line; the probe itself comes on stdin
            // (a Windows command line is limited to 8191 characters).
            let loader = "$s=[Console]::In.ReadToEnd(); Invoke-Expression $s";
            let encoded = b64_encode(&loader.encode_utf16().flat_map(|u| u.to_le_bytes()).collect::<Vec<u8>>());
            (
                format!("powershell -NoProfile -NonInteractive -EncodedCommand {encoded}"),
                format!("$offsets=@{{{table}}}\n{WINDOWS_PROBE}"),
            )
        }
    };
    let mut child = Command::new("ssh")
        .arg("-S")
        .arg(mux_path(session_id))
        .args(["-o", "ControlMaster=no", "-o", "BatchMode=yes", "-T", "--", target, &remote])
        .env_remove("TMUX")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(input.as_bytes());
    }
    let mut stdout = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = (&mut stdout).take(MAX_OUTPUT as u64).read_to_end(&mut buf);
        buf
    });
    let until = Instant::now() + PROBE_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < until => std::thread::sleep(Duration::from_millis(100)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
    let out = reader.join().ok()?;
    Some(String::from_utf8_lossy(&out).into_owned())
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub(super) fn b64_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16) | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8) | u32::from(*chunk.get(2).unwrap_or(&0));
        for (i, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            out.push(if i <= chunk.len() { B64[(n >> shift) as usize & 63] as char } else { '=' });
        }
    }
    out
}

fn b64_decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let (mut acc, mut bits) = (0u32, 0u32);
    for c in text.bytes().filter(|c| !c.is_ascii_whitespace()) {
        if c == b'=' {
            break;
        }
        acc = (acc << 6) | B64.iter().position(|&x| x == c)? as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

#[derive(Debug, Default)]
pub(super) struct Found {
    pub(super) peers: Vec<RemoteClaude>,
    /// Agent kinds running under this tile's connection.
    pub(super) agents: Vec<String>,
    pub(super) sizes: Vec<(String, u64)>,
    pub(super) messages: Vec<(String, String, u64, String, String)>,
}

fn clip(s: &str, n: usize) -> String {
    s.chars().filter(|c| !c.is_control()).take(n).collect()
}

/// Reads a probe's output (see claude_remote_probe.sh).
pub(super) fn parse(out: &str, server_id: &str, tmux_name: &str) -> Found {
    let mut found = Found::default();
    for line in out.lines() {
        let f: Vec<&str> = line.trim_end_matches('\r').split('\t').collect();
        match f.as_slice() {
            ["R", under, b64] => {
                let Some(raw) = b64_decode(b64.trim()) else { continue };
                let Ok(v) = serde_json::from_slice::<serde_json::Value>(&raw) else { continue };
                let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
                let session_id = s("sessionId");
                if session_id.is_empty() || !session_id.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
                    continue;
                }
                let bridge = Some(s("bridgeSessionId")).filter(|b| !b.is_empty() && b.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
                found.peers.push(RemoteClaude {
                    server_id: server_id.to_string(),
                    name: clip(&s("name"), 80),
                    cwd: clip(&s("cwd"), 300),
                    busy: s("status") == "busy",
                    since: v.get("statusUpdatedAt").and_then(serde_json::Value::as_u64).map(|ms| ms / 1000),
                    model: None,
                    version: clip(&s("version"), 20),
                    sock: clip(&s("messagingSocketPath"), 300),
                    bridge,
                    session_id,
                    pid: v.get("pid").and_then(serde_json::Value::as_u64).and_then(|p| u32::try_from(p).ok()).unwrap_or(0),
                    tile: (*under == "1").then(|| tmux_name.to_string()),
                });
            }
            ["U", pid, user] => {
                // Someone else's Claude under this tile (after `su -`): its
                // record is not readable, so no address; it is shown as running.
                let (Ok(pid), user) = (pid.trim().parse::<u32>(), user.trim()) else { continue };
                if user.is_empty() || user.len() > 32 || !user.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c)) {
                    continue;
                }
                found.peers.push(RemoteClaude {
                    server_id: server_id.to_string(),
                    name: format!("Claude ({user})"),
                    cwd: String::new(),
                    busy: false,
                    since: None,
                    model: None,
                    version: String::new(),
                    sock: String::new(),
                    bridge: None,
                    session_id: String::new(),
                    pid,
                    tile: Some(tmux_name.to_string()),
                });
            }
            ["A", kind] => found.agents.push(kind.trim().to_string()),
            ["V", sid, model] => {
                let model = model.trim();
                if super::super::claude_peers::model_ok(model) {
                    for p in found.peers.iter_mut().filter(|p| p.session_id == *sid) {
                        p.model = Some(model.to_string());
                    }
                }
            }
            ["O", sid, size] => {
                if let Ok(n) = size.trim().parse() {
                    found.sizes.push((sid.to_string(), n));
                }
            }
            ["M", sid, from, at, id, rest @ ..] => {
                let Ok(at) = chrono::DateTime::parse_from_rfc3339(at) else { continue };
                let Ok(at) = u64::try_from(at.timestamp()) else { continue };
                // The first line it said, base64 (it may hold tabs or any script).
                let said = rest
                    .first()
                    .and_then(|b| b64_decode(b.trim()))
                    .map(|raw| String::from_utf8_lossy(&raw).replace("\\\"", "\"").replace("\\\\", "\\"))
                    .map(|t| super::super::claude_links::gist(&t))
                    .unwrap_or_default();
                found.messages.push((sid.to_string(), clip(from, 400), at, clip(id, 80), said));
            }
            _ => {}
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_probe_s_answer_becomes_sessions_and_messages() {
        let rec = r#"{"pid":7,"sessionId":"0bb8-ab5","cwd":"C:\\Users\\A\\책","name":"x-f9","status":"busy","messagingSocketPath":"\\\\.\\pipe\\LOCAL\\cc-msg-1","bridgeSessionId":"session_01AB"}"#;
        let other = r#"{"pid":8,"sessionId":"77","cwd":"/w","name":"y","status":"idle","messagingSocketPath":"/tmp/cc-socks/8.sock"}"#;
        let b = |s: &str| b64_encode(s.as_bytes());
        let out = format!(
            "C\t11680\r\nR\t1\t{}\r\nR\t0\t{}\nO\t0bb8-ab5\t1234\nM\t0bb8-ab5\tbridge:session_01CD\t2026-11-05T14:56:18.111Z\tm1\t{}\nM\t77\tuds:/tmp/cc-socks/9.sock\tnot a time\tm2\nnoise\n",
            b(rec),
            b(other),
            b("배포 확인해 줘")
        );
        let got = parse(&out, "srv", "nb_admin_pty_1");
        assert_eq!(got.peers.len(), 2);
        let first = &got.peers[0];
        assert_eq!((first.name.as_str(), first.cwd.as_str(), first.busy), ("x-f9", "C:\\Users\\A\\책", true));
        assert_eq!(first.bridge.as_deref(), Some("session_01AB"));
        assert_eq!(first.tile.as_deref(), Some("nb_admin_pty_1"));
        assert_eq!(got.peers[1].tile, None, "not under this tile's connection");
        assert_eq!(got.sizes, vec![("0bb8-ab5".to_string(), 1234)]);
        assert_eq!(got.messages, vec![("0bb8-ab5".to_string(), "bridge:session_01CD".to_string(), 1793890578, "m1".to_string(), "배포 확인해 줘".to_string())]);
    }

    #[test]
    fn someone_else_s_claude_under_the_tile_shows_without_an_address() {
        let got = parse("C\t9\nU\t1855021\tmaster\nU\tx\troot\nU\t5\tro ot\n", "spark", "nb_c");
        assert_eq!(got.peers.len(), 1);
        let p = &got.peers[0];
        assert_eq!((p.name.as_str(), p.pid, p.tile.as_deref(), p.sock.as_str()), ("Claude (master)", 1855021, Some("nb_c"), ""));
    }

    #[test]
    fn base64_both_ways() {
        for s in ["", "a", "ab", "abc", "책 C:\\x", "{\"pid\":1}"] {
            assert_eq!(b64_decode(&b64_encode(s.as_bytes())).unwrap(), s.as_bytes());
        }
        assert_eq!(b64_encode(b"Man"), "TWFu");
        assert_eq!(b64_encode(b"Ma"), "TWE=");
        assert!(b64_decode("@@").is_none());
    }

    #[test]
    fn the_unix_probe_finds_its_own_records_and_messages_only() {
        // Run the real probe against a made-up home.
        let home = std::env::temp_dir().join(format!("nabiman-probe-{}", uuid::Uuid::new_v4()));
        let sessions = home.join(".claude/sessions");
        let project = home.join(".claude/projects/-w");
        std::fs::create_dir_all(&sessions).unwrap();
        std::fs::create_dir_all(&project).unwrap();
        let me = std::process::id();
        let start = std::fs::read_to_string(format!("/proc/{me}/stat")).unwrap();
        let start = start[start.rfind(')').unwrap() + 2..].split_whitespace().nth(19).unwrap().to_string();
        std::fs::write(sessions.join("a.json"), format!(r#"{{"pid":{me},"sessionId":"abc-1","procStart":"{start}","name":"live","status":"idle","cwd":"/w"}}"#)).unwrap();
        std::fs::write(sessions.join("b.json"), r#"{"pid":999999,"sessionId":"dead","procStart":"1","name":"gone"}"#).unwrap();
        let peer = r#"{"type":"user","message":{"content":"Another Claude session sent a message:\n<cross-session-message from=\"uds:/tmp/cc-socks/5.sock\">\nCheck the build, then report.\nhello \"from\":\"spoof\"\n</cross-session-message>"},"timestamp":"2026-11-05T14:56:18.111Z","origin":{"kind":"peer","from":"uds:/tmp/cc-socks/5.sock","msg_id":"m9"}}"#;
        std::fs::write(project.join("abc-1.jsonl"), format!("{{\"type\":\"user\",\"text\":\"secret\"}}\n{peer}\n")).unwrap();
        let out = Command::new("sh").arg("-s").env("HOME", &home).env("NABIMAN_OFFSETS", "").stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().and_then(|mut c| {
            c.stdin.take().unwrap().write_all(LINUX_PROBE.as_bytes())?;
            c.wait_with_output()
        }).unwrap();
        let text = String::from_utf8_lossy(&out.stdout).to_string();
        let _ = std::fs::remove_dir_all(&home);
        let got = parse(&text, "srv", "t");
        assert_eq!(got.peers.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), vec!["live"], "{text}");
        assert_eq!(got.messages.len(), 1, "{text}");
        assert_eq!(got.messages[0].1, "uds:/tmp/cc-socks/5.sock");
        assert_eq!(got.messages[0].4, "Check the build, then report.");
        assert!(!text.contains("secret") && !text.contains("hello"), "only the first line leaves: {text}");
    }
}
