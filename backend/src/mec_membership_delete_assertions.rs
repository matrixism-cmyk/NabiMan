use super::*;
use actix_web::{body::MessageBody, dev::ServiceResponse, test::TestRequest};
use std::{fmt::Debug, future::Future};

pub(super) async fn verify<B, F, Fut>(
    db: &mec::db::DbHandle,
    worker_state: &web::Data<mec::MecState>,
    worker_users: &users::UserStore,
    writes: &Arc<Mutex<Vec<String>>>,
    provider_exists: &Arc<AtomicBool>,
    operations: &[membership_store::Operation],
    token: &str,
    send: F,
) where
    B: MessageBody,
    B::Error: Debug,
    F: Fn(TestRequest) -> Fut,
    Fut: Future<Output = ServiceResponse<B>>,
{
    use crate::models::{Membership, ScopeKind};
    let mut local = worker_users.lock().unwrap()[0].clone();
    local.id = "local".into();
    local.username = "local-alice".into();
    local.role = UserRole::Viewer;
    local.memberships = vec![Membership {
        kind: ScopeKind::Tenant,
        scope_id: "tenant".into(),
        role: UserRole::Viewer,
        enabled: true,
        allow_exec: None,
        expires_at: None,
    }];
    worker_users.lock().unwrap().push(local);
    db.lock().unwrap().execute("INSERT INTO external_identity_links(local_user_id,realm,remote_user_id,remote_username,verified_at,linked_by,reason) VALUES ('local',?1,'u-one','alice',?2,'admin','Verified fixture')",rusqlite::params![operations[0].realm,chrono::Utc::now().to_rfc3339()]).unwrap();
    let associate = || {
        actix_web::test::TestRequest::post().uri(&format!("/api/mec/v1/tenants/tenant/membership-operations/{}/associate",operations[0].id))
        .insert_header(("Authorization",format!("Bearer {token}"))).set_json(serde_json::json!({"local_user_id":"local","confirm_operation_id":operations[0].id,"reason":"Approve automatic revocation","automatic_revocation":true}))
    };
    let response = send(associate()).await;
    assert_eq!(response.status(), 201);
    assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");
    assert_eq!(send(associate()).await.status(), 409);
    assert_eq!(
        mec::managed_memberships::sweep(&worker_state, &worker_users)
            .await
            .unwrap(),
        0
    );
    worker_users.lock().unwrap()[1].memberships[0].enabled = false;
    assert_eq!(
        mec::managed_memberships::sweep(&worker_state, &worker_users)
            .await
            .unwrap(),
        0
    );
    assert_eq!(writes.lock().unwrap().as_slice(), ["create", "delete"]);
    let pending = membership_store::list(&db, "tenant")
        .unwrap()
        .into_iter()
        .find(|op| op.operation == "delete")
        .unwrap();
    assert_eq!(pending.state, "unknown_result");
    assert_eq!(
        mec::managed_memberships::sweep(&worker_state, &worker_users)
            .await
            .unwrap(),
        0
    );
    assert_eq!(writes.lock().unwrap().as_slice(), ["create", "delete"]);
    provider_exists.store(false, Ordering::SeqCst);
    assert_eq!(
        send(actix_web::test::TestRequest::post().uri(&format!("/api/mec/v1/tenants/tenant/membership-operations/{}/reconcile", pending.id)).insert_header(("Authorization", format!("Bearer {token}"))).set_json(serde_json::json!({"confirm_operation_id":pending.id,"expected_binding_id":pending.binding_id.clone().unwrap_or_default(),"expected_updated_at":pending.updated_at,"reason":"Verified provider inventory without replay"})))
            .await
            .status(),
        200
    );
    assert_eq!(
        mec::managed_memberships::list(&db, None).unwrap()[0].ownership_state,
        "revoked"
    );
    return;
}
