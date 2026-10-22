use super::*;

pub(super) fn user(id: &str, role: UserRole) -> NabimanUser {
    NabimanUser {
        share_authorization_version: uuid::Uuid::new_v4().to_string(),
        id: id.into(),
        username: id.into(),
        password_hash: "unused".into(),
        role: UserRole::Viewer,
        status: AccountStatus::Active,
        memberships: vec![Membership {
            kind: ScopeKind::Tenant,
            scope_id: "tenant".into(),
            role,
            enabled: true,
            allow_exec: None,
            expires_at: None,
        }],
        requested_memberships: vec![],
        scope_restricted: true,
        created_at: "now".into(),
        last_login: None,
        totp_enabled: false,
        totp_secret: None,
        mfa: Default::default(),
        remote_permissions: Default::default(),
    }
}
pub(super) fn tenant(state: &web::Data<mec::MecState>, now: chrono::DateTime<chrono::Utc>) {
    state
        .tenants
        .upsert(
            &Tenant {
                id: "tenant".into(),
                display_name: "Tenant".into(),
                task_name: None,
                contact_email: None,
                namespace: "tenant".into(),
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
            "highest",
        )
        .unwrap();
}
