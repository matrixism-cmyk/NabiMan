//! Offline public-key verification. Issuing keys never belong on this server.
use chrono::{NaiveDate, Utc};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    io::{Read, Write},
    path::{Path, PathBuf},
};

const MAX_FILE: u64 = 65536;
const TRIAL_SECONDS: i64 = 30 * 86400;
#[derive(Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct LicenseInfo {
    pub tier: String,
    pub holder: String,
    pub issued_at: String,
    pub expires_at: Option<String>,
    pub max_servers: Option<u32>,
    pub features: Vec<String>,
    /// A plan's limits (`license_plan`); older licences have none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limits: Option<plan::PlanLimits>,
}
#[derive(Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
struct Claims {
    license_id: String,
    product: String,
    info: LicenseInfo,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    version: u32,
    key_id: String,
    payload: String,
    signature: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Stored {
    signed_key: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct KeyFile {
    keys: Vec<KeySpec>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct KeySpec {
    key_id: String,
    public_key: String,
}
#[derive(Serialize, Clone)]
pub struct LicenseStatus {
    pub active: bool,
    pub tier: String,
    pub holder: String,
    pub expires_at: Option<String>,
    pub days_remaining: Option<i64>,
    pub is_trial: bool,
    pub trial_days_remaining: Option<i64>,
    pub features: Vec<String>,
    pub max_servers: Option<u32>,
    pub validation_state: String,
    pub license_id: Option<String>,
    /// The plan, when the licence carries one: then limits and grace apply.
    pub limits: Option<plan::PlanLimits>,
    pub grace_ends_at: Option<String>,
    pub grace_days_remaining: Option<i64>,
}
fn data_dir() -> PathBuf {
    nabiman_server::data_dir()
}
fn key_path() -> PathBuf {
    PathBuf::from(
        std::env::var("NABIMAN_LICENSE_PUBLIC_KEYS_FILE")
            .unwrap_or_else(|_| "/etc/nabiman/license-public-keys.json".into()),
    )
}
fn read_limited(path: &Path, max: u64) -> std::io::Result<String> {
    let file = std::fs::File::open(path)?;
    let mut text = String::new();
    file.take(max + 1).read_to_string(&mut text)?;
    if text.len() as u64 > max {
        return Err(std::io::Error::other("File exceeds size limit"));
    }
    Ok(text)
}
fn key_id_valid(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
}
fn keys_from(text: &str) -> Result<BTreeMap<String, VerifyingKey>, String> {
    let file: KeyFile =
        serde_json::from_str(text).map_err(|_| "Invalid license verification key file")?;
    if file.keys.is_empty() || file.keys.len() > 64 {
        return Err("Invalid key count".into());
    }
    let mut keys = BTreeMap::new();
    for item in file.keys {
        if !key_id_valid(&item.key_id) {
            return Err("Invalid key ID".into());
        }
        let bytes: [u8; 32] = hex::decode(&item.public_key)
            .map_err(|_| "Invalid public key")?
            .try_into()
            .map_err(|_| "Public key must be 32 bytes")?;
        let key = VerifyingKey::from_bytes(&bytes).map_err(|_| "Invalid Ed25519 public key")?;
        if key.is_weak() || keys.insert(item.key_id, key).is_some() {
            return Err("Weak or duplicate verification key".into());
        }
    }
    Ok(keys)
}
fn load_keys(path: &Path) -> Result<BTreeMap<String, VerifyingKey>, String> {
    keys_from(&read_limited(path, MAX_FILE).map_err(|_| "License verification keys unavailable")?)
}
fn signed_message(key_id: &str, payload: &[u8]) -> Vec<u8> {
    let mut msg = b"NabiMan license v1\0".to_vec();
    msg.extend_from_slice(key_id.as_bytes());
    msg.push(0);
    msg.extend_from_slice(payload);
    msg
}
fn features_for_tier(tier: &str) -> Vec<String> {
    let values: &[&str] = match tier {
        "enterprise" => &[
            "basic_monitoring",
            "management",
            "alerts",
            "backups",
            "ldap",
            "oauth",
            "api_keys",
            "unlimited_servers",
        ],
        "pro" => &["basic_monitoring", "management", "alerts", "backups"],
        _ => &["basic_monitoring"],
    };
    values.iter().map(|s| s.to_string()).collect()
}
fn date(value: &str) -> Result<NaiveDate, String> {
    let date = NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| "Invalid license date")?;
    if date.format("%Y-%m-%d").to_string() != value {
        return Err("License dates must use YYYY-MM-DD".into());
    }
    Ok(date)
}
fn validate_claims(claims: &Claims, today: NaiveDate) -> Result<(), String> {
    let info = &claims.info;
    if claims.product != "nabiman"
        || !key_id_valid(&claims.license_id)
        || !matches!(info.tier.as_str(), "free" | "pro" | "enterprise")
    {
        return Err("Invalid license product, ID or tier".into());
    }
    if info.holder.trim().is_empty()
        || info.holder.len() > 256
        || info.holder.chars().any(char::is_control)
    {
        return Err("Invalid license holder".into());
    }
    let issued = date(&info.issued_at)?;
    if issued > today {
        return Err("License is not yet valid".into());
    }
    if let Some(exp) = &info.expires_at {
        let exp = date(exp)?;
        if exp < issued {
            return Err("Expiry precedes issue date".into());
        }
    }
    let allowed = features_for_tier(&info.tier);
    let mut unique = HashSet::new();
    if info.features.is_empty()
        || info
            .features
            .iter()
            .any(|f| !allowed.contains(f) || !unique.insert(f))
    {
        return Err("Invalid license features".into());
    }
    if let Some(limits) = &info.limits {
        // A plan states its own server limit; the tier's fixed ones do not apply.
        return plan::validate(limits, info.max_servers);
    }
    match info.tier.as_str() {
        "free" if info.max_servers != Some(1) => {
            return Err("Free license server limit must be 1".into())
        }
        "pro" if !matches!(info.max_servers, Some(1..=10)) => {
            return Err("Pro license server limit must be between 1 and 10".into())
        }
        "enterprise" if info.max_servers == Some(0) => return Err("Invalid server limit".into()),
        _ => {}
    }
    Ok(())
}
fn expiry(claims: &Claims) -> Option<NaiveDate> {
    claims.info.expires_at.as_ref().and_then(|d| date(d).ok())
}
/// A licence that verifies and has not expired: what activation accepts.
fn verify(key: &str, keys: &BTreeMap<String, VerifyingKey>, today: NaiveDate) -> Result<Claims, String> {
    let claims = verify_signed(key, keys, today)?;
    if expiry(&claims).is_some_and(|exp| exp < today) {
        return Err("License has expired".into());
    }
    Ok(claims)
}
/// Signature, issuer and every field, but not whether it has expired.
fn verify_signed(
    key: &str,
    keys: &BTreeMap<String, VerifyingKey>,
    today: NaiveDate,
) -> Result<Claims, String> {
    if key.len() > MAX_FILE as usize {
        return Err("License key exceeds size limit".into());
    }
    let encoded = key
        .strip_prefix("NBL1.")
        .ok_or("Unsupported license format; obtain a signed replacement")?;
    let bytes = hex::decode(encoded).map_err(|_| "Invalid license envelope")?;
    let envelope: Envelope =
        serde_json::from_slice(&bytes).map_err(|_| "Invalid license envelope")?;
    if envelope.version != 1 || !key_id_valid(&envelope.key_id) {
        return Err("Unsupported license version or key ID".into());
    }
    let public = keys
        .get(&envelope.key_id)
        .ok_or("Unknown license issuer key")?;
    let payload = hex::decode(&envelope.payload).map_err(|_| "Invalid license payload")?;
    if payload.len() > 8192 {
        return Err("License payload exceeds size limit".into());
    }
    let bytes = hex::decode(&envelope.signature).map_err(|_| "Invalid license signature")?;
    let signature = Signature::from_slice(&bytes).map_err(|_| "Invalid license signature")?;
    public
        .verify_strict(&signed_message(&envelope.key_id, &payload), &signature)
        .map_err(|_| "Invalid license signature")?;
    let claims: Claims =
        serde_json::from_slice(&payload).map_err(|_| "Invalid signed license data")?;
    validate_claims(&claims, today)?;
    Ok(claims)
}
fn inactive(state: &str) -> LicenseStatus {
    LicenseStatus {
        active: false,
        tier: "free".into(),
        holder: String::new(),
        expires_at: None,
        days_remaining: None,
        is_trial: false,
        trial_days_remaining: None,
        features: features_for_tier("free"),
        max_servers: Some(1),
        validation_state: state.into(),
        license_id: None,
        limits: None,
        grace_ends_at: None,
        grace_days_remaining: None,
    }
}
fn trial(dir: &Path, now: i64) -> LicenseStatus {
    use std::os::unix::fs::OpenOptionsExt;
    let path = dir.join("install_timestamp");
    if !path.exists() {
        let result = (|| -> std::io::Result<()> {
            std::fs::create_dir_all(dir)?;
            let mut f = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&path)?;
            write!(f, "{now}")?;
            f.sync_all()
        })();
        if let Err(e) = result {
            if e.kind() != std::io::ErrorKind::AlreadyExists {
                return inactive("storage_error");
            }
        }
    }
    let ts = match read_limited(&path, 64)
        .ok()
        .and_then(|s| s.trim().parse::<i64>().ok())
    {
        Some(ts) if ts > 0 && ts <= now => ts,
        _ => return inactive("invalid_trial_timestamp"),
    };
    let seconds = (TRIAL_SECONDS - (now - ts)).max(0);
    let remaining = (seconds + 86399) / 86400;
    let mut status = inactive(if seconds > 0 {
        "trial"
    } else {
        "trial_expired"
    });
    status.active = seconds > 0;
    status.is_trial = true;
    status.trial_days_remaining = Some(remaining);
    status
}
fn status_at(dir: &Path, key_file: &Path, now: chrono::DateTime<Utc>) -> LicenseStatus {
    let file = match read_limited(&dir.join("license.json"), MAX_FILE) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return trial(dir, now.timestamp()),
        Err(_) => return inactive("storage_error"),
    };
    let stored: Stored = match serde_json::from_str(&file) {
        Ok(s) => s,
        Err(_) => return inactive("invalid_license"),
    };
    let keys = match load_keys(key_file) {
        Ok(keys) => keys,
        Err(_) => return inactive("verification_unavailable"),
    };
    let today = now.date_naive();
    let claims = match verify_signed(&stored.signed_key, &keys, today) {
        Ok(c) => c,
        Err(_) => return inactive("invalid_license"),
    };
    let expired = expiry(&claims).filter(|exp| *exp < today);
    match (expired, &claims.info.limits) {
        (None, _) => licensed(claims, today),
        // Without a plan an expired licence is simply expired, as it always was.
        (Some(_), None) => inactive("license_expired"),
        (Some(exp), Some(limits)) => match plan::after_expiry(limits, exp, today) {
            plan::AfterExpiry::Grace { ends, days_left } => {
                let mut status = licensed(claims, today);
                status.validation_state = "grace".into();
                status.grace_ends_at = Some(ends.to_string());
                status.grace_days_remaining = Some(days_left);
                status
            }
            plan::AfterExpiry::Ended { ended } => {
                // Past grace: the basic features, and the plan kept so growth stays stopped.
                let mut status = inactive("license_expired");
                status.holder = claims.info.holder.clone();
                status.expires_at = claims.info.expires_at.clone();
                status.days_remaining = Some((exp - today).num_days());
                status.max_servers = claims.info.max_servers;
                status.license_id = Some(claims.license_id.clone());
                status.limits = claims.info.limits.clone();
                status.grace_ends_at = Some(ended.to_string());
                status.grace_days_remaining = Some(0);
                status
            }
        },
    }
}
/// The licence as `GET /api/license` reads it now.
pub(crate) fn current() -> LicenseStatus {
    status_at(&data_dir(), &key_path(), Utc::now())
}
fn licensed(claims: Claims, today: NaiveDate) -> LicenseStatus {
    let info = claims.info;
    LicenseStatus {
        active: true,
        tier: info.tier,
        holder: info.holder,
        days_remaining: info.expires_at.as_ref().and_then(|d| date(d).ok()).map(|d| (d - today).num_days()),
        expires_at: info.expires_at,
        is_trial: false,
        trial_days_remaining: None,
        features: info.features,
        max_servers: info.max_servers,
        validation_state: "licensed".into(),
        license_id: Some(claims.license_id),
        limits: info.limits,
        grace_ends_at: None,
        grace_days_remaining: None,
    }
}
#[path = "license_http.rs"]
mod http;
pub use http::config;
#[path = "license_plan.rs"]
pub(crate) mod plan;
#[cfg(test)]
#[path = "license_tests.rs"]
mod tests;
