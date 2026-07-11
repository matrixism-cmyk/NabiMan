//! Best-effort early warning for kubeconfig token expiry.
//!
//! Repeated MEC outages have traced back to an expired Rancher kubeconfig token
//! (a raw 401 on every call). When the token is a JWT (service-account / OIDC),
//! we can read its `exp` claim WITHOUT the signing key and warn N days ahead.
//! Opaque Rancher tokens (`kubeconfig-user-xxx:yyy`) carry no expiry we can see,
//! so we honestly report "unknown" (None) rather than guessing.

/// Days until the kubeconfig token expires, from `NABIMAN_MEC_KUBECONFIG`.
/// `None` when the path is unset/unreadable, the token is missing, or the token
/// is opaque (not a JWT) — i.e. whenever we genuinely cannot tell.
pub fn days_until_expiry_from_env() -> Option<i64> {
    let path = std::env::var("NABIMAN_MEC_KUBECONFIG").ok()?;
    let yaml = std::fs::read_to_string(path).ok()?;
    let token = extract_token(&yaml)?;
    let exp = jwt_exp(&token)?;
    let now = chrono::Utc::now().timestamp();
    Some((exp - now).div_euclid(86_400))
}

/// Pull the first `token:` value out of a kubeconfig YAML document.
fn extract_token(yaml: &str) -> Option<String> {
    for line in yaml.lines() {
        let t = line.trim_start();
        if let Some(rest) = t.strip_prefix("token:") {
            let v = rest.trim().trim_matches('"').trim_matches('\'');
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

/// Read the `exp` (seconds since epoch) from a JWT without verifying its
/// signature. `None` if the string isn't a 3-part JWT or has no numeric `exp`.
fn jwt_exp(token: &str) -> Option<i64> {
    let payload = token.split('.').nth(1)?;
    let bytes = b64url_decode(payload)?;
    let json: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    json.get("exp")?.as_i64()
}

/// Minimal base64url (RFC 4648 §5, no padding) decoder — avoids a new crate dep.
fn b64url_decode(s: &str) -> Option<Vec<u8>> {
    fn val(c: u8) -> Option<u32> {
        match c {
            b'A'..=b'Z' => Some((c - b'A') as u32),
            b'a'..=b'z' => Some((c - b'a' + 26) as u32),
            b'0'..=b'9' => Some((c - b'0' + 52) as u32),
            b'-' => Some(62),
            b'_' => Some(63),
            _ => None,
        }
    }
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut acc: u32 = 0;
    let mut bits = 0;
    for &c in s.as_bytes() {
        let v = val(c)?;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn b64url(bytes: &[u8]) -> String {
        const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
        let mut out = String::new();
        for chunk in bytes.chunks(3) {
            let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
            out.push(T[(b[0] >> 2) as usize] as char);
            out.push(T[(((b[0] & 3) << 4) | (b[1] >> 4)) as usize] as char);
            if chunk.len() > 1 { out.push(T[(((b[1] & 15) << 2) | (b[2] >> 6)) as usize] as char); }
            if chunk.len() > 2 { out.push(T[(b[2] & 63) as usize] as char); }
        }
        out
    }

    fn make_jwt(exp: i64) -> String {
        let header = b64url(br#"{"alg":"RS256","typ":"JWT"}"#);
        let payload = b64url(json!({ "exp": exp, "sub": "svc" }).to_string().as_bytes());
        format!("{}.{}.{}", header, payload, "sigsigsig")
    }

    #[test]
    fn extracts_token_from_kubeconfig_yaml() {
        let yaml = "users:\n- name: u\n  user:\n    token: abc.def.ghi\n";
        assert_eq!(extract_token(yaml), Some("abc.def.ghi".to_string()));
    }

    #[test]
    fn extract_token_none_when_absent() {
        assert_eq!(extract_token("clusters: []\n"), None);
    }

    #[test]
    fn reads_exp_from_jwt() {
        let jwt = make_jwt(1_900_000_000);
        assert_eq!(jwt_exp(&jwt), Some(1_900_000_000));
    }

    #[test]
    fn opaque_token_has_no_exp() {
        // Rancher-style opaque token is not a JWT.
        assert_eq!(jwt_exp("kubeconfig-user-abc123:xyz789"), None);
    }

    #[test]
    fn b64url_roundtrip() {
        let data = b"hello, mec";
        assert_eq!(b64url_decode(&b64url(data)).as_deref(), Some(&data[..]));
    }
}
