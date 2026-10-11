#[path = "mec_membership_delete_assertions.rs"]
mod delete_assertions;

#[path = "mec_membership_intent_fixture.rs"]
mod fixture;

#[path = "mec_membership_managed_assertions.rs"]
mod managed_assertions;

#[path = "mec_membership_intent_provider.rs"]
mod provider;

use super::*;
use crate::mec::{
    db::membership_store,
    services::{
        rancher_real::{RancherReal, RancherRealConfig},
        ServiceBundle,
    },
};
use crate::models::mec::{AllocationType, NodeAllocation, ResourceQuota, Tenant, TenantStatus};
use crate::models::{AccountStatus, NabimanUser, UserRole};
use std::io::{Read, Write};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
#[actix_web::test]
async fn actual_membership_handlers_confirm_writes_and_preserve_uncertain_outcomes() {
    for mode in [
        "normal",
        "lost_create",
        "wrong_identity",
        "readback_failure",
        "confirmation_audit_failure",
        "caller_logout",
        "accepted_delete",
        "automatic_delete_unknown",
        "managed_create",
        "managed_audit_failure",
        "managed_source_revoked",
        "managed_role_ceiling",
        "managed_missing_identity",
        "managed_disabled_remote",
        "managed_lost_create",
        "managed_identity_changed",
        "managed_tenant_changed",
        "managed_auto_cleanup",
        "managed_delegated",
        "managed_delegated_no_policy",
        "managed_delegated_stale",
        "managed_delegated_expired",
        "managed_delegated_foreign_manager",
        "managed_delegated_peer",
        "managed_delegated_self",
        "managed_delegated_foreign_target",
        "managed_delegated_missing_target",
        "managed_delegated_scope_revoked",
        "managed_delegated_manager_revoked",
        "managed_delegated_intent_audit_failure",
    ] {
        let fixture::Fixture {
            secret,
            sessions,
            users,
            db,
            state,
            token,
            delegation_version,
            provider,
        } = fixture::setup(mode);
        let address = provider.address;
        let restore_source = provider.restore_source.clone();
        let revoke_on_inventory = provider.revoke_on_inventory.clone();
        let writes = provider.writes.clone();
        let provider_exists = provider.provider_exists.clone();
        let test_sessions = sessions.clone();
        let worker_users = users.clone();
        let worker_state = state.clone();
        let app = actix_web::test::init_service(
            App::new()
                .wrap(AuthCheck::new(secret, sessions.clone(), users.clone()))
                .app_data(web::Data::new(users))
                .app_data(web::Data::new(sessions))
                .app_data(state)
                .configure(mec::config),
        )
        .await;
        let add = || {
            let (path, body) = if mode.starts_with("managed_") {
                (
                    "/api/mec/v1/tenants/tenant/managed-memberships",
                    serde_json::json!({"expected_delegation_version":if mode=="managed_delegated_stale" { Some("stale".to_string()) } else { delegation_version.clone() },"expected_realm":format!("http://{address}"),"expected_remote_user_id":if mode == "managed_identity_changed" { "u-foreign" } else { "u-one" },"local_user_id":if mode=="managed_delegated_self" {"admin"} else if mode=="managed_delegated_missing_target" {"missing"} else {"local"},"role_template":if mode == "managed_role_ceiling" { "project-owner" } else { "read-only" },"reason":"Approve managed access with automatic revocation","automatic_revocation":true,"acknowledge_project_role_scope":true}),
                )
            } else {
                (
                    "/api/mec/v1/tenants/tenant/members",
                    serde_json::json!({"username":"alice","role":"read-only","create_if_missing":false}),
                )
            };
            actix_web::test::TestRequest::post()
                .uri(path)
                .insert_header(("Authorization", format!("Bearer {token}")))
                .set_json(body)
                .to_request()
        };
        let response = actix_web::test::call_service(&app, add()).await;
        let status = response.status().as_u16();
        let body = actix_web::test::read_body(response).await;
        if mode.starts_with("managed_delegated") {
            let expected = match mode {
                "managed_delegated" => 201,
                "managed_delegated_stale" => 409,
                "managed_delegated_intent_audit_failure" => 500,
                "managed_delegated_manager_revoked" => 401,
                _ => 403,
            };
            assert_eq!(status, expected, "{mode}");
            let operations = membership_store::list(&db, "tenant").unwrap();
            if expected == 201 {
                assert_eq!(operations.len(), 1);
                assert_eq!(operations[0].state, "completed");
                assert_eq!(writes.lock().unwrap().as_slice(), ["create"]);
                let version:String=db.lock().unwrap().query_row("SELECT delegation_version FROM requested_delegated_grants WHERE operation_id=?1",[&operations[0].id],|r|r.get(0)).unwrap();
                assert_eq!(Some(version), delegation_version);
                let context = actix_web::test::call_service(
                    &app,
                    actix_web::test::TestRequest::get()
                        .uri("/api/mec/v1/tenants/tenant/delegated-access-context")
                        .insert_header(("Authorization", format!("Bearer {token}")))
                        .to_request(),
                )
                .await;
                assert_eq!(context.status(), 200);
                assert_eq!(context.headers().get("cache-control").unwrap(), "no-store");
                let context: serde_json::Value = actix_web::test::read_body_json(context).await;
                assert_eq!(context["data"]["candidates"].as_array().unwrap().len(), 1);
                assert_eq!(context["data"]["candidates"][0]["id"], "local");
            } else {
                assert!(operations.is_empty(), "{mode}");
                assert!(writes.lock().unwrap().is_empty(), "{mode}");
            }
            let denied = actix_web::test::call_service(
                &app,
                actix_web::test::TestRequest::get()
                    .uri("/api/mec/v1/tenants/tenant/access-delegation")
                    .insert_header(("Authorization", format!("Bearer {token}")))
                    .to_request(),
            )
            .await;
            assert_eq!(denied.status(), 403, "{mode}");
            continue;
        }
        let expected = match mode {
            "normal" | "accepted_delete" | "automatic_delete_unknown" => 201,
            "managed_create" => 201,
            "managed_source_revoked" | "managed_tenant_changed" => 202,
            "managed_role_ceiling"
            | "managed_missing_identity"
            | "managed_disabled_remote"
            | "managed_identity_changed" => 409,
            "confirmation_audit_failure" => 500,
            "caller_logout" => 401,
            _ => 502,
        };
        assert_eq!(status, expected, "{mode}");
        assert!(!String::from_utf8_lossy(&body).contains("provider-sensitive-marker"));
        let operations = membership_store::list(&db, "tenant").unwrap();
        if expected == 409 {
            assert!(operations.is_empty());
            assert!(writes.lock().unwrap().is_empty());
            assert!(mec::managed_memberships::list(&db, None)
                .unwrap()
                .is_empty());
            continue;
        }

        assert_eq!(operations.len(), 1);
        assert_eq!(
            operations[0].state,
            if expected == 201 || expected == 202 || mode == "caller_logout" {
                "completed"
            } else {
                "unknown_result"
            }
        );
        let owned: i64 = db
            .lock()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM owned_project_bindings", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(
            owned,
            if expected == 201 || expected == 202 || mode == "caller_logout" {
                1
            } else {
                0
            }
        );
        assert_eq!(writes.lock().unwrap().as_slice(), ["create"]);
        if mode.starts_with("managed_") {
            managed_assertions::verify(
                mode,
                &db,
                &worker_state,
                &worker_users,
                &writes,
                &restore_source,
                &operations,
                &token,
                |request| actix_web::test::call_service(&app, request.to_request()),
                || actix_web::test::call_service(&app, add()),
            )
            .await;
            continue;
        }
        if mode == "caller_logout" {
            assert!(body.is_empty());
            continue;
        }
        let retry = actix_web::test::call_service(&app, add()).await;
        assert_eq!(
            retry.status(),
            if mode == "readback_failure" { 502 } else { 409 },
            "retry {mode}"
        );
        assert_eq!(writes.lock().unwrap().as_slice(), ["create"]);
        let reconcile = |operation: &membership_store::Operation| {
            actix_web::test::TestRequest::post().uri(&format!("/api/mec/v1/tenants/tenant/membership-operations/{}/reconcile",operation.id))
                .insert_header(("Authorization",format!("Bearer {token}")))
                .set_json(serde_json::json!({"confirm_operation_id":operation.id,"expected_binding_id":operation.binding_id.clone().unwrap_or_default(),"expected_updated_at":operation.updated_at,"reason":"Verified provider inventory without replay"})).to_request()
        };
        if mode == "confirmation_audit_failure" {
            let mut stale = operations[0].clone();
            stale.updated_at = "stale".into();
            assert_eq!(
                actix_web::test::call_service(&app, reconcile(&stale))
                    .await
                    .status(),
                409
            );
            let wrong_tenant = actix_web::test::TestRequest::post().uri(&format!("/api/mec/v1/tenants/foreign/membership-operations/{}/reconcile",operations[0].id))
                .insert_header(("Authorization",format!("Bearer {token}")))
                .set_json(serde_json::json!({"confirm_operation_id":operations[0].id,"expected_binding_id":operations[0].binding_id,"expected_updated_at":operations[0].updated_at,"reason":"Verified exact provider binding"})).to_request();
            assert_eq!(
                actix_web::test::call_service(&app, wrong_tenant)
                    .await
                    .status(),
                404
            );
            revoke_on_inventory.store(true, Ordering::SeqCst);
            assert_eq!(
                actix_web::test::call_service(&app, reconcile(&operations[0]))
                    .await
                    .status(),
                401
            );
            assert_eq!(
                membership_store::get(&db, "tenant", &operations[0].id)
                    .unwrap()
                    .state,
                "unknown_result"
            );
            revoke_on_inventory.store(false, Ordering::SeqCst);
            jwt_sessions::register_session(&test_sessions, "sid", "admin", "admin", "127.0.0.1");
        }
        let response = actix_web::test::call_service(&app, reconcile(&operations[0])).await;
        assert_eq!(
            response.status().as_u16(),
            match mode {
                "confirmation_audit_failure" => 200,
                "readback_failure" => 502,
                _ => 409,
            },
            "reconcile {mode}"
        );
        if mode == "confirmation_audit_failure" {
            assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");
            assert_eq!(
                membership_store::get(&db, "tenant", &operations[0].id)
                    .unwrap()
                    .state,
                "completed"
            );
            assert_eq!(
                actix_web::test::call_service(&app, reconcile(&operations[0]))
                    .await
                    .status(),
                409
            );
        }
        assert_eq!(writes.lock().unwrap().as_slice(), ["create"]);
        if mode == "automatic_delete_unknown" {
            delete_assertions::verify(
                &db,
                &worker_state,
                &worker_users,
                &writes,
                &provider_exists,
                &operations,
                &token,
                |request| actix_web::test::call_service(&app, request.to_request()),
            )
            .await;
            continue;
        }
        if expected == 201 {
            let delete = || {
                actix_web::test::TestRequest::delete()
                    .uri("/api/mec/v1/tenants/tenant/members/binding-one")
                    .insert_header(("Authorization", format!("Bearer {token}")))
                    .to_request()
            };
            let response = actix_web::test::call_service(&app, delete()).await;
            assert_eq!(
                response.status().as_u16(),
                if mode == "accepted_delete" { 502 } else { 204 }
            );
            let owned_state: String = db
                .lock()
                .unwrap()
                .query_row("SELECT state FROM owned_project_bindings", [], |r| r.get(0))
                .unwrap();
            assert_eq!(
                owned_state,
                if mode == "accepted_delete" {
                    "active"
                } else {
                    "revoked"
                }
            );
            if mode == "accepted_delete" {
                assert_eq!(
                    actix_web::test::call_service(&app, delete()).await.status(),
                    409
                );
                let pending = membership_store::list(&db, "tenant")
                    .unwrap()
                    .into_iter()
                    .find(|op| op.operation == "delete")
                    .unwrap();
                assert_eq!(
                    actix_web::test::call_service(&app, reconcile(&pending))
                        .await
                        .status(),
                    409
                );
                provider_exists.store(false, Ordering::SeqCst); // Deferred provider deletion now completed.
                assert_eq!(
                    actix_web::test::call_service(&app, reconcile(&pending))
                        .await
                        .status(),
                    200
                );
                let state: String = db
                    .lock()
                    .unwrap()
                    .query_row("SELECT state FROM owned_project_bindings", [], |r| r.get(0))
                    .unwrap();
                assert_eq!(state, "revoked");
            }
            assert_eq!(writes.lock().unwrap().as_slice(), ["create", "delete"]);
        }
    }
}
