//! Which peers may say where a request really came from.
//!
//! NabiMan normally sits behind nginx on the same machine, so every request's
//! peer is 127.0.0.1 and the person's address is in `X-Forwarded-For` (or
//! `X-Real-IP`). Those headers are only believed when the peer is a trusted
//! proxy: `NABIMAN_TRUSTED_PROXIES`, a comma-separated list of addresses or
//! networks, default `127.0.0.1,::1`. Setting it to an empty value trusts
//! nobody. The client is then the right-most address in `X-Forwarded-For`
//! that is not itself a trusted proxy (everything left of it may be
//! anything the browser sent).

use std::net::IpAddr;
use std::sync::OnceLock;

use actix_web::http::header::HeaderMap;
use actix_web::HttpRequest;

pub(crate) type Network = (IpAddr, u8);

const DEFAULT: &str = "127.0.0.1,::1";

/// The trusted networks, read once; entries that do not parse are left out
/// (and said so on stderr), so a typo trusts less, never more.
pub(crate) fn trusted() -> &'static [Network] {
    static TRUSTED: OnceLock<Vec<Network>> = OnceLock::new();
    TRUSTED.get_or_init(|| parse(&std::env::var("NABIMAN_TRUSTED_PROXIES").unwrap_or_else(|_| DEFAULT.into())))
}

pub(crate) fn parse(list: &str) -> Vec<Network> {
    list.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .filter_map(|s| {
            let net = crate::access_rules::parse_network(s);
            if net.is_none() {
                eprintln!("NABIMAN_TRUSTED_PROXIES: '{s}' is not an address or network; ignored");
            }
            net
        })
        .collect()
}

fn is_trusted(ip: IpAddr, trusted: &[Network]) -> bool {
    trusted.iter().any(|net| crate::access_rules::in_network(ip, *net))
}

/// The client's address, given the peer and the headers it sent.
pub(crate) fn client_ip_from(peer: IpAddr, headers: &HeaderMap, trusted: &[Network]) -> IpAddr {
    if !is_trusted(peer, trusted) {
        return peer;
    }
    let chain: Vec<IpAddr> = headers
        .get_all("x-forwarded-for")
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .filter_map(|a| a.trim().parse().ok())
        .collect();
    if let Some(client) = chain.iter().rev().find(|ip| !is_trusted(**ip, trusted)) {
        return *client;
    }
    if let Some(first) = chain.first() {
        // Every hop is a trusted proxy: the left-most is where it started.
        return *first;
    }
    headers
        .get("x-real-ip")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(peer)
}

/// The client's address for `req` (see the module notes).
pub(crate) fn client_ip(req: &HttpRequest) -> Option<IpAddr> {
    let peer = req.peer_addr()?.ip();
    Some(client_ip_from(peer, req.headers(), trusted()))
}

/// Whether a trusted proxy says the request reached it over HTTPS
/// (`X-Forwarded-Proto: https`).
pub(crate) fn forwarded_https(peer: Option<IpAddr>, headers: &HeaderMap, trusted: &[Network]) -> bool {
    peer.is_some_and(|p| is_trusted(p, trusted))
        && headers
            .get("x-forwarded-proto")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split(',').next())
            .is_some_and(|proto| proto.trim().eq_ignore_ascii_case("https"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::http::header::{HeaderName, HeaderValue};

    fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for (k, v) in pairs {
            h.append(HeaderName::from_bytes(k.as_bytes()).unwrap(), HeaderValue::from_str(v).unwrap());
        }
        h
    }
    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn headers_from_an_untrusted_peer_are_ignored() {
        let t = parse(DEFAULT);
        let h = headers(&[("x-forwarded-for", "203.0.113.9"), ("x-real-ip", "203.0.113.9")]);
        assert_eq!(client_ip_from(ip("198.51.100.7"), &h, &t), ip("198.51.100.7"));
        assert_eq!(client_ip_from(ip("127.0.0.2"), &h, &t), ip("127.0.0.2"), "only 127.0.0.1 by default");
    }

    #[test]
    fn the_right_most_untrusted_hop_is_the_client() {
        let t = parse("127.0.0.1, ::1, 10.0.0.0/8");
        // The browser forged the left entry; nginx appended the real peer.
        let h = headers(&[("x-forwarded-for", "8.8.8.8, 192.168.1.5")]);
        assert_eq!(client_ip_from(ip("127.0.0.1"), &h, &t), ip("192.168.1.5"));
        // A second trusted proxy in front of nginx is skipped.
        let h = headers(&[("x-forwarded-for", "1.2.3.4"), ("x-forwarded-for", "203.0.113.5, 10.1.2.3")]);
        assert_eq!(client_ip_from(ip("::1"), &h, &t), ip("203.0.113.5"));
        // All hops trusted: the left-most one started it.
        let h = headers(&[("x-forwarded-for", "10.0.0.4, 10.0.0.5")]);
        assert_eq!(client_ip_from(ip("127.0.0.1"), &h, &t), ip("10.0.0.4"));
    }

    #[test]
    fn x_real_ip_is_the_fallback_and_nothing_trusts_nobody() {
        let t = parse(DEFAULT);
        let h = headers(&[("x-real-ip", "203.0.113.8")]);
        assert_eq!(client_ip_from(ip("127.0.0.1"), &h, &t), ip("203.0.113.8"));
        assert_eq!(client_ip_from(ip("127.0.0.1"), &headers(&[]), &t), ip("127.0.0.1"));
        let none = parse("");
        assert_eq!(client_ip_from(ip("127.0.0.1"), &h, &none), ip("127.0.0.1"));
        assert_eq!(parse("bogus, 10.0.0.0/33, 192.0.2.1").len(), 1, "typos trust less");
    }

    #[test]
    fn https_is_believed_only_from_a_trusted_proxy() {
        let t = parse(DEFAULT);
        let h = headers(&[("x-forwarded-proto", "https")]);
        assert!(forwarded_https(Some(ip("127.0.0.1")), &h, &t));
        assert!(!forwarded_https(Some(ip("203.0.113.1")), &h, &t));
        assert!(!forwarded_https(Some(ip("127.0.0.1")), &headers(&[("x-forwarded-proto", "http")]), &t));
        assert!(!forwarded_https(None, &h, &t));
    }
}
