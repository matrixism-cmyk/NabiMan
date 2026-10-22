//! Exercise the authenticated handler and the actual service mutation, not a
//! separate copy of its authorization predicate.
use super::*;
use crate::models::mec::{AllocationType, NodeAllocation, ResourceQuota, Tenant, TenantStatus};
use crate::models::{NabimanUser, UserRole};
use std::sync::{Arc, Mutex};

#[actix_web::test]
async fn member_delete_is_bound_to_the_url_tenant_project() {
    let secret: auth::JwtSecret = Arc::new("member-boundary-test-key".into());
    let sessions = jwt_sessions::new_session_store();
    jwt_sessions::register_session(&sessions, "test-sid", "admin", "admin", "127.0.0.1");
    let users = Arc::new(Mutex::new(vec![NabimanUser {
        share_authorization_version: uuid::Uuid::new_v4().to_string(),
        id: "admin".into(),
        username: "admin".into(),
        password_hash: "unused".into(),
        role: UserRole::Admin,
        status: Default::default(),
        memberships: vec![],
        requested_memberships: vec![],
        scope_restricted: false,
        created_at: "now".into(),
        last_login: None,
        totp_enabled: false,
        totp_secret: None,
        mfa: Default::default(),
        remote_permissions: Default::default(),
    }]));
    let state = web::Data::new(mec::MecState::mocks());
    let rancher = &state.services.rancher;
    let a = rancher.create_project("c", "a").await.unwrap();
    let b = rancher.create_project("c", "b").await.unwrap();
    let user = rancher.create_user("alice", "test-only").await.unwrap();
    let own = rancher
        .create_prtb(&a.id, &user.id, "project-member")
        .await
        .unwrap();
    let foreign = rancher
        .create_prtb(&b.id, &user.id, "read-only")
        .await
        .unwrap();
    for (id, project) in [("tenant-a", Some(a.id.clone())), ("unconfigured", None)] {
        let now = chrono::Utc::now();
        state
            .tenants
            .upsert(
                &Tenant {
                    id: id.into(),
                    display_name: id.into(),
                    task_name: None,
                    contact_email: None,
                    namespace: id.into(),
                    rancher_project_id: project,
                    rancher_user_id: None,
                    allocation: NodeAllocation {
                        alloc_type: AllocationType::Shared,
                        node: None,
                        gpu_label: None,
                        tier: None,
                    },
                    quota: ResourceQuota::standard(),
                    starter_kit: None,
                    created_at: now,
                    updated_at: now,
                    status: TenantStatus::Active,
                },
                "admin",
            )
            .unwrap();
    }
    let claims = auth::Claims {
        sub: "admin".into(),
        role: "admin".into(),
        sid: "test-sid".into(),
        iat: chrono::Utc::now().timestamp() as usize,
        exp: (chrono::Utc::now().timestamp() + 300) as usize,
    };
    let token = jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(secret.as_bytes()),
    )
    .unwrap();
    let app = actix_web::test::init_service(
        App::new()
            .wrap(AuthCheck::new(secret, sessions.clone(), users.clone()))
            .app_data(web::Data::new(users))
            .app_data(web::Data::new(sessions))
            .app_data(state.clone())
            .route(
                "/api/mec/v1/tenants/{tenant}/members/{binding}",
                web::delete().to(mec::handlers::members::remove),
            ),
    )
    .await;
    for (tenant, binding, status) in [
        ("tenant-a", foreign.id.as_str(), 404),
        ("missing", own.id.as_str(), 404),
        ("unconfigured", own.id.as_str(), 400),
        ("tenant-a", "missing-binding", 404),
        ("tenant-a", own.id.as_str(), 204),
        ("tenant-a", own.id.as_str(), 404),
    ] {
        let req = actix_web::test::TestRequest::delete()
            .uri(&format!("/api/mec/v1/tenants/{tenant}/members/{binding}"))
            .insert_header(("Authorization", format!("Bearer {token}")))
            .to_request();
        let response = actix_web::test::call_service(&app, req).await;
        assert_eq!(response.status().as_u16(), status, "{tenant}/{binding}");
        assert_eq!(rancher.list_prtb(&b.id).await.unwrap()[0].id, foreign.id);
        if status != 204 && binding != own.id {
            assert_eq!(rancher.list_prtb(&a.id).await.unwrap().len(), 1);
        }
    }
    assert!(rancher.list_prtb(&a.id).await.unwrap().is_empty());
}
