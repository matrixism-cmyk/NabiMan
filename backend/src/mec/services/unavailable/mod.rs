//! Disconnected connectors reject reads and writes instead of returning demo data.
mod axgate;
mod harbor;
mod kube;
mod rancher;

#[derive(Clone)]
pub struct UnavailableService {
    source: &'static str,
    reason: &'static str,
}
impl UnavailableService {
    pub fn new(source: &'static str, reason: &'static str) -> Self {
        Self { source, reason }
    }
    fn reject<T>(&self) -> super::ServiceResult<T> {
        Err(super::ServiceError::Unavailable(format!(
            "{}: {}",
            self.source, self.reason
        )))
    }
}
