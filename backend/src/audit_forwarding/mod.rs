//! Durable HTTPS forwarding, separate from the local request journal.
mod api;
mod store;
#[cfg(test)]
mod tests;
use crate::audit::AuditEntry;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use store::{Delivery, Store};
#[derive(Clone, Copy, Debug)]
pub enum Error {
    Storage,
    Invalid,
    Conflict,
    Limit,
    Credential,
    TlsConfig,
}
impl From<rusqlite::Error> for Error {
    fn from(_: rusqlite::Error) -> Self {
        Self::Storage
    }
}
impl Error {
    fn message(self) -> &'static str {
        match self {
            Self::Storage => "Forwarding storage unavailable; inspect storage before restart",
            Self::Invalid => "Invalid HTTPS destination or request",
            Self::Conflict => "Destination changed; reload before retrying",
            Self::Limit => "Forwarding destination or queue limit reached",
            Self::Credential => "Forwarding credential unavailable",
            Self::TlsConfig => "Forwarding TLS configuration invalid",
        }
    }
}
struct Inner {
    store: Mutex<Option<Store>>,
    failure: Mutex<Option<Error>>,
    client: Option<reqwest::Client>,
    legacy: bool,
    paused: bool,
}
#[derive(Clone)]
pub struct Forwarder(Arc<Inner>);
pub fn validate_url(value: &str) -> Result<String, Error> {
    if value.len() > 2048 || value.chars().any(char::is_control) {
        return Err(Error::Invalid);
    }
    let url = reqwest::Url::parse(value).map_err(|_| Error::Invalid)?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(Error::Invalid);
    }
    Ok(url.into())
}
fn client() -> Result<reqwest::Client, Error> {
    let mut builder = reqwest::Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(5));
    if let Some(path) = std::env::var_os("NABIMAN_AUDIT_CA_FILE") {
        if std::fs::metadata(&path)
            .map_err(|_| Error::TlsConfig)?
            .len()
            > 1024 * 1024
        {
            return Err(Error::TlsConfig);
        }
        let bytes = std::fs::read(path).map_err(|_| Error::TlsConfig)?;
        if bytes.len() > 1024 * 1024 {
            return Err(Error::TlsConfig);
        }
        builder = builder.add_root_certificate(
            reqwest::Certificate::from_pem(&bytes).map_err(|_| Error::TlsConfig)?,
        );
    }
    builder.build().map_err(|_| Error::TlsConfig)
}
impl Forwarder {
    pub fn start() -> Self {
        let dir = nabiman_server::data_dir();
        let opened = Store::open(&dir.join("audit-forwarding.sqlite3"));
        let http = client();
        let failure = opened
            .as_ref()
            .err()
            .copied()
            .or_else(|| http.as_ref().err().copied());
        let service = Self(Arc::new(Inner {
            store: Mutex::new(opened.ok()),
            failure: Mutex::new(failure),
            client: http.ok(),
            legacy: dir.join("audit_forwarding.json").exists(),
            paused: std::env::var("NABIMAN_AUDIT_FORWARDING_DISABLED").as_deref() == Ok("1"),
        }));
        let worker = service.clone();
        actix_web::rt::spawn(async move {
            worker.run().await;
        });
        service
    }
    fn access<T>(
        &self,
        operation: impl FnOnce(&mut Store) -> Result<T, Error>,
    ) -> Result<T, Error> {
        if let Some(error) = *self.0.failure.lock().unwrap() {
            return Err(error);
        }
        let mut guard = self.0.store.lock().unwrap();
        let result = operation(guard.as_mut().ok_or(Error::Storage)?);
        if matches!(result, Err(Error::Storage)) {
            *self.0.failure.lock().unwrap() = Some(Error::Storage);
            eprintln!("Audit forwarding storage unavailable; local journal and completed operation outcomes are unchanged");
        }
        result
    }
    pub fn enqueue(&self, entry: &AuditEntry) -> bool {
        // Do not forward high-frequency reads of forwarding's own queue status.
        if entry.method == "GET" && entry.path.starts_with("/api/audit/forwarding") {
            return true;
        }
        self.access(|s| s.enqueue(entry, None)).is_ok()
    }
    async fn send(&self, row: &Delivery) -> Result<(), String> {
        let url = validate_url(&row.url).map_err(|_| "destination_invalid".to_string())?;
        let mut request = self
            .0
            .client
            .as_ref()
            .ok_or("tls_configuration_invalid")?
            .post(url)
            .header("Idempotency-Key", &row.id)
            .header("Content-Type", "application/json")
            .body(row.payload.clone());
        if !row.auth.is_empty() {
            let plain = crate::secret_store::decrypt(&row.auth)
                .map_err(|_| "credential_unavailable".to_string())?;
            let mut header = reqwest::header::HeaderValue::from_str(&plain)
                .map_err(|_| "credential_invalid".to_string())?;
            header.set_sensitive(true);
            request = request.header(reqwest::header::AUTHORIZATION, header);
        }
        let response = request
            .send()
            .await
            .map_err(|_| "tls_or_transport_failed".to_string())?;
        if !response.status().is_success() {
            return Err(format!("http_rejected_{}", response.status().as_u16()));
        }
        Ok(())
    }
    async fn run(self) {
        loop {
            if self.0.paused {
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            }
            let claimed = self.access(|s| s.claim(chrono::Utc::now().timestamp()));
            match claimed {
                Ok(Some(row)) => {
                    let result = self.send(&row).await;
                    let _ = self.access(|s| {
                        s.finish(
                            &row,
                            result.as_ref().err().map(String::as_str),
                            chrono::Utc::now().timestamp(),
                        )
                    });
                }
                _ => tokio::time::sleep(Duration::from_millis(500)).await,
            }
        }
    }
}
pub fn config(cfg: &mut actix_web::web::ServiceConfig) {
    api::config(cfg)
}
