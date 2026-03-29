use actix_web::{web, HttpResponse};
use crate::models::ApiResponse;
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use std::path::PathBuf;

const HMAC_SECRET: &[u8] = b"nabiman-license-hmac-secret-key-2026";
const TRIAL_DAYS: i64 = 30;
const HMAC_BLOCK_SIZE: usize = 64;

#[derive(Serialize, Deserialize, Clone)]
pub struct LicenseInfo {
    pub tier: String,
    pub holder: String,
    pub issued_at: String,
    pub expires_at: Option<String>,
    pub max_servers: Option<u32>,
    pub features: Vec<String>,
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
}

#[derive(Deserialize)]
pub struct ActivateRequest { pub key: String }

fn data_dir() -> PathBuf {
    PathBuf::from(std::env::var("NABIMAN_DATA_DIR").unwrap_or_else(|_| "/var/lib/nabiman".into()))
}

fn load_license() -> Option<LicenseInfo> {
    std::fs::read_to_string(data_dir().join("license.json"))
        .ok().and_then(|s| serde_json::from_str(&s).ok())
}

fn save_license(info: &LicenseInfo) -> Result<(), String> {
    let dir = data_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("Cannot create data dir: {}", e))?;
    let json = serde_json::to_string_pretty(info).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("license.json"), json)
        .map_err(|e| format!("Cannot write license: {}", e))
}

fn get_trial_info() -> (bool, Option<i64>) {
    let ts_path = data_dir().join("install_timestamp");
    let install_ts = if ts_path.exists() {
        std::fs::read_to_string(&ts_path).ok().and_then(|s| s.trim().parse::<i64>().ok())
    } else {
        let now = chrono::Utc::now().timestamp();
        let _ = std::fs::create_dir_all(data_dir());
        let _ = std::fs::write(&ts_path, now.to_string());
        Some(now)
    };
    match install_ts {
        Some(ts) => {
            let remaining = TRIAL_DAYS - (chrono::Utc::now().timestamp() - ts) / 86400;
            if remaining > 0 { (true, Some(remaining)) } else { (false, Some(0)) }
        }
        None => (false, None),
    }
}

fn features_for_tier(tier: &str) -> Vec<String> {
    match tier {
        "enterprise" => vec![
            "basic_monitoring", "management", "alerts", "backups",
            "ldap", "oauth", "api_keys", "unlimited_servers",
        ].into_iter().map(String::from).collect(),
        "pro" => vec!["basic_monitoring", "management", "alerts", "backups"]
            .into_iter().map(String::from).collect(),
        _ => vec!["basic_monitoring".into()],
    }
}

fn max_servers_for_tier(tier: &str) -> Option<u32> {
    match tier { "enterprise" => None, "pro" => Some(10), _ => Some(1) }
}

fn license_days_remaining(expires_at: &Option<String>) -> Option<i64> {
    expires_at.as_ref().and_then(|exp| {
        chrono::NaiveDate::parse_from_str(exp, "%Y-%m-%d").ok().map(|d| {
            (d - chrono::Utc::now().date_naive()).num_days()
        })
    })
}

fn get_license_status() -> LicenseStatus {
    let (trial_active, trial_remaining) = get_trial_info();
    match load_license() {
        Some(lic) => {
            let days_rem = license_days_remaining(&lic.expires_at);
            let expired = days_rem.map(|d| d < 0).unwrap_or(false);
            LicenseStatus {
                active: !expired, tier: lic.tier, holder: lic.holder,
                expires_at: lic.expires_at, days_remaining: days_rem,
                is_trial: false, trial_days_remaining: None,
                features: lic.features, max_servers: lic.max_servers,
            }
        }
        None => LicenseStatus {
            active: trial_active, tier: "free".into(), holder: String::new(),
            expires_at: None, days_remaining: None,
            is_trial: true, trial_days_remaining: trial_remaining,
            features: features_for_tier("free"), max_servers: max_servers_for_tier("free"),
        },
    }
}

// HMAC-SHA256 (RFC 2104) implemented with sha2
fn hmac_sha256(key: &[u8], message: &[u8]) -> Vec<u8> {
    let mut padded_key = vec![0u8; HMAC_BLOCK_SIZE];
    if key.len() > HMAC_BLOCK_SIZE {
        let hash = Sha256::digest(key);
        padded_key[..hash.len()].copy_from_slice(&hash);
    } else {
        padded_key[..key.len()].copy_from_slice(key);
    }
    let mut ipad = vec![0x36u8; HMAC_BLOCK_SIZE];
    let mut opad = vec![0x5cu8; HMAC_BLOCK_SIZE];
    for i in 0..HMAC_BLOCK_SIZE {
        ipad[i] ^= padded_key[i];
        opad[i] ^= padded_key[i];
    }
    let mut inner = Sha256::new();
    inner.update(&ipad);
    inner.update(message);
    let inner_hash = inner.finalize();
    let mut outer = Sha256::new();
    outer.update(&opad);
    outer.update(&inner_hash);
    outer.finalize().to_vec()
}

fn base64_decode(input: &str) -> Result<Vec<u8>, String> {
    let table = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut lookup = [255u8; 256];
    for (i, &ch) in table.iter().enumerate() { lookup[ch as usize] = i as u8; }
    let clean: Vec<u8> = input.bytes()
        .filter(|&b| b != b'\n' && b != b'\r' && b != b' ').collect();
    let stripped: &[u8] = clean.strip_suffix(b"==")
        .or_else(|| clean.strip_suffix(b"=")).unwrap_or(&clean);
    let mut out = Vec::with_capacity(stripped.len() * 3 / 4);
    for chunk in stripped.chunks(4) {
        let mut accum: u32 = 0;
        for (j, &byte) in chunk.iter().enumerate() {
            let val = lookup[byte as usize];
            if val == 255 { return Err(format!("Invalid base64 character: {}", byte as char)); }
            accum |= (val as u32) << (6 * (3 - j));
        }
        let n = match chunk.len() { 4 => 3, 3 => 2, 2 => 1, _ => return Err("Bad base64".into()) };
        if n >= 1 { out.push((accum >> 16) as u8); }
        if n >= 2 { out.push((accum >> 8) as u8); }
        if n >= 3 { out.push(accum as u8); }
    }
    Ok(out)
}

// Key format: base64( json_payload + "." + hex(hmac_sha256(secret, json_payload)) )
fn validate_license_key(key: &str) -> Result<LicenseInfo, String> {
    let decoded = base64_decode(key)?;
    let decoded_str = String::from_utf8(decoded).map_err(|_| "Invalid UTF-8 in key")?;
    let dot_pos = decoded_str.rfind('.').ok_or("Invalid key format: no separator")?;
    let json_part = &decoded_str[..dot_pos];
    let sig_hex = &decoded_str[dot_pos + 1..];
    let expected_hex = hex::encode(hmac_sha256(HMAC_SECRET, json_part.as_bytes()));
    if !constant_time_eq(sig_hex.as_bytes(), expected_hex.as_bytes()) {
        return Err("Invalid license signature".into());
    }
    let info: LicenseInfo = serde_json::from_str(json_part)
        .map_err(|e| format!("Invalid license data: {}", e))?;
    if !["free", "pro", "enterprise"].contains(&info.tier.as_str()) {
        return Err("Unknown license tier".into());
    }
    if let Some(ref exp) = info.expires_at {
        if let Ok(d) = chrono::NaiveDate::parse_from_str(exp, "%Y-%m-%d") {
            if d < chrono::Utc::now().date_naive() { return Err("License has expired".into()); }
        }
    }
    Ok(info)
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() { return false; }
    a.iter().zip(b.iter()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

// --- Route handlers ---

async fn get_status() -> HttpResponse {
    HttpResponse::Ok().json(ApiResponse::ok(get_license_status()))
}

async fn activate(body: web::Json<ActivateRequest>) -> HttpResponse {
    let key = body.key.trim();
    if key.is_empty() {
        return HttpResponse::Ok().json(ApiResponse::<LicenseStatus>::error("License key is empty"));
    }
    match validate_license_key(key) {
        Ok(info) => match save_license(&info) {
            Ok(()) => HttpResponse::Ok().json(ApiResponse::ok(get_license_status())),
            Err(e) => HttpResponse::Ok().json(ApiResponse::<LicenseStatus>::error(&e)),
        },
        Err(e) => HttpResponse::Ok().json(ApiResponse::<LicenseStatus>::error(&e)),
    }
}

async fn get_features() -> HttpResponse {
    HttpResponse::Ok().json(ApiResponse::ok(get_license_status().features))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/license")
            .route("", web::get().to(get_status))
            .route("/activate", web::post().to(activate))
            .route("/features", web::get().to(get_features)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base64_encode_test(input: &str) -> String {
        let table = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let bytes = input.as_bytes();
        let mut result = String::new();
        for chunk in bytes.chunks(3) {
            let (b0, b1, b2) = (chunk[0] as u32,
                chunk.get(1).copied().unwrap_or(0) as u32,
                chunk.get(2).copied().unwrap_or(0) as u32);
            let triple = (b0 << 16) | (b1 << 8) | b2;
            result.push(table[((triple >> 18) & 0x3F) as usize] as char);
            result.push(table[((triple >> 12) & 0x3F) as usize] as char);
            result.push(if chunk.len() > 1 { table[((triple >> 6) & 0x3F) as usize] as char } else { '=' });
            result.push(if chunk.len() > 2 { table[(triple & 0x3F) as usize] as char } else { '=' });
        }
        result
    }

    fn make_key(info: &LicenseInfo, secret: &[u8]) -> String {
        let json = serde_json::to_string(info).unwrap();
        let mac = hmac_sha256(secret, json.as_bytes());
        base64_encode_test(&format!("{}.{}", json, hex::encode(&mac)))
    }

    #[test]
    fn test_features_for_tier() {
        assert_eq!(features_for_tier("free"), vec!["basic_monitoring"]);
        assert!(features_for_tier("pro").contains(&"alerts".to_string()));
        assert!(features_for_tier("enterprise").contains(&"ldap".to_string()));
    }

    #[test]
    fn test_max_servers_for_tier() {
        assert_eq!(max_servers_for_tier("free"), Some(1));
        assert_eq!(max_servers_for_tier("pro"), Some(10));
        assert_eq!(max_servers_for_tier("enterprise"), None);
    }

    #[test]
    fn test_hmac_sha256_consistency() {
        let mac1 = hmac_sha256(b"key", b"message");
        let mac2 = hmac_sha256(b"key", b"message");
        assert_eq!(mac1, mac2);
        assert_ne!(mac1, hmac_sha256(b"key", b"different"));
    }

    #[test]
    fn test_constant_time_eq() {
        assert!(constant_time_eq(b"hello", b"hello"));
        assert!(!constant_time_eq(b"hello", b"world"));
        assert!(!constant_time_eq(b"short", b"longer"));
    }

    #[test]
    fn test_base64_decode() {
        assert_eq!(base64_decode("aGVsbG8=").unwrap(), b"hello");
        assert!(base64_decode("").unwrap().is_empty());
    }

    #[test]
    fn test_validate_bad_key() {
        assert!(validate_license_key("not-valid-base64!!!").is_err());
        assert!(validate_license_key("aGVsbG8=").is_err());
    }

    #[test]
    fn test_roundtrip_license_key() {
        let info = LicenseInfo {
            tier: "pro".into(), holder: "Test User".into(),
            issued_at: "2026-01-01".into(), expires_at: Some("2027-01-01".into()),
            max_servers: Some(10), features: features_for_tier("pro"),
        };
        let key = make_key(&info, HMAC_SECRET);
        let result = validate_license_key(&key).unwrap();
        assert_eq!(result.tier, "pro");
        assert_eq!(result.holder, "Test User");
    }

    #[test]
    fn test_tampered_key_rejected() {
        let info = LicenseInfo {
            tier: "enterprise".into(), holder: "Legit User".into(),
            issued_at: "2026-01-01".into(), expires_at: Some("2027-01-01".into()),
            max_servers: None, features: features_for_tier("enterprise"),
        };
        let key = make_key(&info, b"wrong-secret");
        assert!(validate_license_key(&key).is_err());
    }

    #[test]
    fn test_license_days_remaining() {
        let future = chrono::Utc::now().date_naive() + chrono::Duration::days(30);
        let exp = Some(future.format("%Y-%m-%d").to_string());
        assert_eq!(license_days_remaining(&exp), Some(30));
    }
}
