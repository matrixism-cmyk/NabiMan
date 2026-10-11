use super::*;
use actix_web::{body::MessageBody, dev::ServiceResponse, test::TestRequest};
use std::{fmt::Debug, future::Future};

pub(super) async fn verify<B, F, Fut, A, AddFut>(
    mode: &str,
    db: &mec::db::DbHandle,
    worker_state: &web::Data<mec::MecState>,
    worker_users: &users::UserStore,
    writes: &Arc<Mutex<Vec<String>>>,
    restore_source: &Arc<AtomicBool>,
    operations: &[membership_store::Operation],
    token: &str,
    send: F,
    add: A,
) where
    B: MessageBody,
    B::Error: Debug,
    F: Fn(TestRequest) -> Fut,
    Fut: Future<Output = ServiceResponse<B>>,
    A: Fn() -> AddFut,
    AddFut: Future<Output = ServiceResponse<B>>,
{
    let requested: i64 = db
        .lock()
        .unwrap()
        .query_row("SELECT COUNT(*) FROM requested_managed_grants", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(requested, 1);
    if mode == "managed_lost_create" {
        assert!(operations[0].binding_id.is_none());
        assert!(mec::managed_memberships::list(&db, None)
            .unwrap()
            .is_empty());
        assert_eq!(add().await.status(), 409);
        assert_eq!(writes.lock().unwrap().as_slice(), ["create"]);
        return;
    }
    if mode == "managed_auto_cleanup" {
        let inventory = || {
            actix_web::test::TestRequest::get()
                .uri("/api/mec/v1/tenants/tenant/pending-managed-grants")
                .insert_header(("Authorization", format!("Bearer {token}")))
        };
        let response = send(inventory()).await;
        assert_eq!(response.status(), 200);
        assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");
        let body: serde_json::Value = actix_web::test::read_body_json(response).await;
        assert_eq!(body["data"][0]["local_user_id"], "local");
        assert_eq!(
            mec::managed_memberships::sweep(&worker_state, &worker_users)
                .await
                .unwrap(),
            0
        );
        worker_users.lock().unwrap()[1].memberships[0].enabled = false;
        restore_source.store(true, Ordering::SeqCst);
        assert_eq!(
            mec::managed_memberships::sweep(&worker_state, &worker_users)
                .await
                .unwrap(),
            0
        );
        assert!(mec::managed_memberships::list(&db, None)
            .unwrap()
            .is_empty());
        restore_source.store(false, Ordering::SeqCst);
        worker_users.lock().unwrap()[1].memberships[0].enabled = false;
        assert_eq!(
            mec::managed_memberships::sweep(&worker_state, &worker_users)
                .await
                .unwrap(),
            0
        );
        assert!(mec::managed_memberships::list(&db, None)
            .unwrap()
            .is_empty());
        assert_eq!(writes.lock().unwrap().as_slice(), ["create"]);
        db.lock()
            .unwrap()
            .execute_batch("DROP TRIGGER reject_managed")
            .unwrap();
        assert_eq!(
            mec::managed_memberships::sweep(&worker_state, &worker_users)
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            mec::managed_memberships::list(&db, None).unwrap()[0].ownership_state,
            "revoked"
        );
        assert_eq!(writes.lock().unwrap().as_slice(), ["create", "delete"]);
        let response = send(inventory()).await;
        let body: serde_json::Value = actix_web::test::read_body_json(response).await;
        assert!(body["data"].as_array().unwrap().is_empty());
        return;
    }
    if mode == "managed_audit_failure" {
        assert!(mec::managed_memberships::list(&db, None)
            .unwrap()
            .is_empty());
        let reconcile = || {
            actix_web::test::TestRequest::post().uri(&format!("/api/mec/v1/tenants/tenant/membership-operations/{}/reconcile",operations[0].id)).insert_header(("Authorization",format!("Bearer {token}")))
            .set_json(serde_json::json!({"confirm_operation_id":operations[0].id,"expected_binding_id":operations[0].binding_id,"expected_updated_at":operations[0].updated_at,"reason":"Confirm exact managed creation candidate"}))
        };
        assert_eq!(send(reconcile()).await.status(), 500);
        assert!(mec::managed_memberships::list(&db, None)
            .unwrap()
            .is_empty());
        db.lock()
            .unwrap()
            .execute_batch("DROP TRIGGER reject_managed")
            .unwrap();
        assert_eq!(send(reconcile()).await.status(), 200);
    }
    let policy = mec::managed_memberships::list(&db, None).unwrap();
    assert_eq!(policy.len(), 1);
    assert_eq!(policy[0].local_user_id, "local");
    assert_eq!(add().await.status(), 409);
    if mode != "managed_source_revoked" && mode != "managed_tenant_changed" {
        assert_eq!(
            mec::managed_memberships::sweep(&worker_state, &worker_users)
                .await
                .unwrap(),
            0
        );
        worker_users.lock().unwrap()[1].memberships[0].enabled = false;
    }
    assert_eq!(
        mec::managed_memberships::sweep(&worker_state, &worker_users)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        mec::managed_memberships::list(&db, None).unwrap()[0].ownership_state,
        "revoked"
    );
    assert_eq!(writes.lock().unwrap().as_slice(), ["create", "delete"]);
    return;
}
