//! Explicit host identity and time-bounded legacy SSH policy.
use crate::mec::services::{ServiceError, ServiceResult};
use chrono::{DateTime, Utc};

#[derive(Clone)]
pub(super) struct HostTrust {
    fingerprint: String,
    pub legacy: bool,
}

impl HostTrust {
    pub fn parse(pin: &str, legacy_until: Option<&str>, now: DateTime<Utc>) -> ServiceResult<Self> {
        let digest = pin.strip_prefix("SHA256:").ok_or_else(||
            ServiceError::Unavailable("AXGate host key is not trusted; configure NABIMAN_MEC_AXGATE_HOST_KEY_SHA256 after independent verification".into()))?;
        // Canonical OpenSSH SHA256 is 32 bytes encoded as 43 unpadded base64 chars.
        let canonical = digest.len() == 43
            && digest
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'/')
            && digest
                .as_bytes()
                .last()
                .is_some_and(|b| b"AEIMQUYcgkosw048".contains(b));
        if !canonical {
            return Err(ServiceError::Unavailable(
                "Invalid AXGate host key fingerprint; require canonical SHA256 fingerprint".into(),
            ));
        }
        let legacy = match legacy_until {
            None => false,
            Some(value) => {
                let until = DateTime::parse_from_rfc3339(value)
                    .map_err(|_| {
                        ServiceError::Unavailable(
                            "Invalid AXGate legacy exception expiry; require RFC3339".into(),
                        )
                    })?
                    .with_timezone(&Utc);
                if until <= now || until > now + chrono::Duration::days(30) {
                    return Err(ServiceError::Unavailable("AXGate legacy exception must expire within 30 days and must not be expired".into()));
                }
                true
            }
        };
        Ok(Self {
            fingerprint: pin.to_owned(),
            legacy,
        })
    }

    pub fn matches(&self, key: &russh::keys::PublicKey) -> bool {
        self.fingerprint == key.fingerprint(russh::keys::HashAlg::Sha256).to_string()
    }
}
