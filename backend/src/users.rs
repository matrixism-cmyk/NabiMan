use actix_web::{web, HttpResponse};
use crate::models::{ApiResponse, NabimanUser, UserRole, CreateUserRequest, UpdateUserRequest};
use std::sync::{Arc, Mutex};

pub type UserStore = Arc<Mutex<Vec<NabimanUser>>>;

pub fn new_user_store(pw_hash: &str) -> UserStore {
    let path = users_file_path();
    if let Ok(data) = std::fs::read_to_string(&path) {
        if let Ok(users) = serde_json::from_str::<Vec<NabimanUser>>(&data) {
            if !users.is_empty() {
                return Arc::new(Mutex::new(users));
            }
        }
    }
    // Migrate: create default admin from existing password hash
    let admin = NabimanUser {
        id: generate_id(),
        username: "admin".into(),
        password_hash: pw_hash.to_string(),
        role: UserRole::Admin,
        created_at: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        last_login: None,
        totp_enabled: false,
        totp_secret: None,
    };
    let users = vec![admin];
    let _ = save_users(&users);
    println!("Default admin user created from existing password");
    Arc::new(Mutex::new(users))
}

fn users_file_path() -> std::path::PathBuf {
    let dir = std::env::var("NABIMAN_DATA_DIR").unwrap_or_else(|_| "/var/lib/nabiman".into());
    std::path::PathBuf::from(dir).join("users.json")
}

fn save_users(users: &[NabimanUser]) -> Result<(), String> {
    use std::os::unix::fs::OpenOptionsExt;
    let path = users_file_path();
    if let Some(p) = path.parent() { let _ = std::fs::create_dir_all(p); }
    let json = serde_json::to_string_pretty(users).map_err(|e| e.to_string())?;
    std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600)
        .open(&path)
        .and_then(|mut f| { use std::io::Write; f.write_all(json.as_bytes()) })
        .map_err(|e| e.to_string())
}

fn generate_id() -> String {
    use rand::Rng;
    let id: u64 = rand::thread_rng().gen();
    format!("{:016x}", id)
}

pub fn find_user_by_name(store: &UserStore, username: &str) -> Option<NabimanUser> {
    store.lock().unwrap().iter().find(|u| u.username == username).cloned()
}

pub fn update_last_login(store: &UserStore, username: &str) {
    let mut users = store.lock().unwrap();
    if let Some(u) = users.iter_mut().find(|u| u.username == username) {
        u.last_login = Some(chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string());
    }
    let _ = save_users(&users);
}

async fn list_users(store: web::Data<UserStore>) -> HttpResponse {
    let users = store.lock().unwrap().clone();
    HttpResponse::Ok().json(ApiResponse::ok(users))
}

async fn create_user(body: web::Json<CreateUserRequest>, store: web::Data<UserStore>) -> HttpResponse {
    if body.username.len() < 2 || body.username.len() > 32 {
        return HttpResponse::Ok().json(ApiResponse::<()>::error("Username must be 2-32 characters"));
    }
    if !body.username.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
        return HttpResponse::Ok().json(ApiResponse::<()>::error("Username: alphanumeric, _, - only"));
    }
    if body.password.len() < 8 {
        return HttpResponse::Ok().json(ApiResponse::<()>::error("Password must be at least 8 characters"));
    }
    let mut users = store.lock().unwrap();
    if users.iter().any(|u| u.username == body.username) {
        return HttpResponse::Ok().json(ApiResponse::<()>::error("Username already exists"));
    }
    let hash = match bcrypt::hash(&body.password, 10) {
        Ok(h) => h, Err(e) => return HttpResponse::Ok().json(ApiResponse::<()>::error(&e.to_string())),
    };
    let user = NabimanUser {
        id: generate_id(), username: body.username.clone(), password_hash: hash,
        role: body.role.clone(),
        created_at: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        last_login: None, totp_enabled: false, totp_secret: None,
    };
    users.push(user);
    let _ = save_users(&users);
    HttpResponse::Ok().json(ApiResponse::ok("User created"))
}

async fn update_user(
    path: web::Path<String>, body: web::Json<UpdateUserRequest>, store: web::Data<UserStore>,
) -> HttpResponse {
    let user_id = path.into_inner();
    let mut users = store.lock().unwrap();
    let user = match users.iter_mut().find(|u| u.id == user_id) {
        Some(u) => u,
        None => return HttpResponse::Ok().json(ApiResponse::<()>::error("User not found")),
    };
    if let Some(ref pw) = body.password {
        if pw.len() < 8 {
            return HttpResponse::Ok().json(ApiResponse::<()>::error("Password must be at least 8 characters"));
        }
        user.password_hash = bcrypt::hash(pw, 10).unwrap_or_default();
    }
    if let Some(ref role) = body.role { user.role = role.clone(); }
    let _ = save_users(&users);
    HttpResponse::Ok().json(ApiResponse::ok("User updated"))
}

async fn delete_user(path: web::Path<String>, store: web::Data<UserStore>) -> HttpResponse {
    let user_id = path.into_inner();
    let mut users = store.lock().unwrap();
    let admin_count = users.iter().filter(|u| u.role == UserRole::Admin).count();
    if let Some(u) = users.iter().find(|u| u.id == user_id) {
        if u.role == UserRole::Admin && admin_count <= 1 {
            return HttpResponse::Ok().json(ApiResponse::<()>::error("Cannot delete the last admin"));
        }
    }
    let before = users.len();
    users.retain(|u| u.id != user_id);
    if users.len() == before {
        return HttpResponse::Ok().json(ApiResponse::<()>::error("User not found"));
    }
    let _ = save_users(&users);
    HttpResponse::Ok().json(ApiResponse::ok("User deleted"))
}

async fn get_me(req: actix_web::HttpRequest, store: web::Data<UserStore>) -> HttpResponse {
    let username = extract_username_from_req(&req);
    let users = store.lock().unwrap();
    match users.iter().find(|u| u.username == username) {
        Some(u) => HttpResponse::Ok().json(ApiResponse::ok(u.clone())),
        None => HttpResponse::Ok().json(ApiResponse::<()>::error("User not found")),
    }
}

fn extract_username_from_req(req: &actix_web::HttpRequest) -> String {
    if let Some(auth) = req.headers().get("Authorization") {
        if let Ok(val) = auth.to_str() {
            if let Some(token) = val.strip_prefix("Bearer ") {
                if let Ok(data) = crate::auth::decode_claims_unverified(token) {
                    return data.claims.sub;
                }
            }
        }
    }
    "admin".into()
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/users")
            .route("", web::get().to(list_users))
            .route("", web::post().to(create_user))
            .route("/me", web::get().to(get_me))
            .route("/{id}", web::put().to(update_user))
            .route("/{id}", web::delete().to(delete_user))
    );
}
