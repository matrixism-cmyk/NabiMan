use actix_web::{get, post, delete, web, HttpResponse};
use std::fs;
use std::process::Command;
use crate::models::{
    ApiResponse, ChangePasswordRequest, CreateAccountRequest, DeleteAccountRequest, UserAccount,
};

#[get("/api/accounts")]
async fn list_accounts() -> HttpResponse {
    let accounts = read_passwd_file();
    HttpResponse::Ok().json(ApiResponse::ok(accounts))
}

#[post("/api/accounts")]
async fn create_account(body: web::Json<CreateAccountRequest>) -> HttpResponse {
    let shell = body.shell.as_deref().unwrap_or("/bin/bash");

    // Validate username: only alphanumeric, underscore, hyphen
    if !body
        .username
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
    {
        return HttpResponse::BadRequest()
            .json(ApiResponse::<()>::error("Invalid username format"));
    }

    let output = Command::new("useradd")
        .args(["-m", "-s", shell, &body.username])
        .output();

    match output {
        Ok(result) if result.status.success() => {
            // Set password
            let passwd_result = Command::new("chpasswd")
                .stdin(std::process::Stdio::piped())
                .spawn()
                .and_then(|mut child| {
                    use std::io::Write;
                    if let Some(ref mut stdin) = child.stdin {
                        stdin.write_all(
                            format!("{}:{}", body.username, body.password).as_bytes(),
                        )?;
                    }
                    child.wait()
                });

            match passwd_result {
                Ok(status) if status.success() => {
                    HttpResponse::Ok().json(ApiResponse::ok("Account created"))
                }
                _ => HttpResponse::InternalServerError()
                    .json(ApiResponse::<()>::error("Account created but password set failed")),
            }
        }
        Ok(result) => {
            let stderr = String::from_utf8_lossy(&result.stderr);
            HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::error(&format!("useradd failed: {}", stderr)))
        }
        Err(e) => HttpResponse::InternalServerError()
            .json(ApiResponse::<()>::error(&format!("Failed to run useradd: {}", e))),
    }
}

#[post("/api/accounts/password")]
async fn change_password(body: web::Json<ChangePasswordRequest>) -> HttpResponse {
    let result = Command::new("chpasswd")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(ref mut stdin) = child.stdin {
                stdin.write_all(
                    format!("{}:{}", body.username, body.new_password).as_bytes(),
                )?;
            }
            child.wait()
        });

    match result {
        Ok(status) if status.success() => {
            HttpResponse::Ok().json(ApiResponse::ok("Password changed"))
        }
        _ => HttpResponse::InternalServerError()
            .json(ApiResponse::<()>::error("Password change failed")),
    }
}

#[delete("/api/accounts")]
async fn delete_account(body: web::Json<DeleteAccountRequest>) -> HttpResponse {
    if body.username == "root" {
        return HttpResponse::Forbidden()
            .json(ApiResponse::<()>::error("Cannot delete root"));
    }

    let output = Command::new("userdel")
        .args(["-r", &body.username])
        .output();

    match output {
        Ok(result) if result.status.success() => {
            HttpResponse::Ok().json(ApiResponse::ok("Account deleted"))
        }
        Ok(result) => {
            let stderr = String::from_utf8_lossy(&result.stderr);
            HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::error(&format!("userdel failed: {}", stderr)))
        }
        Err(e) => HttpResponse::InternalServerError()
            .json(ApiResponse::<()>::error(&format!("Failed to run userdel: {}", e))),
    }
}

fn read_passwd_file() -> Vec<UserAccount> {
    let logged_in_users = get_logged_in_users();

    let content = match fs::read_to_string("/etc/passwd") {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    content
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() < 7 {
                return None;
            }
            let uid: u32 = parts[2].parse().ok()?;
            // Show regular users and root
            if uid > 999 || uid == 0 {
                Some(UserAccount {
                    username: parts[0].to_string(),
                    uid,
                    gid: parts[3].parse().unwrap_or(0),
                    home: parts[5].to_string(),
                    shell: parts[6].to_string(),
                    is_logged_in: logged_in_users.contains(&parts[0].to_string()),
                })
            } else {
                None
            }
        })
        .collect()
}

fn get_logged_in_users() -> Vec<String> {
    if let Ok(output) = Command::new("who").output() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        return stdout
            .lines()
            .filter_map(|l| l.split_whitespace().next())
            .map(String::from)
            .collect();
    }
    Vec::new()
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(list_accounts)
        .service(create_account)
        .service(change_password)
        .service(delete_account);
}
