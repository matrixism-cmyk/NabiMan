use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::io::{self, Read, Write};

/// A request, and every frame the helper reads.
pub const MAX_FRAME: usize = 4096;
/// A system operation's reply, which carries a command's (bounded) output; read only by the API.
pub const MAX_REPLY: usize = 64 * 1024;
pub const VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ServiceAction {
    Start,
    Stop,
    Restart,
    Enable,
    Disable,
}
impl ServiceAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Stop => "stop",
            Self::Restart => "restart",
            Self::Enable => "enable",
            Self::Disable => "disable",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    Execute { unit: String, action: ServiceAction },
    Inspect { operation_id: String },
    /// The firewall's rules as its tool prints them (ufw, firewalld, iptables).
    FirewallStatus {},
    /// Allow or deny one port or port range (`port..=port_end`).
    FirewallAdd { verdict: Verdict, protocol: Protocol, port: u16, port_end: Option<u16> },
    /// Delete rule `number` as `FirewallStatus` lists it (ufw, iptables).
    FirewallDelete { number: u32 },
    /// Refresh the package lists (`apt-get update`, `dnf makecache`).
    PackageRefresh {},
    /// Upgrade every package, or the one named, from the configured repositories.
    PackageUpgrade { package: Option<String> },
}
pub use super::system_ops::{Outcome, Protocol, SystemReply, Verdict};
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub version: u32,
    pub request_id: String,
    pub actor_id: String,
    pub expires_at: i64,
    pub operation: Operation,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Running,
    CommandSucceeded,
    CommandFailed,
    NotDispatched,
    UnknownResult,
}
impl State {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::CommandSucceeded => "command_succeeded",
            Self::CommandFailed => "command_failed",
            Self::NotDispatched => "not_dispatched",
            Self::UnknownResult => "unknown_result",
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub policy_hash: String,
    pub operation_id: String,
    pub actor_id: String,
    pub unit: String,
    pub action: ServiceAction,
    pub state: State,
    pub exit_code: Option<i32>,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Completed,
    Rejected,
    Conflict,
    NotFound,
    Expired,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Reply {
    pub version: u32,
    pub request_id: String,
    pub status: Status,
    pub receipt: Option<Receipt>,
}

pub fn is_id(id: &str) -> bool {
    uuid::Uuid::parse_str(id).is_ok_and(|value| value.to_string() == id)
}
pub fn read_frame<T: DeserializeOwned>(reader: &mut impl Read) -> io::Result<T> {
    read_frame_within(reader, MAX_FRAME)
}
pub fn read_frame_within<T: DeserializeOwned>(reader: &mut impl Read, max: usize) -> io::Result<T> {
    let mut header = [0u8; 4];
    reader.read_exact(&mut header)?;
    let len = u32::from_be_bytes(header) as usize;
    if len == 0 || len > max {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid frame size",
        ));
    }
    let mut bytes = vec![0; len];
    reader.read_exact(&mut bytes)?;
    serde_json::from_slice(&bytes)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid helper message"))
}
pub fn write_frame<T: Serialize>(writer: &mut impl Write, value: &T) -> io::Result<()> {
    write_frame_within(writer, value, MAX_FRAME)
}
pub fn write_frame_within<T: Serialize>(writer: &mut impl Write, value: &T, max: usize) -> io::Result<()> {
    let bytes = serde_json::to_vec(value).map_err(io::Error::other)?;
    if bytes.is_empty() || bytes.len() > max {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid frame size",
        ));
    }
    writer.write_all(&(bytes.len() as u32).to_be_bytes())?;
    writer.write_all(&bytes)
}
