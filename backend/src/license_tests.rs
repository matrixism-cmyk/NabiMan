use super::*;
use chrono::{Duration, TimeZone};
use ed25519_dalek::{Signer, SigningKey};

fn today() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 5, 12, 0, 0).unwrap()
}
fn issuer() -> SigningKey {
    SigningKey::from_bytes(&[37; 32])
}
fn keys() -> BTreeMap<String, VerifyingKey> {
    BTreeMap::from([("test-issuer".into(), issuer().verifying_key())])
}
fn claims() -> Claims {
    Claims {
        license_id: "test-license-1".into(),
        product: "nabiman".into(),
        info: LicenseInfo {
            tier: "pro".into(),
            holder: "Test customer".into(),
            issued_at: "2026-10-01".into(),
            expires_at: Some("2026-10-31".into()),
            max_servers: Some(10),
            features: features_for_tier("pro"),
        },
    }
}
fn sign_with(c: &Claims, id: &str, signer: &SigningKey) -> String {
    let payload = serde_json::to_vec(c).unwrap();
    let signature = signer.sign(&signed_message(id, &payload));
    encode(&Envelope {
        version: 1,
        key_id: id.into(),
        payload: hex::encode(payload),
        signature: hex::encode(signature.to_bytes()),
    })
}
fn sign(c: &Claims) -> String {
    sign_with(c, "test-issuer", &issuer())
}
fn encode(e: &Envelope) -> String {
    format!("NBL1.{}", hex::encode(serde_json::to_vec(e).unwrap()))
}
fn unpack(s: &str) -> Envelope {
    serde_json::from_slice(&hex::decode(s.strip_prefix("NBL1.").unwrap()).unwrap()).unwrap()
}
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!("nabiman-license-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&p).unwrap();
        Self(p)
    }
    fn roots(&self) -> PathBuf {
        let p = self.0.join("roots.json");
        std::fs::write(&p,serde_json::json!({"keys":[{"key_id":"test-issuer","public_key":hex::encode(issuer().verifying_key().to_bytes())}]}).to_string()).unwrap();
        p
    }
    fn store(&self, key: &str) {
        crate::private_storage::write(
            &self.0.join("license.json"),
            &serde_json::to_vec(&Stored {
                signed_key: key.into(),
            })
            .unwrap(),
        )
        .unwrap();
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn valid_signature_preserves_only_signed_entitlements() {
    let mut c = claims();
    c.info.features = vec!["basic_monitoring".into()];
    c.info.max_servers = Some(3);
    let r = verify(&sign(&c), &keys(), today().date_naive()).unwrap();
    assert_eq!(r.info.features, c.info.features);
    assert_eq!(r.info.max_servers, Some(3));
}
#[test]
fn changed_payload_signature_or_key_id_is_rejected() {
    let good = sign(&claims());
    let mut e = unpack(&good);
    let mut c = claims();
    c.info.tier = "enterprise".into();
    e.payload = hex::encode(serde_json::to_vec(&c).unwrap());
    assert!(verify(&encode(&e), &keys(), today().date_naive()).is_err());
    let mut e = unpack(&good);
    e.signature.replace_range(..2, "00");
    assert!(verify(&encode(&e), &keys(), today().date_naive()).is_err());
    let mut e = unpack(&good);
    e.key_id = "other".into();
    let mut roots = keys();
    roots.insert("other".into(), issuer().verifying_key());
    assert!(verify(&encode(&e), &roots, today().date_naive()).is_err());
}
#[test]
fn wrong_issuer_version_and_old_hmac_are_rejected() {
    assert!(verify(
        &sign_with(&claims(), "test-issuer", &SigningKey::from_bytes(&[42; 32])),
        &keys(),
        today().date_naive()
    )
    .is_err());
    let mut e = unpack(&sign(&claims()));
    e.version = 2;
    assert!(verify(&encode(&e), &keys(), today().date_naive()).is_err());
    assert!(verify("old-hmac-key", &keys(), today().date_naive()).is_err());
}
#[test]
fn rotation_accepts_overlap_then_rejects_retired_issuer() {
    let second = SigningKey::from_bytes(&[42; 32]);
    let mut roots = keys();
    roots.insert("next".into(), second.verifying_key());
    let old = sign(&claims());
    let new = sign_with(&claims(), "next", &second);
    assert!(verify(&old, &roots, today().date_naive()).is_ok());
    assert!(verify(&new, &roots, today().date_naive()).is_ok());
    roots.remove("test-issuer");
    assert!(verify(&old, &roots, today().date_naive()).is_err());
    assert!(verify(&new, &roots, today().date_naive()).is_ok());
}
#[test]
fn invalid_weak_duplicate_or_empty_roots_are_rejected() {
    assert!(keys_from(r#"{"keys":[]}"#).is_err());
    let root = serde_json::json!({"key_id":"test-issuer","public_key":hex::encode(issuer().verifying_key().to_bytes())});
    assert!(keys_from(&serde_json::json!({"keys":[root.clone(),root]}).to_string()).is_err());
    assert!(keys_from(
        &serde_json::json!({"keys":[{"key_id":"weak","public_key":hex::encode([0u8;32])}]})
            .to_string()
    )
    .is_err());
    assert!(keys_from(r#"{"keys":[{"key_id":"bad/id","public_key":"00"}]}"#).is_err());
}
#[test]
fn signed_invalid_dates_and_future_issue_are_rejected() {
    for (issued, expires) in [
        ("2026-10-32", "2026-10-31"),
        ("2026-10-1", "2026-10-31"),
        ("2026-10-06", "2026-10-31"),
        ("2026-10-01", "invalid"),
        ("2026-10-01", "2026-09-30"),
        ("2026-10-01", "2026-10-04"),
    ] {
        let mut c = claims();
        c.info.issued_at = issued.into();
        c.info.expires_at = Some(expires.into());
        assert!(
            verify(&sign(&c), &keys(), today().date_naive()).is_err(),
            "{issued} {expires}"
        );
    }
}
#[test]
fn expiry_is_inclusive_and_rechecked_on_each_read() {
    let d = Temp::new();
    let roots = d.roots();
    let mut c = claims();
    c.info.expires_at = Some("2026-10-05".into());
    d.store(&sign(&c));
    assert!(status_at(&d.0, &roots, today()).active);
    let expired = status_at(&d.0, &roots, today() + Duration::days(1));
    assert!(!expired.active);
    assert_eq!(expired.validation_state, "license_expired");
    assert_eq!(expired.features, features_for_tier("free"));
}
#[test]
fn invalid_signed_features_limits_holder_and_product_are_rejected() {
    let mut variants = Vec::new();
    let mut c = claims();
    c.info.features.push("ldap".into());
    variants.push(c);
    let mut c = claims();
    c.info.features.push("backups".into());
    variants.push(c);
    let mut c = claims();
    c.info.max_servers = None;
    variants.push(c);
    let mut c = claims();
    c.info.max_servers = Some(11);
    variants.push(c);
    let mut c = claims();
    c.info.holder = " ".into();
    variants.push(c);
    let mut c = claims();
    c.product = "nabizip".into();
    variants.push(c);
    for c in variants {
        assert!(verify(&sign(&c), &keys(), today().date_naive()).is_err());
    }
}
#[test]
fn unsigned_and_corrupt_files_do_not_fall_back_to_trial() {
    let d = Temp::new();
    let roots = d.roots();
    for text in [
        serde_json::to_string(&claims().info).unwrap(),
        "invalid".into(),
    ] {
        std::fs::write(d.0.join("license.json"), text).unwrap();
        let s = status_at(&d.0, &roots, today());
        assert!(!s.active);
        assert!(!s.is_trial);
        assert_eq!(s.validation_state, "invalid_license");
        assert!(!d.0.join("install_timestamp").exists());
    }
}
#[test]
fn trusted_roots_are_required_even_for_previously_stored_license() {
    let d = Temp::new();
    let roots = d.roots();
    d.store(&sign(&claims()));
    assert!(status_at(&d.0, &roots, today()).active);
    std::fs::remove_file(&roots).unwrap();
    let s = status_at(&d.0, &roots, today());
    assert!(!s.active);
    assert_eq!(s.validation_state, "verification_unavailable");
}
#[test]
fn tampering_persisted_license_is_detected_on_next_read() {
    let d = Temp::new();
    let roots = d.roots();
    d.store(&sign(&claims()));
    assert!(status_at(&d.0, &roots, today()).active);
    let mut e = unpack(&sign(&claims()));
    e.payload.push_str("00");
    d.store(&encode(&e));
    assert!(!status_at(&d.0, &roots, today()).active);
}
#[test]
fn trial_timestamp_is_bounded_and_not_silently_renewed() {
    let d = Temp::new();
    let s = trial(&d.0, today().timestamp());
    assert!(s.active);
    assert_eq!(s.trial_days_remaining, Some(30));
    assert_eq!(
        trial(&d.0, today().timestamp() + TRIAL_SECONDS).validation_state,
        "trial_expired"
    );
    for bad in [
        "garbage".into(),
        (today().timestamp() + 1).to_string(),
        "-1".into(),
    ] {
        std::fs::write(d.0.join("install_timestamp"), &bad).unwrap();
        let s = trial(&d.0, today().timestamp());
        assert!(!s.active);
        assert_eq!(s.validation_state, "invalid_trial_timestamp");
        assert_eq!(
            std::fs::read_to_string(d.0.join("install_timestamp")).unwrap(),
            bad
        );
    }
}
#[test]
fn failed_trial_storage_never_grants_trial() {
    let d = Temp::new();
    let file = d.0.join("not-directory");
    std::fs::write(&file, "x").unwrap();
    assert!(!trial(&file, today().timestamp()).active);
}
