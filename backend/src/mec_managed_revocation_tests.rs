#[path = "mec_managed_revocation_fixture.rs"]
mod fixture;
use fixture::user;

#[path = "mec_managed_revocation_provider.rs"]
mod provider;

use super::*;
use crate::mec::{
    access_delegation,
    db::membership_store,
    services::{
        rancher_real::{RancherReal, RancherRealConfig},
        ServiceBundle,
    },
};
use crate::models::mec::{AllocationType, NodeAllocation, ResourceQuota, Tenant, TenantStatus};
use crate::models::{AccountStatus, Membership, NabimanUser, ScopeKind, UserRole};
use std::io::{Read, Write};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Mutex,
};
#[actix_web::test]
async fn scoped_managed_revocation_uses_real_auth_inventory_and_exact_durable_ownership() {
    for mode in [
        "normal",
        "grant_only",
        "stale_policy",
        "expired",
        "disabled",
        "peer",
        "global_admin",
        "foreign_member",
        "removed_member",
        "disabled_source",
        "suspended_source",
        "changed_project",
        "stale_binding",
        "stale_ownership",
        "no_confirmation",
        "missing_association",
        "unresolved",
        "wrong_identity",
        "wrong_project",
        "scope_revoked_during_io",
        "manager_revoked_during_io",
        "absent",
        "lost_delete",
        "accepted_delete",
        "readback_failure",
        "intent_audit_failure",
        "confirmation_audit_failure",
        "caller_logout",
    ] {
        let db = mec::db::open_in_memory().unwrap();
        let users = Arc::new(Mutex::new(vec![
            user("manager", UserRole::Admin),
            user("member", UserRole::Viewer),
            {
                let mut u = user("highest", UserRole::Viewer);
                u.role = UserRole::Admin;
                u.memberships.clear();
                u.scope_restricted = false;
                u
            },
        ]));
        let sessions = jwt_sessions::new_session_store();
        jwt_sessions::register_session(&sessions, "manager-sid", "manager", "manager", "127.0.0.1");
        jwt_sessions::register_session(&sessions, "root-sid", "highest", "highest", "127.0.0.1");
        let secret: auth::JwtSecret = Arc::new("scoped-revocation-fixture-secret".into());
        let provider = provider::start(mode, &db, &users, &sessions);
        let address = provider.address;
        let armed = provider.armed.clone();
        let exists = provider.exists.clone();
        let deletes = provider.deletes.clone();
        let realm = format!("http://{address}");
        let mut services = ServiceBundle::mocks();
        services.rancher = Arc::new(
            RancherReal::new(RancherRealConfig {
                base_url: realm.clone(),
                token: "fixture-only-key".into(),
                cluster_id: "c".into(),
                insecure_tls: false,
            })
            .unwrap(),
        );
        let state = web::Data::new(mec::MecState::new(db.clone(), services));
        let now = chrono::Utc::now();
        fixture::tenant(&state, now);
        db.lock().unwrap().execute("INSERT INTO external_identity_links(local_user_id,realm,remote_user_id,remote_username,verified_at,linked_by,reason) VALUES ('member',?1,'remote-member','remote-alice',?2,'highest','private highest identity reason')",rusqlite::params![realm,now.to_rfc3339()]).unwrap();
        // Seed approved ownership only after observation through the real adapter.
        let observed = state
            .services
            .rancher
            .list_prtb("c:p")
            .await
            .unwrap()
            .into_iter()
            .find(|b| b.id == "owned-one")
            .unwrap();
        let creation = membership_store::begin_managed(
            &db,
            "tenant",
            &realm,
            &observed,
            "highest",
            None,
            None,
            membership_store::ManagedIntent {
                delegation: None,
                local_user_id: "member",
                source_role: "viewer",
                reason: "Approved owned fixture binding",
            },
        )
        .unwrap();
        membership_store::observe(&db, &creation, "owned-one").unwrap();
        membership_store::complete(&db, &creation).unwrap();
        let policy = access_delegation::save(
            &db,
            "tenant",
            &access_delegation::UpdateRequest {
                expected_version: None,
                expected_realm: realm.clone(),
                expected_project_id: "c:p".into(),
                confirm_tenant_id: "tenant".into(),
                enabled: true,
                manager_ids: vec!["manager".into()],
                allowed_roles: vec!["read-only".into()],
                allowed_actions: if mode == "grant_only" {
                    vec!["grant".into()]
                } else {
                    vec!["grant".into(), "revoke".into()]
                },
                expires_at: Some((now + chrono::Duration::days(1)).to_rfc3339()),
                reason: "Approved direct binding revocation".into(),
                acknowledge_project_role_scope: true,
            },
            "highest",
            None,
            None,
        )
        .unwrap();
        let token_for = |name: &str, sid: &str, role: &str| {
            jsonwebtoken::encode(
                &jsonwebtoken::Header::default(),
                &auth::Claims {
                    sub: name.into(),
                    sid: sid.into(),
                    role: role.into(),
                    iat: now.timestamp() as usize,
                    exp: (now.timestamp() + 300) as usize,
                },
                &jsonwebtoken::EncodingKey::from_secret(secret.as_bytes()),
            )
            .unwrap()
        };
        let token = token_for("manager", "manager-sid", "viewer");
        let root_token = token_for("highest", "root-sid", "admin");
        let app = actix_web::test::init_service(
            App::new()
                .wrap(AuthCheck::new(secret, sessions.clone(), users.clone()))
                .app_data(web::Data::new(users.clone()))
                .app_data(web::Data::new(sessions))
                .app_data(state.clone())
                .configure(mec::config),
        )
        .await;
        let inventory = || {
            actix_web::test::TestRequest::get()
                .uri("/api/mec/v1/tenants/tenant/scoped-managed-memberships")
                .insert_header(("Authorization", format!("Bearer {token}")))
                .to_request()
        };
        let initial = actix_web::test::call_service(&app, inventory()).await;
        assert_eq!(initial.status(), 200, "{mode}");
        assert_eq!(initial.headers().get("cache-control").unwrap(), "no-store");
        let initial: serde_json::Value = actix_web::test::read_body_json(initial).await;
        let row = &initial["data"][0];
        assert_eq!(initial["data"].as_array().unwrap().len(), 1);
        assert_eq!(row["can_revoke"], mode != "grant_only");
        assert_eq!(row["username"], "member");
        assert!(!initial.to_string().contains("private highest"));
        assert!(row.get("source_ip").is_none());
        assert!(row.get("actor").is_none());
        let mut body = serde_json::json!({"confirm_operation_id":creation.id,"expected_binding_id":"owned-one","expected_updated_at":row["ownership_updated_at"],"expected_delegation_version":policy.version,"reason":"Review the exact owned direct binding only","acknowledge_direct_binding_only":true});
        match mode {
            "stale_policy" => body["expected_delegation_version"] = serde_json::json!("stale"),
            "expired" => {
                db.lock().unwrap().execute("UPDATE tenant_access_delegations SET expires_at='2000-01-01T00:00:00Z' WHERE tenant_id='tenant'",[]).unwrap();
            }
            "disabled" => {
                db.lock()
                    .unwrap()
                    .execute(
                        "UPDATE tenant_access_delegations SET enabled=0 WHERE tenant_id='tenant'",
                        [],
                    )
                    .unwrap();
            }
            "peer" => users.lock().unwrap()[1].memberships[0].role = UserRole::Admin,
            "global_admin" => users.lock().unwrap()[1].role = UserRole::Admin,
            "foreign_member" => users.lock().unwrap()[1].memberships[0].scope_id = "foreign".into(),
            "removed_member" => users.lock().unwrap()[1].memberships.clear(),
            "disabled_source" => users.lock().unwrap()[1].memberships[0].enabled = false,
            "suspended_source" => users.lock().unwrap()[1].status = AccountStatus::Suspended,
            "changed_project" => {
                db.lock()
                    .unwrap()
                    .execute(
                        "UPDATE tenant_profiles SET rancher_project_id='c:other' WHERE id='tenant'",
                        [],
                    )
                    .unwrap();
            }
            "stale_binding" => body["expected_binding_id"] = serde_json::json!("different-binding"),
            "stale_ownership" => body["expected_updated_at"] = serde_json::json!("old"),
            "no_confirmation" => body["acknowledge_direct_binding_only"] = serde_json::json!(false),
            "missing_association" => {
                db.lock()
                    .unwrap()
                    .execute("DELETE FROM managed_project_bindings", [])
                    .unwrap();
            }
            "unresolved" => {
                let op = membership_store::begin(
                    &db, "tenant", &realm, &observed, "delete", "highest", None, None,
                )
                .unwrap();
                membership_store::unknown(&db, &op, "Existing uncertain change").unwrap();
            }
            "absent" => exists.store(false, Ordering::SeqCst),
            "intent_audit_failure" => {
                db.lock().unwrap().execute_batch("CREATE TRIGGER reject_revoke_intent BEFORE INSERT ON audit_logs WHEN NEW.action='tenant_member_delete_managed_revoke_request' BEGIN SELECT RAISE(ABORT,'intent audit fault'); END;").unwrap();
            }
            _ => (),
        }
        armed.store(true, Ordering::SeqCst);
        let revoke = || {
            actix_web::test::TestRequest::post()
                .uri(&format!(
                    "/api/mec/v1/tenants/tenant/managed-memberships/{}/revoke",
                    creation.id
                ))
                .insert_header(("Authorization", format!("Bearer {token}")))
                .set_json(&body)
                .to_request()
        };
        let response = actix_web::test::call_service(&app, revoke()).await;
        let status = response.status().as_u16();
        let raw = actix_web::test::read_body(response).await;
        assert!(!String::from_utf8_lossy(&raw).contains("provider-sensitive-marker"));
        let expected = match mode {
            "normal" | "disabled_source" | "suspended_source" | "absent" => 200,
            "no_confirmation" => 400,
            "missing_association" => 404,
            "stale_policy" | "stale_binding" | "stale_ownership" | "changed_project"
            | "wrong_identity" | "unresolved" => 409,
            "wrong_project"
            | "lost_delete"
            | "accepted_delete"
            | "readback_failure"
            | "confirmation_audit_failure" => 502,
            "intent_audit_failure" => 500,
            "manager_revoked_during_io" | "caller_logout" => 401,
            _ => 403,
        };
        assert_eq!(status, expected, "{mode}");
        let operations = membership_store::list(&db, "tenant").unwrap();
        let dispatched = matches!(
            mode,
            "normal"
                | "disabled_source"
                | "suspended_source"
                | "lost_delete"
                | "accepted_delete"
                | "readback_failure"
                | "confirmation_audit_failure"
                | "caller_logout"
        );
        assert_eq!(
            deletes.load(Ordering::SeqCst),
            usize::from(dispatched),
            "{mode}"
        );
        let uncertain = matches!(
            mode,
            "lost_delete" | "accepted_delete" | "readback_failure" | "confirmation_audit_failure"
        );
        if dispatched || mode == "absent" {
            assert_eq!(operations.len(), 2, "{mode}");
            let deletion = operations.iter().find(|o| o.operation == "delete").unwrap();
            assert_eq!(
                deletion.state,
                if uncertain {
                    "unknown_result"
                } else {
                    "completed"
                }
            );
            assert_eq!(
                mec::managed_memberships::list(&db, None).unwrap()[0].ownership_state,
                if uncertain { "active" } else { "revoked" }
            );
            assert_eq!(db.lock().unwrap().query_row("SELECT COUNT(*) FROM delegated_membership_revocations WHERE operation_id=?1 AND manager_id='manager'",[&deletion.id],|r|r.get::<_,i64>(0)).unwrap(),1);
            if status == 200 {
                let receipt: serde_json::Value = serde_json::from_slice(&raw).unwrap();
                assert_eq!(receipt["data"]["direct_binding_absence_confirmed"], true);
                assert_eq!(receipt["data"]["all_external_access_revoked"], false);
                assert_eq!(
                    receipt["data"]["provider_delete_dispatched"],
                    mode != "absent"
                );
            }
            if uncertain {
                assert_eq!(
                    actix_web::test::call_service(&app, revoke()).await.status(),
                    409,
                    "{mode}"
                );
                assert_eq!(deletes.load(Ordering::SeqCst), 1);
                armed.store(false, Ordering::SeqCst);
                exists.store(false, Ordering::SeqCst);
                if mode == "confirmation_audit_failure" {
                    db.lock()
                        .unwrap()
                        .execute_batch("DROP TRIGGER reject_revoke_confirm")
                        .unwrap();
                }
                let reconciled=actix_web::test::call_service(&app,actix_web::test::TestRequest::post().uri(&format!("/api/mec/v1/tenants/tenant/membership-operations/{}/reconcile",deletion.id)).insert_header(("Authorization",format!("Bearer {root_token}"))).set_json(serde_json::json!({"confirm_operation_id":deletion.id,"expected_binding_id":"owned-one","expected_updated_at":deletion.updated_at,"reason":"Read-only confirmation after uncertain deletion"})).to_request()).await;
                assert_eq!(reconciled.status(), 200, "{mode}");
                assert_eq!(deletes.load(Ordering::SeqCst), 1);
                assert_eq!(
                    mec::managed_memberships::list(&db, None).unwrap()[0].ownership_state,
                    "revoked"
                );
            } else if mode != "caller_logout" {
                assert_eq!(
                    actix_web::test::call_service(&app, revoke()).await.status(),
                    409
                );
                assert_eq!(deletes.load(Ordering::SeqCst), usize::from(dispatched));
            }
        } else {
            assert_eq!(
                operations.len(),
                if mode == "unresolved" { 2 } else { 1 },
                "{mode}"
            );
            assert_eq!(
                db.lock()
                    .unwrap()
                    .query_row(
                        "SELECT COUNT(*) FROM delegated_membership_revocations",
                        [],
                        |r| r.get::<_, i64>(0)
                    )
                    .unwrap(),
                0,
                "{mode}"
            );
        }
        if !matches!(mode, "manager_revoked_during_io" | "caller_logout") {
            let list = actix_web::test::call_service(&app, inventory()).await;
            assert_eq!(list.status(), 200, "{mode}");
            let list: serde_json::Value = actix_web::test::read_body_json(list).await;
            if matches!(
                mode,
                "peer"
                    | "global_admin"
                    | "foreign_member"
                    | "removed_member"
                    | "missing_association"
            ) {
                assert!(list["data"].as_array().unwrap().is_empty(), "{mode}");
            }
        }
    }
}
