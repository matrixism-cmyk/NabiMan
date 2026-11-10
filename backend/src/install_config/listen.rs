//! Where the server listens: `NABIMAN_LISTEN` (`listen` in the file), an
//! address with or without a port. Unset, it is the loopback address on
//! `NABIMAN_PORT` (default 8080): NabiMan speaks plain HTTP and expects a TLS
//! proxy on the same machine in front of it. Every address (`0.0.0.0`) only
//! when it is asked for in so many words.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use super::values::{env, number_in, text_in, Lookup};

/// The port when neither `NABIMAN_LISTEN` nor `NABIMAN_PORT` gives one.
pub(crate) const DEFAULT_PORT: u16 = 8080;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Listen {
    pub addr: SocketAddr,
    /// `NABIMAN_LISTEN` named a port and `NABIMAN_PORT` another (the first wins).
    pub port_overridden: bool,
}

impl Listen {
    pub fn is_loopback(&self) -> bool {
        self.addr.ip().is_loopback()
    }

    /// What startup says about an address other machines can reach: nothing
    /// for loopback. (Whether a TLS proxy is in front cannot be seen from here.)
    pub fn warning(&self) -> Option<String> {
        (!self.is_loopback()).then(|| format!(
            "WARNING: listening on {}, which other machines can reach. NabiMan speaks plain HTTP: \
             put a TLS proxy in front and firewall this port, or set listen = \"127.0.0.1:{}\" (NABIMAN_LISTEN)",
            self.addr, self.addr.port()
        ))
    }
}

fn address(text: &str) -> Option<IpAddr> {
    let bare = text.strip_prefix('[').and_then(|t| t.strip_suffix(']')).unwrap_or(text);
    if bare.eq_ignore_ascii_case("localhost") {
        return Some(IpAddr::V4(Ipv4Addr::LOCALHOST));
    }
    bare.parse().ok()
}

/// `NABIMAN_LISTEN`: `127.0.0.1:18080`, `[::1]:8080`, `0.0.0.0` (port from
/// `NABIMAN_PORT`), `localhost`. Host names other than `localhost` are refused
/// (which address a name means can change), as is anything that does not read.
pub(crate) fn listen_in(get: Lookup) -> Result<Listen, String> {
    let port: u16 = number_in(get, "NABIMAN_PORT", DEFAULT_PORT);
    let Some(text) = text_in(get, "NABIMAN_LISTEN") else {
        return Ok(Listen { addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port), port_overridden: false });
    };
    let bad = || format!("NABIMAN_LISTEN is not an address to listen on: {text:?} (e.g. 127.0.0.1:8080, [::1]:8080, 0.0.0.0)");
    if let Ok(addr) = text.parse::<SocketAddr>() {
        let port_overridden = text_in(get, "NABIMAN_PORT").is_some() && addr.port() != port;
        return Ok(Listen { addr, port_overridden });
    }
    if let Some(ip) = address(&text) {
        return Ok(Listen { addr: SocketAddr::new(ip, port), port_overridden: false });
    }
    // `localhost:8080`
    let (host, named) = text.rsplit_once(':').ok_or_else(bad)?;
    let (Some(ip), Ok(named)) = (address(host).filter(|_| host.eq_ignore_ascii_case("localhost")), named.parse::<u16>()) else {
        return Err(bad());
    };
    let port_overridden = text_in(get, "NABIMAN_PORT").is_some() && named != port;
    Ok(Listen { addr: SocketAddr::new(ip, named), port_overridden })
}

pub(crate) fn listen() -> Result<Listen, String> {
    listen_in(&env)
}

/// Where to bind, said at startup (with the warning for a reachable address).
pub(crate) fn at_startup() -> std::io::Result<Listen> {
    let listen = listen().map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
    if listen.port_overridden {
        eprintln!("Settings: NABIMAN_LISTEN names port {}; NABIMAN_PORT is not used", listen.addr.port());
    }
    if let Some(warning) = listen.warning() {
        eprintln!("{warning}");
    }
    Ok(listen)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with(pairs: &'static [(&'static str, &'static str)]) -> Result<Listen, String> {
        listen_in(&move |name: &str| pairs.iter().find(|(k, _)| *k == name).map(|(_, v)| v.to_string()))
    }
    fn addr(pairs: &'static [(&'static str, &'static str)]) -> String {
        with(pairs).unwrap().addr.to_string()
    }

    #[test]
    fn unset_is_loopback_on_the_port() {
        assert_eq!(addr(&[]), "127.0.0.1:8080");
        assert_eq!(addr(&[("NABIMAN_PORT", "18080")]), "127.0.0.1:18080");
        assert_eq!(addr(&[("NABIMAN_LISTEN", "  "), ("NABIMAN_PORT", "18080")]), "127.0.0.1:18080");
        assert!(with(&[]).unwrap().is_loopback() && with(&[]).unwrap().warning().is_none());
    }

    #[test]
    fn an_address_takes_the_port_setting_and_a_full_one_its_own() {
        assert_eq!(addr(&[("NABIMAN_LISTEN", "0.0.0.0"), ("NABIMAN_PORT", "18080")]), "0.0.0.0:18080");
        assert_eq!(addr(&[("NABIMAN_LISTEN", "::")]), "[::]:8080");
        assert_eq!(addr(&[("NABIMAN_LISTEN", "[::1]")]), "[::1]:8080");
        assert_eq!(addr(&[("NABIMAN_LISTEN", "10.0.0.5:9000")]), "10.0.0.5:9000");
        assert_eq!(addr(&[("NABIMAN_LISTEN", "[::1]:9000")]), "[::1]:9000");
        assert_eq!(addr(&[("NABIMAN_LISTEN", "localhost")]), "127.0.0.1:8080");
        assert_eq!(addr(&[("NABIMAN_LISTEN", "localhost:9000")]), "127.0.0.1:9000");
        // A port in NABIMAN_LISTEN wins over NABIMAN_PORT, and startup says so.
        let both = with(&[("NABIMAN_LISTEN", "127.0.0.1:9000"), ("NABIMAN_PORT", "18080")]).unwrap();
        assert_eq!((both.addr.port(), both.port_overridden), (9000, true));
        let same = with(&[("NABIMAN_LISTEN", "127.0.0.1:18080"), ("NABIMAN_PORT", "18080")]).unwrap();
        assert!(!same.port_overridden);
    }

    #[test]
    fn a_reachable_address_is_warned_about_and_loopback_is_not() {
        for open in ["0.0.0.0", "::", "192.168.1.10:8080"] {
            let pairs: &'static [(&'static str, &'static str)] = Box::leak(Box::new([("NABIMAN_LISTEN", open)]));
            let warning = with(pairs).unwrap().warning().unwrap_or_default();
            assert!(warning.contains("TLS proxy"), "{open}: {warning}");
        }
        for closed in ["127.0.0.1", "127.0.0.2:80", "::1", "localhost"] {
            let pairs: &'static [(&'static str, &'static str)] = Box::leak(Box::new([("NABIMAN_LISTEN", closed)]));
            assert!(with(pairs).unwrap().warning().is_none(), "{closed}");
        }
    }

    #[test]
    fn what_does_not_read_is_refused_rather_than_guessed() {
        for bad in ["man.bebo.kr", "man.bebo.kr:80", "0.0.0.0:http", "1.2.3.4:70000", ":8080", "*", "0.0.0.0:-1"] {
            let pairs: &'static [(&'static str, &'static str)] = Box::leak(Box::new([("NABIMAN_LISTEN", bad)]));
            assert!(with(pairs).is_err(), "{bad}");
        }
        // A port setting that does not read stays the default, as before.
        assert_eq!(addr(&[("NABIMAN_PORT", "eighty")]), "127.0.0.1:8080");
    }
}
