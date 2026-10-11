use super::*;
use crate::models::mec::{AllocationType, NodeAllocation, ResourceQuota, Tenant, TenantStatus};
use crate::models::{AccountStatus, Membership, NabimanUser, ScopeKind, UserRole};
use std::sync::{Arc, Mutex};
fn user(name: &str, role: UserRole, scope: Option<(&str, UserRole)>) -> NabimanUser {
    NabimanUser {
        share_authorization_version: uuid::Uuid::new_v4().to_string(),
        id: name.into(),
        username: name.into(),
        password_hash: "unused".into(),
        role,
        status: AccountStatus::Active,
        memberships: scope
            .map(|(id, role)| {
                vec![Membership {
                    kind: ScopeKind::Tenant,
                    scope_id: id.into(),
                    role,
                    enabled: true,
                    allow_exec: None,
                    expires_at: None,
                }]
            })
            .unwrap_or_default(),
        requested_memberships: vec![],
        scope_restricted: true,
        created_at: "now".into(),
        last_login: None,
        totp_enabled: false,
        totp_secret: None,
        mfa: Default::default(),
    }
}
#[actix_web::test]
async fn access_inspection_is_scoped_and_never_confuses_direct_binding_with_total_access() {
    let secret: auth::JwtSecret = Arc::new("access-plan-fixture".into());
    let sessions = jwt_sessions::new_session_store();
    let users = Arc::new(Mutex::new(vec![
        user("root", UserRole::Admin, None),
        user("manager", UserRole::Viewer, Some(("a", UserRole::Admin))),
        user("alice", UserRole::Viewer, Some(("a", UserRole::Viewer))),
        user("bob", UserRole::Viewer, Some(("b", UserRole::Viewer))),
        user("peer", UserRole::Viewer, Some(("a", UserRole::Admin))),
        user(
            "unlinked",
            UserRole::Viewer,
            Some(("a", UserRole::Operator)),
        ),
    ]));
    let tokens: Vec<_> = ["root", "manager", "alice"]
        .iter()
        .map(|name| {
            jwt_sessions::register_session(&sessions, name, name, name, "127.0.0.1");
            jsonwebtoken::encode(
                &jsonwebtoken::Header::default(),
                &auth::Claims {
                    sub: (*name).into(),
                    role: "admin".into(),
                    sid: (*name).into(),
                    iat: chrono::Utc::now().timestamp() as usize,
                    exp: (chrono::Utc::now().timestamp() + 300) as usize,
                },
                &jsonwebtoken::EncodingKey::from_secret(secret.as_bytes()),
            )
            .unwrap()
        })
        .collect();
    let state = web::Data::new(mec::MecState::mocks());
    let rancher = &state.services.rancher;
    let project = rancher.create_project("c", "a").await.unwrap();
    let other_project = rancher.create_project("c", "b").await.unwrap();
    let remote = rancher
        .create_user("different-external-name", "fixture-password")
        .await
        .unwrap();
    let unrelated = rancher
        .create_user("unrelated", "fixture-password")
        .await
        .unwrap();
    rancher
        .create_prtb(&project.id, &unrelated.id, "project-owner")
        .await
        .unwrap();
    rancher
        .create_prtb(&other_project.id, &remote.id, "project-owner")
        .await
        .unwrap();
    for (id, project_id) in [
        ("a", Some(project.id.clone())),
        ("b", Some(other_project.id)),
        ("no-project", None),
    ] {
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
                    rancher_project_id: project_id,
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
                "root",
            )
            .unwrap();
    }
    let app = actix_web::test::init_service(
        App::new()
            .wrap(AuthCheck::new(secret, sessions.clone(), users.clone()))
            .app_data(web::Data::new(users.clone()))
            .app_data(web::Data::new(sessions.clone()))
            .app_data(state.clone())
            .configure(external_identities::config)
            .configure(mec::config),
    )
    .await;
    let link=actix_web::test::TestRequest::post().uri("/api/users/alice/external-identities/rancher").insert_header(("Authorization",format!("Bearer {}",tokens[0]))).set_json(serde_json::json!({"realm":"mock://rancher","remote_user_id":remote.id,"reason":"Verified account ownership"})).to_request();
    assert_eq!(
        actix_web::test::call_service(&app, link).await.status(),
        201
    );
    for (tenant, target, token, status) in [
        ("a", "alice", 1, 200),
        ("a", "root", 1, 403),
        ("b", "bob", 1, 403),
        ("a", "bob", 1, 403),
        ("a", "peer", 1, 403),
        ("a", "manager", 1, 403),
        ("a", "alice", 2, 403),
        ("missing", "alice", 0, 404),
        ("a", "missing", 0, 404),
        ("no-project", "alice", 0, 200),
        ("a", "unlinked", 1, 200),
    ] {
        let req = actix_web::test::TestRequest::get()
            .uri(&format!(
                "/api/mec/v1/tenants/{tenant}/members/{target}/access-plan"
            ))
            .insert_header(("Authorization", format!("Bearer {}", tokens[token])))
            .to_request();
        let response = actix_web::test::call_service(&app, req).await;
        assert_eq!(response.status().as_u16(), status, "{tenant}/{target}");
        if status == 200 {
            assert_eq!(response.headers().get("Cache-Control").unwrap(), "no-store");
            let body: serde_json::Value = actix_web::test::read_body_json(response).await;
            assert_eq!(body["data"]["mutation_supported"], false);
            assert_eq!(
                body["data"]["unverified_access_sources"]
                    .as_array()
                    .unwrap()
                    .len(),
                5
            );
            assert!(body["data"]["direct_bindings"]
                .as_array()
                .unwrap()
                .is_empty());
            if target == "unlinked" {
                assert!(body["data"]["blockers"]
                    .as_array()
                    .unwrap()
                    .contains(&serde_json::json!("identity_unlinked")));
            }
        }
    }
    let binding = rancher
        .create_prtb(&project.id, &remote.id, "read-only")
        .await
        .unwrap();
    for (mode, source_allowed, matches) in [
        ("active", true, true),
        ("expired", false, false),
        ("disabled", false, false),
        ("suspended", false, false),
    ] {
        {
            let mut guard = users.lock().unwrap();
            let target = guard.iter_mut().find(|u| u.id == "alice").unwrap();
            target.status = if mode == "suspended" {
                AccountStatus::Suspended
            } else {
                AccountStatus::Active
            };
            target.memberships[0].enabled = mode != "disabled";
            target.memberships[0].expires_at = if mode == "expired" {
                Some((chrono::Utc::now() - chrono::Duration::seconds(1)).to_rfc3339())
            } else {
                None
            };
        }
        let req = actix_web::test::TestRequest::get()
            .uri("/api/mec/v1/tenants/a/members/alice/access-plan")
            .insert_header(("Authorization", format!("Bearer {}", tokens[0])))
            .to_request();
        let response = actix_web::test::call_service(&app, req).await;
        assert_eq!(response.status(), 200);
        let body: serde_json::Value = actix_web::test::read_body_json(response).await;
        assert_eq!(body["data"]["source_access_allowed"], source_allowed);
        assert_eq!(body["data"]["direct_project_role_matches"], matches);
        assert_eq!(body["data"]["direct_bindings"][0]["id"], binding.id);
        assert_eq!(body["data"]["direct_bindings"].as_array().unwrap().len(), 1);
    }
    // Reports never mutate the provider, even when local access is revoked.
    assert_eq!(rancher.list_prtb(&project.id).await.unwrap().len(), 2);
    rancher.delete_user(&remote.id).await.unwrap();
    let req = actix_web::test::TestRequest::get()
        .uri("/api/mec/v1/tenants/a/members/alice/access-plan")
        .insert_header(("Authorization", format!("Bearer {}", tokens[0])))
        .to_request();
    assert_eq!(actix_web::test::call_service(&app, req).await.status(), 404);
    sessions.lock().unwrap().remove("manager");
    let req = actix_web::test::TestRequest::get()
        .uri("/api/mec/v1/tenants/a/members/unlinked/access-plan")
        .insert_header(("Authorization", format!("Bearer {}", tokens[1])))
        .to_request();
    assert_eq!(actix_web::test::call_service(&app, req).await.status(), 401);
}

#[actix_web::test]
async fn inspection_discards_upstream_results_after_manager_scope_or_session_revocation() {
    use crate::mec::services::{
        rancher_real::{RancherReal, RancherRealConfig},
        ServiceBundle,
    };
    use std::io::{Read, Write};
    for mode in ["scope", "session"] {
        let secret: auth::JwtSecret = Arc::new("revocation-fixture-key".into());
        let sessions = jwt_sessions::new_session_store();
        let users = Arc::new(Mutex::new(vec![
            user("root", UserRole::Admin, None),
            user("manager", UserRole::Viewer, Some(("a", UserRole::Admin))),
            user("alice", UserRole::Viewer, Some(("a", UserRole::Viewer))),
        ]));
        let tokens: Vec<_> = ["root", "manager"]
            .iter()
            .map(|name| {
                jwt_sessions::register_session(&sessions, name, name, name, "127.0.0.1");
                jsonwebtoken::encode(
                    &jsonwebtoken::Header::default(),
                    &auth::Claims {
                        sub: (*name).into(),
                        role: "admin".into(),
                        sid: (*name).into(),
                        iat: chrono::Utc::now().timestamp() as usize,
                        exp: (chrono::Utc::now().timestamp() + 300) as usize,
                    },
                    &jsonwebtoken::EncodingKey::from_secret(secret.as_bytes()),
                )
                .unwrap()
            })
            .collect();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let revoke_users = users.clone();
        let revoke_sessions = sessions.clone();
        let server = std::thread::spawn(move || {
            for index in 0..3 {
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
                let mut socket = loop {
                    match listener.accept() {
                        Ok((socket, _)) => break socket,
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            assert!(std::time::Instant::now() < deadline);
                            std::thread::sleep(std::time::Duration::from_millis(5));
                        }
                        Err(e) => panic!("{e}"),
                    }
                };
                socket
                    .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                    .unwrap();
                let mut bytes = Vec::new();
                let mut buffer = [0; 1024];
                while !bytes.windows(4).any(|w| w == b"\r\n\r\n") {
                    let count = socket.read(&mut buffer).unwrap();
                    assert!(count > 0);
                    bytes.extend_from_slice(&buffer[..count]);
                }
                assert!(bytes.starts_with(b"GET "));
                if index == 1 {
                    if mode == "scope" {
                        revoke_users
                            .lock()
                            .unwrap()
                            .iter_mut()
                            .find(|u| u.id == "manager")
                            .unwrap()
                            .memberships
                            .clear();
                    } else {
                        revoke_sessions.lock().unwrap().remove("manager");
                    }
                }
                let body = if index == 2 {
                    r#"{"data":[]}"#
                } else {
                    r#"{"id":"u-1","username":"external-alice","enabled":true}"#
                };
                write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
            }
        });
        let mut services = ServiceBundle::mocks();
        services.rancher = Arc::new(
            RancherReal::new(RancherRealConfig {
                base_url: format!("http://{address}"),
                token: "fixture-only".into(),
                cluster_id: "c".into(),
                insecure_tls: false,
            })
            .unwrap(),
        );
        let state = web::Data::new(mec::MecState::new(
            mec::db::open_in_memory().unwrap(),
            services,
        ));
        let now = chrono::Utc::now();
        state
            .tenants
            .upsert(
                &Tenant {
                    id: "a".into(),
                    display_name: "A".into(),
                    task_name: None,
                    contact_email: None,
                    namespace: "a".into(),
                    rancher_project_id: Some("c:p".into()),
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
                "root",
            )
            .unwrap();
        let app = actix_web::test::init_service(
            App::new()
                .wrap(AuthCheck::new(secret, sessions.clone(), users.clone()))
                .app_data(web::Data::new(users))
                .app_data(web::Data::new(sessions))
                .app_data(state)
                .configure(external_identities::config)
                .configure(mec::config),
        )
        .await;
        let req=actix_web::test::TestRequest::post().uri("/api/users/alice/external-identities/rancher").insert_header(("Authorization",format!("Bearer {}",tokens[0]))).set_json(serde_json::json!({"realm":format!("http://{address}"),"remote_user_id":"u-1","reason":"Verified account ownership"})).to_request();
        assert_eq!(actix_web::test::call_service(&app, req).await.status(), 201);
        let req = actix_web::test::TestRequest::get()
            .uri("/api/mec/v1/tenants/a/members/alice/access-plan")
            .insert_header(("Authorization", format!("Bearer {}", tokens[1])))
            .to_request();
        let response = actix_web::test::call_service(&app, req).await;
        assert_eq!(response.status(), 401, "{mode}");
        assert!(actix_web::test::read_body(response).await.is_empty());
        server.join().unwrap();
    }
}
