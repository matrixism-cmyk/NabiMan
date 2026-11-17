//! Which site this install's passkeys belong to: `NABIMAN_PUBLIC_ORIGIN`.
//!
//! A passkey is bound to a relying-party id (a domain) and is only accepted
//! from the origins the server names. Both come from the setting — a
//! comma-separated list of origins, the first being the main one, whose host
//! is the relying-party id — and never from a request's `Host` header, which
//! whoever sends the request chooses. Unset or wrong: passkeys are off.

use webauthn_rs::prelude::{Url, Webauthn, WebauthnBuilder};

pub(super) const SETTING: &str = "NABIMAN_PUBLIC_ORIGIN";

/// `https://host[:port]` and nothing else; `http` only for `localhost`,
/// which browsers treat as a secure context (for trying it out).
fn origin(text: &str) -> Result<Url, String> {
    let url = Url::parse(text).map_err(|_| format!("{SETTING}: {text} is not an origin (https://name)"))?;
    let host = url.domain().ok_or_else(|| {
        format!("{SETTING}: {text} has no domain name; passkeys cannot belong to an IP address (use a name, or http://localhost:PORT to try them)")
    })?;
    let bare = url.path() == "/" && url.query().is_none() && url.fragment().is_none() && url.username().is_empty() && url.password().is_none();
    if !bare {
        return Err(format!("{SETTING}: {text} must be an origin only (scheme, name and port)"));
    }
    let local = host == "localhost" || host.ends_with(".localhost");
    match url.scheme() {
        "https" => Ok(url),
        "http" if local => Ok(url),
        _ => Err(format!("{SETTING}: {text} must be https (http is accepted for localhost only)")),
    }
}

/// The relying party for `setting`, or why there is none.
pub(super) fn engine(setting: Option<&str>, product: &str) -> Result<Webauthn, String> {
    let listed: Vec<&str> = setting.unwrap_or("").split(',').map(str::trim).filter(|o| !o.is_empty()).collect();
    let Some((first, others)) = listed.split_first() else {
        return Err(format!("{SETTING} is not set"));
    };
    let main = origin(first)?;
    let rp_id = main.domain().unwrap_or_default().to_string();
    let mut builder = WebauthnBuilder::new(&rp_id, &main).map_err(|e| format!("{SETTING}: {e}"))?.rp_name(product);
    for other in others {
        let url = origin(other)?;
        let host = url.domain().unwrap_or_default();
        // A browser only lets a page use a relying-party id that its own host ends with.
        if host != rp_id && !host.ends_with(&format!(".{rp_id}")) {
            return Err(format!("{SETTING}: {other} is not {rp_id} or one of its subdomains"));
        }
        builder = builder.append_allowed_origin(&url);
    }
    builder.build().map_err(|e| format!("{SETTING}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn origins(setting: &str) -> Result<Vec<String>, String> {
        engine(Some(setting), "NabiMan").map(|w| w.get_allowed_origins().iter().map(|u| u.origin().ascii_serialization()).collect())
    }

    #[test]
    fn unset_or_blank_means_off_never_a_guess() {
        assert!(engine(None, "NabiMan").unwrap_err().contains("is not set"));
        assert!(engine(Some(" , "), "NabiMan").unwrap_err().contains("is not set"));
    }

    #[test]
    fn a_public_name_behind_the_proxy_and_localhost_for_trying_out() {
        assert_eq!(origins("https://man.example.com").unwrap(), ["https://man.example.com"]);
        assert_eq!(origins("http://localhost:18080").unwrap(), ["http://localhost:18080"]);
        assert_eq!(
            origins("https://example.com, https://man.example.com:8443").unwrap(),
            ["https://example.com", "https://man.example.com:8443"]
        );
    }

    #[test]
    fn what_a_browser_would_refuse_is_refused_at_startup() {
        for (setting, why) in [
            ("http://man.example.com", "must be https"),
            ("http://127.0.0.1:18080", "IP address"),
            ("https://203.0.113.7", "IP address"),
            ("man.example.com", "not an origin"),
            ("https://man.example.com/app", "origin only"),
            ("https://user@man.example.com", "origin only"),
            ("https://man.example.com, https://other.example.org", "subdomains"),
            ("https://man.example.com, https://evilman.example.com", "subdomains"),
        ] {
            let error = engine(Some(setting), "NabiMan").map(|_| ()).unwrap_err();
            assert!(error.contains(why), "{setting}: {error}");
        }
    }
}
