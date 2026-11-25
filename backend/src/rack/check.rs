//! How a device that is not a Kubernetes node is asked whether it answers:
//! the layout file's optional `check` field, and which addresses a check
//! may be sent to at all.

use std::net::Ipv4Addr;

use serde::{Serialize, Serializer};

/// `check` in the layout file. Without the field a device that has an
/// address is pinged.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub(crate) enum Check {
    /// One ICMP echo.
    #[default]
    Ping,
    /// One TCP connect to this port, for a device that drops ICMP.
    Tcp(u16),
    /// Never asked.
    None,
}

impl Check {
    /// The field's text, checked. `at` says where it is, for the fault.
    pub(crate) fn parse(at: &str, value: Option<&str>) -> Result<Self, String> {
        let Some(value) = value.map(str::trim).filter(|v| !v.is_empty()) else { return Ok(Check::Ping) };
        let lower = value.to_ascii_lowercase();
        let port = |text: &str| (!text.is_empty() && text.len() <= 5 && text.bytes().all(|b| b.is_ascii_digit())).then(|| text.parse::<u16>().ok().filter(|p| *p > 0)).flatten();
        match lower.as_str() {
            "ping" => Ok(Check::Ping),
            "none" => Ok(Check::None),
            other => match other.strip_prefix("tcp:") {
                Some(text) => port(text).map(Check::Tcp).ok_or_else(|| format!("{at}: the port in tcp:<port> must be 1–65535")),
                None => Err(format!("{at}: must be ping, tcp:<port> or none")),
            },
        }
    }

    /// As the file writes it: `ping`, `tcp:443`, `none`.
    pub(crate) fn name(&self) -> String {
        match self {
            Check::Ping => "ping".into(),
            Check::Tcp(port) => format!("tcp:{port}"),
            Check::None => "none".into(),
        }
    }
}

impl Serialize for Check {
    fn serialize<S: Serializer>(&self, to: S) -> Result<S::Ok, S::Error> {
        to.serialize_str(&self.name())
    }
}

/// The address a check may go to, from the layout's `ip` (already an IPv4
/// address in plain dotted form, or the file was refused). Private, loopback,
/// link-local and public addresses are all somebody's management network;
/// "no address" (0.x), broadcast and multicast are not a machine.
pub(crate) fn target_address(ip: &str) -> Option<Ipv4Addr> {
    let address: Ipv4Addr = ip.parse().ok()?;
    let plain = address.to_string() == ip && address.octets()[0] != 0 && !address.is_broadcast() && !address.is_multicast();
    plain.then_some(address)
}
