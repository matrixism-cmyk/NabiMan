//! Plans: what they validate, refuse, warn about, and how grace runs out.
use super::*;
use crate::license::plan::{admit_with, entitled_with, notices, Counts, PlanLimits, Resource};

fn plan() -> PlanLimits {
    PlanLimits {
        plan: "team".into(),
        deployment: Some("installed".into()),
        max_users: Some(5),
        max_servers: Some(10),
        features: vec!["recording".into(), "graph_3d".into()],
        grace_days: None,
    }
}
fn planned() -> Claims {
    let mut c = claims();
    c.info.limits = Some(plan());
    c
}
fn at(days: i64) -> chrono::DateTime<Utc> {
    today() + Duration::days(days)
}
/// The stored licence's status `days` after `today()` (the licence expires 2026-10-31).
fn stored(c: &Claims, days: i64) -> LicenseStatus {
    let d = Temp::new();
    let roots = d.roots();
    d.store(&sign(c));
    status_at(&d.0, &roots, at(days))
}

#[test]
fn a_licence_without_a_plan_limits_and_warns_about_nothing() {
    let s = stored(&claims(), 0);
    assert!(s.active && s.limits.is_none());
    for resource in [Resource::Users, Resource::Servers] {
        assert!(admit_with(&s, resource, 10_000).is_ok());
    }
    assert!(entitled_with(&s, "recording") && entitled_with(&s, "mec"));
    assert!(notices(&s, &Counts { users: 99, servers: 99 }).is_empty());
    // And it expires exactly as before: no grace.
    let expired = stored(&claims(), 27);
    assert_eq!(expired.validation_state, "license_expired");
    assert!(expired.license_id.is_none() && expired.grace_ends_at.is_none());
    assert!(admit_with(&expired, Resource::Servers, 10_000).is_ok());
}
#[test]
fn no_licence_trials_and_unverifiable_licences_never_refuse() {
    let d = Temp::new();
    let roots = d.roots();
    let trial = status_at(&d.0, &roots, today());
    let after_trial = status_at(&d.0, &roots, at(40));
    d.store(&sign(&planned()));
    std::fs::remove_file(&roots).unwrap();
    let unverifiable = status_at(&d.0, &roots, today());
    assert_eq!(unverifiable.validation_state, "verification_unavailable");
    for s in [trial, after_trial, unverifiable] {
        assert!(s.limits.is_none());
        assert!(admit_with(&s, Resource::Users, 10_000).is_ok());
        assert!(entitled_with(&s, "orchestration"));
    }
}
#[test]
fn a_plan_refuses_only_beyond_its_limits() {
    let s = stored(&planned(), 0);
    assert_eq!(s.limits, Some(plan()));
    assert!(admit_with(&s, Resource::Users, 4).is_ok());
    assert!(admit_with(&s, Resource::Users, 5).unwrap_err().contains("한도(5)"));
    assert!(admit_with(&s, Resource::Servers, 9).is_ok());
    assert!(admit_with(&s, Resource::Servers, 10).is_err());
    assert!(entitled_with(&s, "recording") && !entitled_with(&s, "mec"));
    let mut c = planned();
    c.info.limits.as_mut().unwrap().max_users = None;
    assert!(admit_with(&stored(&c, 0), Resource::Users, 1_000_000).is_ok());
}
#[test]
fn notices_say_what_is_at_or_over_its_limit_and_when_expiry_nears() {
    let mut far = planned();
    far.info.expires_at = Some("2027-10-01".into());
    let s = stored(&far, 0);
    assert!(notices(&s, &Counts { users: 1, servers: 1 }).is_empty());
    assert_eq!(notices(&s, &Counts { users: 5, servers: 11 }), ["users_at_limit", "servers_over_limit"]);
    // 26 days before the last valid day.
    assert_eq!(notices(&stored(&planned(), 0), &Counts::default()), ["expiring_soon"]);
}
#[test]
fn grace_keeps_an_expired_plan_working_then_stops_only_growth() {
    // Last valid day 2026-10-31 (day 26); grace 14 days to 2026-11-14 (day 40).
    let last = stored(&planned(), 26);
    assert_eq!(last.validation_state, "licensed");
    let first = stored(&planned(), 27);
    assert!(first.active);
    assert_eq!(first.validation_state, "grace");
    assert_eq!(first.grace_ends_at.as_deref(), Some("2026-11-14"));
    assert_eq!(first.grace_days_remaining, Some(13));
    assert!(admit_with(&first, Resource::Users, 0).is_ok());
    assert_eq!(notices(&first, &Counts::default()), ["grace"]);
    let final_day = stored(&planned(), 40);
    assert_eq!((final_day.validation_state.as_str(), final_day.grace_days_remaining), ("grace", Some(0)));
    let ended = stored(&planned(), 41);
    assert!(!ended.active);
    assert_eq!(ended.validation_state, "license_expired");
    assert_eq!(ended.license_id.as_deref(), Some("test-license-1"));
    assert_eq!(ended.features, features_for_tier("free"));
    // Growth stops; what was licensed stays usable.
    assert!(admit_with(&ended, Resource::Users, 0).unwrap_err().contains("유예 기간"));
    assert!(admit_with(&ended, Resource::Servers, 0).is_err());
    assert!(entitled_with(&ended, "recording"));
    assert_eq!(notices(&ended, &Counts::default()), ["grace_ended"]);
}
#[test]
fn the_grace_period_is_the_licences_to_set() {
    let mut c = planned();
    c.info.limits.as_mut().unwrap().grace_days = Some(0);
    assert_eq!(stored(&c, 27).validation_state, "license_expired");
    c.info.limits.as_mut().unwrap().grace_days = Some(90);
    assert_eq!(stored(&c, 26 + 90).validation_state, "grace");
}
#[test]
fn activation_still_refuses_an_expired_planned_licence() {
    assert!(verify(&sign(&planned()), &keys(), at(27).date_naive()).err().unwrap().contains("expired"));
    assert!(verify(&sign(&planned()), &keys(), today().date_naive()).is_ok());
}
#[test]
fn invalid_plans_are_refused_with_the_signature() {
    let edits: Vec<fn(&mut PlanLimits)> = vec![
        |p| p.plan = "Team!".into(),
        |p| p.plan = String::new(),
        |p| p.deployment = Some("cloud".into()),
        |p| p.max_users = Some(0),
        |p| p.max_servers = Some(11),
        |p| p.grace_days = Some(91),
        |p| p.features = vec!["recording".into(), "recording".into()],
        |p| p.features = vec!["Bad Feature".into()],
    ];
    for edit in edits {
        let mut c = planned();
        edit(c.info.limits.as_mut().unwrap());
        assert!(verify(&sign(&c), &keys(), today().date_naive()).is_err());
    }
    // A plan sets its own server limit, beyond the tier's fixed ones; a newer
    // issuer's feature names are kept and grant nothing here.
    let mut c = planned();
    c.info.max_servers = Some(500);
    let p = c.info.limits.as_mut().unwrap();
    p.max_servers = Some(500);
    p.features.push("future_feature".into());
    assert!(verify(&sign(&c), &keys(), today().date_naive()).is_ok());
}
#[test]
fn an_unknown_plan_field_is_refused_like_any_other() {
    let mut value = serde_json::to_value(planned()).unwrap();
    value["info"]["limits"]["max_cpus"] = 4.into();
    let payload = serde_json::to_vec(&value).unwrap();
    let signature = issuer().sign(&signed_message("test-issuer", &payload));
    let key = encode(&Envelope {
        version: 1,
        key_id: "test-issuer".into(),
        payload: hex::encode(payload),
        signature: hex::encode(signature.to_bytes()),
    });
    assert!(verify(&key, &keys(), today().date_naive()).is_err());
}
