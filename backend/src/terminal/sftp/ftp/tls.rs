//! The TLS half of FTPS. A certificate is trusted when a public authority
//! vouches for it under the server's host name, or when it is exactly the one
//! the operator verified by hand and saved as its fingerprint, the way an SSH
//! host key is approved. Nothing else is accepted, and a refusal names the
//! certificate's fingerprint so it can be checked out of band.

use std::sync::{Arc, Mutex};

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::client::WebPkiServerVerifier;
use rustls::crypto::{verify_tls12_signature, verify_tls13_signature, CryptoProvider};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{ClientConfig, DigitallySignedStruct, Error, RootCertStore, SignatureScheme};
use sha2::{Digest, Sha256};

/// `SHA256:AB:CD:…`, the form `openssl x509 -fingerprint -sha256` prints
/// (after its own prefix).
pub(crate) fn fingerprint(cert: &[u8]) -> String {
    let hex: Vec<String> = Sha256::digest(cert).iter().map(|b| format!("{b:02X}")).collect();
    format!("SHA256:{}", hex.join(":"))
}

/// The 32 bytes of a fingerprint as an operator might paste it: with or
/// without `SHA256:` or openssl's `sha256 Fingerprint=`, colons or spaces.
pub(crate) fn parse_fingerprint(text: &str) -> Option<[u8; 32]> {
    let t = text.trim();
    let t = t.rsplit_once('=').map_or(t, |(_, v)| v);
    let t = t.strip_prefix("SHA256:").or_else(|| t.strip_prefix("sha256:")).unwrap_or(t);
    let digits: String = t.chars().filter(|c| !matches!(c, ':' | ' ')).collect();
    hex::decode(digits).ok()?.try_into().ok()
}

#[derive(Debug)]
struct Verifier {
    /// The certificate saved by the operator, or none to rely on authorities.
    pinned: Option<[u8; 32]>,
    authorities: Option<Arc<WebPkiServerVerifier>>,
    provider: Arc<CryptoProvider>,
    /// The last certificate refused, for the error message.
    refused: Arc<Mutex<Option<String>>>,
}

impl ServerCertVerifier for Verifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        name: &ServerName<'_>,
        ocsp: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, Error> {
        let seen: [u8; 32] = Sha256::digest(end_entity.as_ref()).into();
        let verdict = match (&self.pinned, &self.authorities) {
            (Some(pinned), _) if *pinned == seen => Ok(ServerCertVerified::assertion()),
            (Some(_), _) => Err(Error::General("certificate does not match the saved fingerprint".into())),
            (None, Some(webpki)) => webpki.verify_server_cert(end_entity, intermediates, name, ocsp, now),
            (None, None) => Err(Error::General("no certificate authorities available".into())),
        };
        if verdict.is_err() {
            *self.refused.lock().unwrap_or_else(|e| e.into_inner()) = Some(fingerprint(end_entity.as_ref()));
        }
        verdict
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        verify_tls12_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        verify_tls13_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider.signature_verification_algorithms.supported_schemes()
    }
}

/// One server's TLS settings. Each control connection takes its own copy
/// (`connection_config`), shared with its data connections so they resume the
/// control connection's TLS session, which many servers insist on.
pub(crate) struct Tls {
    pub config: Arc<ClientConfig>,
    pub name: ServerName<'static>,
    refused: Arc<Mutex<Option<String>>>,
}

impl Tls {
    pub fn new(host: &str, saved_fingerprint: &str) -> Result<Self, String> {
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let pinned = if saved_fingerprint.trim().is_empty() {
            None
        } else {
            Some(parse_fingerprint(saved_fingerprint).ok_or("The saved certificate fingerprint is not valid")?)
        };
        let authorities = if pinned.is_some() {
            None
        } else {
            let mut roots = RootCertStore::empty();
            for cert in rustls_native_certs::load_native_certs().certs {
                let _ = roots.add(cert);
            }
            WebPkiServerVerifier::builder_with_provider(Arc::new(roots), provider.clone()).build().ok()
        };
        let refused = Arc::new(Mutex::new(None));
        let verifier = Verifier { pinned, authorities, provider: provider.clone(), refused: refused.clone() };
        let config = ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .map_err(|e| format!("TLS setup failed: {e}"))?
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(verifier))
            .with_no_client_auth();
        let name = ServerName::try_from(host.to_string()).map_err(|_| "Invalid host name for TLS")?;
        Ok(Tls { config: Arc::new(config), name, refused })
    }

    /// The settings for one control connection and its data connections,
    /// with a session cache of their own: a data connection must resume its
    /// own control connection's session, not another's.
    pub fn connection_config(&self) -> Arc<ClientConfig> {
        let mut config = (*self.config).clone();
        // Room for several servers' worth: a cache sized for one server
        // evicts each session as soon as it is stored.
        config.resumption = rustls::client::Resumption::in_memory_sessions(64);
        Arc::new(config)
    }

    /// Why a handshake failed, in words an operator can act on.
    pub fn explain(&self, e: &std::io::Error) -> String {
        match self.refused.lock().unwrap_or_else(|e| e.into_inner()).take() {
            Some(seen) => format!(
                "The server's TLS certificate is not trusted ({e}). Its fingerprint is {seen}; \
                 verify it with the server's administrator and save it in the server settings"
            ),
            None => format!("TLS handshake failed: {e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprints_parse_in_the_forms_people_paste() {
        let printed = fingerprint(b"certificate");
        let bytes = parse_fingerprint(&printed).unwrap();
        assert_eq!(bytes, <[u8; 32]>::from(Sha256::digest(b"certificate")));
        let openssl = format!("sha256 Fingerprint={}", printed.trim_start_matches("SHA256:"));
        assert_eq!(parse_fingerprint(&openssl), Some(bytes));
        let bare = printed.trim_start_matches("SHA256:").replace(':', "").to_lowercase();
        assert_eq!(parse_fingerprint(&bare), Some(bytes));
        assert_eq!(parse_fingerprint("SHA256:abcd"), None);
        assert_eq!(parse_fingerprint("not hex"), None);
    }
}
