//! bcrypt is slow on purpose (tens of milliseconds at cost 10, more under
//! load), so it never runs on an HTTP worker: a burst of sign-in attempts
//! would otherwise stall every request sharing those workers.

/// `bcrypt::verify` on the blocking pool. A runner failure counts as a
/// mismatch, never as a match.
pub async fn verify(password: &str, hash: &str) -> bcrypt::BcryptResult<bool> {
    let (password, hash) = (password.to_owned(), hash.to_owned());
    actix_web::web::block(move || bcrypt::verify(password, &hash)).await.unwrap_or(Ok(false))
}

/// `bcrypt::hash` on the blocking pool.
pub async fn hash(password: &str, cost: u32) -> Result<String, String> {
    let password = password.to_owned();
    match actix_web::web::block(move || bcrypt::hash(password, cost)).await {
        Ok(hashed) => hashed.map_err(|e| e.to_string()),
        Err(_) => Err("Password hashing stopped unexpectedly".into()),
    }
}

/// The user's current password hash, when `password` matches it. Checked
/// outside the user store's lock; a caller then confirms, inside the lock,
/// that the hash is still the same before acting on it.
pub async fn verified_hash(store: &crate::users::UserStore, user_id: &str, password: &str) -> Option<String> {
    use crate::lock_safe::LockExt;
    let hash = store.lock_safe().iter().find(|u| u.id == user_id)?.password_hash.clone();
    verify(password, &hash).await.unwrap_or(false).then_some(hash)
}

#[cfg(test)]
mod tests {
    #[actix_rt::test]
    async fn matches_only_the_right_password() {
        let hashed = super::hash("correct horse", 4).await.unwrap();
        assert!(super::verify("correct horse", &hashed).await.unwrap());
        assert!(!super::verify("wrong", &hashed).await.unwrap());
        assert!(super::verify("x", "not a hash").await.is_err());
    }
}
