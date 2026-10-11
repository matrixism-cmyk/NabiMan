#[path = "user_profile.rs"]
mod profile;
#[path = "users_storage.rs"]
mod storage;
use storage::*;
pub(crate) use storage::{commit_users, generate_id};

use crate::models::{ApiResponse, CreateUserRequest, NabimanUser, UpdateUserRequest, UserRole};
use actix_web::{web, HttpResponse};
use std::sync::{Arc, Mutex};

// API responses omit secrets; the on-disk representation must retain them.
#[derive(serde::Serialize)]
struct StoredUser<'a> {
    id: &'a str,
    username: &'a str,
    share_authorization_version: &'a str,
    password_hash: &'a str,
    role: &'a UserRole,
    status: &'a crate::models::AccountStatus,
    memberships: &'a [crate::models::Membership],
    requested_memberships: &'a [crate::models::Membership],
    scope_restricted: bool,
    created_at: &'a str,
    last_login: &'a Option<String>,
    totp_enabled: bool,
    totp_secret: &'a Option<String>,
    mfa: &'a crate::models::MfaState,
}

pub type UserStore = Arc<Mutex<Vec<NabimanUser>>>;

pub fn new_user_store(pw_hash: &str) -> UserStore {
    let path = users_file_path();
    match read_users_at(&path) {
        Ok(data) => {
            let mut users: Vec<NabimanUser> = serde_json::from_str(&data)
                .unwrap_or_else(|e| panic!("Invalid user database {}: {e}", path.display()));
            assert!(
                !users.is_empty(),
                "User database {} is empty",
                path.display()
            );
            let migrated = users.iter_mut().fold(false, |changed, user| {
                if user.share_authorization_version.is_empty() {
                    user.share_authorization_version = uuid::Uuid::new_v4().to_string();
                    true
                } else {
                    changed
                }
            });
            let replacement = std::env::var("NABIMAN_PASSWORD").ok();
            if rotate_published_admin_password(&mut users, replacement.as_deref())
                .unwrap_or_else(|e| panic!("{e}"))
                || migrated
            {
                save_users(&users).expect("Could not persist rotated admin password");
            }
            return Arc::new(Mutex::new(users));
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => panic!("Cannot read user database {}: {e}", path.display()),
    }
    // Migrate: create default admin from existing password hash
    assert!(
        !pw_hash.is_empty(),
        "Bootstrap password is required for initial admin creation"
    );
    let admin = NabimanUser {
        share_authorization_version: uuid::Uuid::new_v4().to_string(),
        id: generate_id(),
        username: "admin".into(),
        password_hash: pw_hash.to_string(),
        role: UserRole::Admin,
        created_at: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        last_login: None,
        totp_enabled: false,
        totp_secret: None,
        mfa: Default::default(),
        status: Default::default(),
        memberships: vec![],
        requested_memberships: vec![],
        scope_restricted: false,
    };
    let users = vec![admin];
    save_users(&users).expect("Failed to persist initial admin user");
    println!("Default admin user created from existing password");
    Arc::new(Mutex::new(users))
}

pub fn find_user_by_name(store: &UserStore, username: &str) -> Option<NabimanUser> {
    store
        .lock()
        .unwrap()
        .iter()
        .find(|u| u.username == username)
        .cloned()
}

pub fn update_password(
    store: &UserStore,
    user_id: &str,
    expected_hash: &str,
    new_hash: String,
) -> Result<(), String> {
    let mut users = store.lock().unwrap();
    let user = users
        .iter()
        .position(|u| u.id == user_id && u.password_hash == expected_hash)
        .ok_or_else(|| "User not found".to_string())?;
    let previous = users.clone();
    users[user].password_hash = new_hash;
    commit_users(&mut users, previous)
}

pub fn mutate_user<F, T>(store: &UserStore, user_id: &str, update: F) -> Result<T, String>
where
    F: FnOnce(&mut NabimanUser) -> Result<T, String>,
{
    let mut users = store.lock().unwrap();
    let index = users
        .iter()
        .position(|u| u.id == user_id)
        .ok_or_else(|| "User not found".to_string())?;
    let previous = users.clone();
    let original = users[index].clone();
    let result = match update(&mut users[index]) {
        Ok(value) => value,
        Err(error) => {
            users[index] = original;
            return Err(error);
        }
    };
    commit_users(&mut users, previous)?;
    Ok(result)
}

/// Auto-provision or update an external (OAuth/LDAP) user
pub fn ensure_oauth_user(store: &UserStore, username: &str, _role: &str) -> Result<(), String> {
    let mut users = store.lock().unwrap();
    let previous = users.clone();
    if let Some(u) = users.iter_mut().find(|u| u.username == username) {
        if !u.password_hash.is_empty() {
            return Err("External identity conflicts with a local user".into());
        }
        if !u.active() {
            return Err("최고관리자의 가입 승인이 필요합니다".into());
        }
    } else {
        users.push(NabimanUser {
            share_authorization_version: uuid::Uuid::new_v4().to_string(),
            id: generate_id(),
            username: username.to_string(),
            password_hash: String::new(),
            role: UserRole::Viewer,
            status: crate::models::AccountStatus::Pending,
            memberships: vec![],
            requested_memberships: vec![],
            scope_restricted: true,
            created_at: chrono::Utc::now().to_rfc3339(),
            last_login: None,
            totp_enabled: false,
            totp_secret: None,
            mfa: Default::default(),
        });
        commit_users(&mut users, previous)?;
        return Err("최고관리자의 가입 승인이 필요합니다".into());
    }
    commit_users(&mut users, previous)
}

pub fn update_last_login(store: &UserStore, username: &str) {
    let mut users = store.lock().unwrap();
    let previous = users.clone();
    if let Some(u) = users.iter_mut().find(|u| u.username == username) {
        u.last_login = Some(chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string());
    }
    if let Err(error) = commit_users(&mut users, previous) {
        eprintln!("Could not persist last login for {username}: {error}");
    }
}

async fn list_users(req: actix_web::HttpRequest, store: web::Data<UserStore>) -> HttpResponse {
    let actor = match crate::membership::actor(&req, &store) {
        Ok(u) => u,
        Err(r) => return r,
    };
    let users = store
        .lock()
        .unwrap()
        .iter()
        .filter(|u| crate::membership::visible(&actor, u))
        .cloned()
        .map(|mut u| {
            if !actor.super_admin() && actor.id != u.id {
                u.memberships
                    .retain(|m| actor.manages(&m.kind, &m.scope_id));
                u.requested_memberships.clear();
            }
            u
        })
        .collect::<Vec<_>>();
    HttpResponse::Ok().json(ApiResponse::ok(users))
}

async fn create_user(
    req: actix_web::HttpRequest,
    body: web::Json<CreateUserRequest>,
    store: web::Data<UserStore>,
) -> HttpResponse {
    if let Err(r) = crate::membership::require_super(&req, &store) {
        return r;
    }
    if body.username.len() < 2 || body.username.len() > 32 {
        return HttpResponse::Ok()
            .json(ApiResponse::<()>::error("Username must be 2-32 characters"));
    }
    if !body
        .username
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
    {
        return HttpResponse::Ok().json(ApiResponse::<()>::error(
            "Username: alphanumeric, _, - only",
        ));
    }
    if body.password.len() < 8 {
        return HttpResponse::Ok().json(ApiResponse::<()>::error(
            "Password must be at least 8 characters",
        ));
    }
    let mut users = store.lock().unwrap();
    if users.iter().any(|u| u.username == body.username) {
        return HttpResponse::Ok().json(ApiResponse::<()>::error("Username already exists"));
    }
    let hash = match bcrypt::hash(&body.password, 10) {
        Ok(h) => h,
        Err(e) => return HttpResponse::Ok().json(ApiResponse::<()>::error(&e.to_string())),
    };
    let user = NabimanUser {
        share_authorization_version: uuid::Uuid::new_v4().to_string(),
        id: generate_id(),
        username: body.username.clone(),
        password_hash: hash,
        role: body.role.clone(),
        created_at: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        last_login: None,
        totp_enabled: false,
        totp_secret: None,
        mfa: Default::default(),
        status: Default::default(),
        memberships: vec![],
        requested_memberships: vec![],
        scope_restricted: body.role != UserRole::Admin,
    };
    let previous = users.clone();
    users.push(user);
    match commit_users(&mut users, previous) {
        Ok(()) => HttpResponse::Ok().json(ApiResponse::ok("User created")),
        Err(error) => HttpResponse::InternalServerError().json(ApiResponse::<()>::error(&error)),
    }
}

async fn update_user(
    req: actix_web::HttpRequest,
    path: web::Path<String>,
    body: web::Json<UpdateUserRequest>,
    store: web::Data<UserStore>,
    sessions: web::Data<crate::jwt_sessions::SessionStore>,
) -> HttpResponse {
    if let Err(r) = crate::membership::require_super(&req, &store) {
        return r;
    }
    let user_id = path.into_inner();
    let mut users = store.lock().unwrap();
    let previous = users.clone();
    let admin_count = users.iter().filter(|u| u.super_admin()).count();
    if let Some(role) = &body.role {
        if users
            .iter()
            .any(|u| u.id == user_id && u.role == UserRole::Admin)
            && *role != UserRole::Admin
            && admin_count <= 1
        {
            return HttpResponse::BadRequest()
                .json(ApiResponse::<()>::error("Cannot demote the last admin"));
        }
    }
    let user = match users.iter_mut().find(|u| u.id == user_id) {
        Some(u) => u,
        None => return HttpResponse::Ok().json(ApiResponse::<()>::error("User not found")),
    };
    let target_username = user.username.clone();
    let password_changed = body.password.is_some();
    if let Some(ref pw) = body.password {
        if pw.len() < 8 {
            return HttpResponse::Ok().json(ApiResponse::<()>::error(
                "Password must be at least 8 characters",
            ));
        }
        user.password_hash = match bcrypt::hash(pw, 10) {
            Ok(hash) => hash,
            Err(error) => {
                return HttpResponse::InternalServerError()
                    .json(ApiResponse::<()>::error(&error.to_string()))
            }
        };
    }
    if let Some(ref role) = body.role {
        user.role = role.clone();
        if *role != UserRole::Admin {
            user.scope_restricted = true;
        }
    }
    match commit_users(&mut users, previous) {
        Ok(()) => {
            if password_changed || body.role.is_some() {
                crate::jwt_sessions::revoke_user_sessions(&sessions, &target_username, None);
            }
            HttpResponse::Ok().json(ApiResponse::ok("User updated"))
        }
        Err(error) => HttpResponse::InternalServerError().json(ApiResponse::<()>::error(&error)),
    }
}

async fn delete_user(
    req: actix_web::HttpRequest,
    path: web::Path<String>,
    store: web::Data<UserStore>,
    sessions: web::Data<crate::jwt_sessions::SessionStore>,
) -> HttpResponse {
    if let Err(r) = crate::membership::require_super(&req, &store) {
        return r;
    }
    let user_id = path.into_inner();
    let mut users = store.lock().unwrap();
    let admin_count = users.iter().filter(|u| u.super_admin()).count();
    let target_username = users
        .iter()
        .find(|u| u.id == user_id)
        .map(|u| u.username.clone());
    if let Some(u) = users.iter().find(|u| u.id == user_id) {
        if u.role == UserRole::Admin && admin_count <= 1 {
            return HttpResponse::Ok()
                .json(ApiResponse::<()>::error("Cannot delete the last admin"));
        }
    }
    let before = users.len();
    let previous = users.clone();
    users.retain(|u| u.id != user_id);
    if users.len() == before {
        return HttpResponse::Ok().json(ApiResponse::<()>::error("User not found"));
    }
    match commit_users(&mut users, previous) {
        Ok(()) => {
            if let Some(username) = target_username {
                crate::jwt_sessions::revoke_user_sessions(&sessions, &username, None);
            }
            HttpResponse::Ok().json(ApiResponse::ok("User deleted"))
        }
        Err(error) => HttpResponse::InternalServerError().json(ApiResponse::<()>::error(&error)),
    }
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.configure(crate::membership::config);
    cfg.service(
        web::scope("/api/users")
            .route("", web::get().to(list_users))
            .route("", web::post().to(create_user))
            .route("/me", web::get().to(profile::get_me))
            .route("/{id}", web::put().to(update_user))
            .route("/{id}", web::delete().to(delete_user)),
    );
}

#[cfg(test)]
#[path = "users_tests.rs"]
mod tests;

#[cfg(test)]
pub(crate) use storage::isolated_test_storage;
