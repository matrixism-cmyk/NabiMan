#[path = "share_admission_tests.rs"]
mod admission_tests;

use super::*;
use crate::models::{AccountStatus, UserRole};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};

struct Fixture {
    directory: PathBuf,
    shares: ShareStore,
    users: UserStore,
    sessions: PtyStore,
    link: ShareLink,
}
impl Fixture {
    fn new() -> Self {
        let directory =
            std::env::temp_dir().join(format!("nabiman-shares-test-{}", uuid::Uuid::new_v4()));
        let shares =
            super::super::share_store::open(directory.join("terminal_shares.json")).unwrap();
        let owner: NabimanUser = serde_json::from_value(serde_json::json!({"id":"owner-immutable-id","share_authorization_version":"fixture-version","username":"owner","password_hash":"fixture","role":"admin","created_at":"now"})).unwrap();
        let users = Arc::new(Mutex::new(vec![owner]));
        let link = ShareLink {
            token: generate_token().unwrap(),
            session_id: "terminal-one".into(),
            owner: "owner".into(),
            owner_id: "owner-immutable-id".into(),
            owner_authorization_version: "fixture-version".into(),
            label: "fixture".into(),
            created_unix: now_unix(),
            expires_unix: now_unix() + 3600,
            password_hash: None,
            read_only: true,
            last_used_unix: 0,
            needs_password: false,
            alive: true,
            usable: true,
            files: Default::default(),
            server_id: String::new(),
            server_binding: String::new(),
        };
        shares
            .lock()
            .unwrap()
            .update(|m| {
                m.insert(link.token.clone(), link.clone());
                Ok(())
            })
            .unwrap();
        let session: super::super::session::TmuxSession = serde_json::from_value(serde_json::json!({"tmux_name":"fixture-tmux","owner":"owner","owner_id":"owner-immutable-id","label":"local","timeout_secs":3600})).unwrap();
        let sessions = Arc::new(Mutex::new(HashMap::from([(
            link.session_id.clone(),
            session,
        )])));
        Self {
            directory,
            shares,
            users,
            sessions,
            link,
        }
    }
    fn grant(&self) -> ShareGrant {
        ShareGrant {
            link: self.link.clone(),
            tmux_name: "fixture-tmux".into(),
            shares: self.shares.clone(),
            users: self.users.clone(),
            sessions: self.sessions.clone(),
        }
    }
    fn change(&self, change: impl FnOnce(&mut ShareLink)) {
        self.shares
            .lock()
            .unwrap()
            .update(|m| {
                change(m.get_mut(&self.link.token).unwrap());
                Ok(())
            })
            .unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

#[test]
fn live_grant_is_revoked_and_published_registry_removes_it() {
    let fixture = Fixture::new();
    let grant = fixture.grant();
    assert!(grant.valid());
    fixture
        .sessions
        .lock()
        .unwrap()
        .get_mut(&fixture.link.session_id)
        .unwrap()
        .owner = "historical-display-name".into();
    assert!(grant.valid());
    fixture
        .shares
        .lock()
        .unwrap()
        .update(|m| {
            m.remove(&fixture.link.token);
            Ok(())
        })
        .unwrap();
    assert!(!grant.valid());
    let stored: HashMap<String, ShareLink> = serde_json::from_slice(
        &std::fs::read(fixture.directory.join("terminal_shares.json")).unwrap(),
    )
    .unwrap();
    assert!(stored.is_empty());
}

#[test]
fn owner_demotion_suspension_deletion_and_recreated_username_deny_existing_grant() {
    let fixture = Fixture::new();
    let grant = fixture.grant();
    fixture.users.lock().unwrap()[0].role = UserRole::Viewer;
    assert!(!grant.valid());
    fixture.users.lock().unwrap()[0].role = UserRole::Admin;
    fixture.users.lock().unwrap()[0].status = AccountStatus::Suspended;
    assert!(!grant.valid());
    fixture.users.lock().unwrap()[0].status = AccountStatus::Active;
    fixture.users.lock().unwrap()[0].id = "new-owner-same-username".into();
    assert!(!grant.valid());
    fixture.users.lock().unwrap().clear();
    assert!(!grant.valid());
}

#[test]
fn legacy_missing_owner_id_and_unbounded_expiry_are_not_admitted() {
    let fixture = Fixture::new();
    fixture.change(|l| l.owner_id.clear());
    assert!(!fixture.grant().valid());
    fixture.change(|l| {
        l.owner_id = fixture.link.owner_id.clone();
        l.expires_unix = 0;
    });
    assert!(!fixture.grant().valid());
    assert!(
        !public_view(
            fixture
                .shares
                .lock()
                .unwrap()
                .get(&fixture.link.token)
                .unwrap()
        )
        .usable
    );
}
#[test]
fn missing_or_different_terminal_owner_id_denies_even_when_display_name_matches() {
    let fixture = Fixture::new();
    let grant = fixture.grant();
    assert!(grant.valid());
    for id in ["", "replacement-owner-id"] {
        fixture
            .sessions
            .lock()
            .unwrap()
            .get_mut(&fixture.link.session_id)
            .unwrap()
            .owner_id = id.into();
        assert!(!grant.valid());
    }
}

#[test]
fn expiry_boundary_clock_rollback_and_retargeted_connection_deny_access() {
    let fixture = Fixture::new();
    fixture.change(|l| l.expires_unix = now_unix());
    assert!(live_link(&fixture.shares, &fixture.users, &fixture.link.token).is_none());
    fixture.change(|l| {
        l.created_unix = now_unix() + 10;
        l.expires_unix = now_unix() + 100;
    });
    assert!(live_link(&fixture.shares, &fixture.users, &fixture.link.token).is_none());
    fixture.change(|l| *l = fixture.link.clone());
    fixture
        .sessions
        .lock()
        .unwrap()
        .get_mut(&fixture.link.session_id)
        .unwrap()
        .tmux_name = "another-tmux".into();
    assert!(!fixture.grant().valid());
    fixture.sessions.lock().unwrap().clear();
    assert!(!fixture.grant().valid());
}

#[test]
fn changed_password_readonly_expiry_or_target_invalidates_old_attachment() {
    let fixture = Fixture::new();
    let grant = fixture.grant();
    fixture.change(|l| l.read_only = false);
    assert!(!grant.valid());
    fixture.change(|l| {
        *l = fixture.link.clone();
        l.password_hash = Some("changed-hash".into());
    });
    assert!(!grant.valid());
    fixture.change(|l| {
        *l = fixture.link.clone();
        l.expires_unix += 1;
    });
    assert!(!grant.valid());
    fixture.change(|l| {
        *l = fixture.link.clone();
        l.session_id = "another-session".into();
    });
    assert!(!grant.valid());
}

#[test]
fn new_request_defaults_to_readonly_and_public_view_omits_password_hash() {
    let request: CreateShareRequest =
        serde_json::from_value(serde_json::json!({"expires_in_secs":3600})).unwrap();
    assert!(request.read_only);
    let fixture = Fixture::new();
    let mut link = fixture.link.clone();
    link.password_hash = Some("private-hash".into());
    let value = serde_json::to_value(public_view(link)).unwrap();
    assert_eq!(value["needs_password"], true);
    assert_eq!(value["password_hash"], serde_json::Value::Null);
}

#[test]
fn owner_generation_change_permanently_denies_old_share_and_pending_snapshot() {
    let fixture = Fixture::new();
    assert!(fixture.grant().valid());
    let old_version = fixture.users.lock().unwrap()[0]
        .share_authorization_version
        .clone();
    {
        let mut users = fixture.users.lock().unwrap();
        users[0].role = UserRole::Viewer;
        users[0].share_authorization_version = uuid::Uuid::new_v4().to_string();
    }
    assert!(!fixture.grant().valid());
    {
        let mut users = fixture.users.lock().unwrap();
        users[0].role = UserRole::Admin;
        users[0].share_authorization_version = uuid::Uuid::new_v4().to_string();
    }
    assert!(!fixture.grant().valid());
    assert!(live_link(&fixture.shares, &fixture.users, &fixture.link.token).is_none());
    let current = fixture.users.lock().unwrap()[0]
        .share_authorization_version
        .clone();
    assert_ne!(current, old_version);
    fixture.change(|l| l.owner_authorization_version = current);
    assert!(live_link(&fixture.shares, &fixture.users, &fixture.link.token).is_some());
    assert!(!fixture.grant().valid()); // old approval snapshot never upgrades itself
}

#[test]
fn legacy_link_without_owner_generation_is_unusable() {
    let fixture = Fixture::new();
    fixture.change(|l| l.owner_authorization_version.clear());
    assert!(!fixture.grant().valid());
    assert!(
        !public_view(
            fixture
                .shares
                .lock()
                .unwrap()
                .get(&fixture.link.token)
                .unwrap()
        )
        .usable
    );
}
