use super::client::{handle_response, upstream_err, RancherReal};
use crate::mec::services::rancher_service::RancherUser;
use crate::mec::services::ServiceResult;
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Deserialize)]
struct UserList {
    #[serde(default)]
    data: Vec<UserWire>,
}

#[derive(Debug, Deserialize)]
struct UserWire {
    id: String,
    #[serde(default)]
    username: Option<String>,
    #[serde(default, rename = "displayName")]
    display_name: Option<String>,
    #[serde(default)]
    enabled: Option<bool>,
    #[serde(default, rename = "mustChangePassword")]
    must_change_password: Option<bool>,
}

impl From<UserWire> for RancherUser {
    fn from(w: UserWire) -> Self {
        Self {
            id: w.id,
            username: w.username.unwrap_or_default(),
            display_name: w.display_name,
            enabled: w.enabled.unwrap_or(true),
            must_change_password: w.must_change_password.unwrap_or(false),
        }
    }
}

pub async fn create(r: &RancherReal, username: &str, password: &str) -> ServiceResult<RancherUser> {
    let url = r.url("/v3/users");
    let body = json!({
        "type": "user",
        "username": username,
        "password": password,
        "enabled": true,
        "mustChangePassword": false,
    });
    let res = r
        .auth(r.http.post(&url))
        .json(&body)
        .send()
        .await
        .map_err(upstream_err)?;
    let wire: UserWire = handle_response(res, &format!("create user {}", username)).await?;
    Ok(wire.into())
}

pub async fn delete(r: &RancherReal, id: &str) -> ServiceResult<()> {
    let url = r.url(&format!("/v3/users/{}", id));
    let res = r
        .auth(r.http.delete(&url))
        .send()
        .await
        .map_err(upstream_err)?;
    if res.status().is_success() || res.status() == reqwest::StatusCode::NOT_FOUND {
        Ok(())
    } else {
        let code = res.status();
        let body = res.text().await.unwrap_or_default();
        Err(upstream_err(format!("delete user {}: {} {}", id, code, body)))
    }
}

pub async fn get(r: &RancherReal, id: &str) -> ServiceResult<RancherUser> {
    let url = r.url(&format!("/v3/users/{}", id));
    let res = r
        .auth(r.http.get(&url))
        .send()
        .await
        .map_err(upstream_err)?;
    let wire: UserWire = handle_response(res, &format!("user {}", id)).await?;
    Ok(wire.into())
}

pub async fn list(r: &RancherReal) -> ServiceResult<Vec<RancherUser>> {
    let url = r.url("/v3/users");
    let res = r
        .auth(r.http.get(&url))
        .send()
        .await
        .map_err(upstream_err)?;
    let list: UserList = handle_response(res, "list users").await?;
    Ok(list.data.into_iter().map(Into::into).collect())
}

pub async fn reset_password(
    r: &RancherReal,
    id: &str,
    new_password: &str,
) -> ServiceResult<()> {
    let url = r.url(&format!("/v3/users/{}?action=setpassword", id));
    let body = json!({ "newPassword": new_password });
    let res = r
        .auth(r.http.post(&url))
        .json(&body)
        .send()
        .await
        .map_err(upstream_err)?;
    if res.status().is_success() {
        Ok(())
    } else if res.status() == reqwest::StatusCode::NOT_FOUND {
        Err(crate::mec::services::ServiceError::NotFound(format!(
            "user {}",
            id
        )))
    } else {
        let code = res.status();
        let body = res.text().await.unwrap_or_default();
        Err(upstream_err(format!(
            "reset password {}: {} {}",
            id, code, body
        )))
    }
}
