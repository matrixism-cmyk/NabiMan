use super::db::audit_store::AuditStore;
use crate::models::mec::{AuditStatus, MecAuditLog, OperationLog};
use chrono::Utc;
use std::sync::{Arc, Mutex};
use std::time::Instant;

pub struct AuditLogger {
    store: Arc<AuditStore>,
}

impl AuditLogger {
    pub fn new(store: Arc<AuditStore>) -> Self {
        Self { store }
    }

    pub fn begin(&self, user: &str, action: &str, resource_type: &str, resource_id: &str) -> AuditBuilder {
        AuditBuilder::new(self.store.clone(), user, action, resource_type, resource_id)
    }
}

pub struct AuditBuilder {
    store: Arc<AuditStore>,
    id: String,
    user: String,
    action: String,
    resource_type: String,
    resource_id: String,
    started: Instant,
    input: Option<serde_json::Value>,
    source_ip: Option<String>,
    user_agent: Option<String>,
    operations: Mutex<Vec<OperationLog>>,
}

impl AuditBuilder {
    fn new(
        store: Arc<AuditStore>,
        user: &str,
        action: &str,
        resource_type: &str,
        resource_id: &str,
    ) -> Self {
        Self {
            store,
            id: uuid::Uuid::new_v4().to_string(),
            user: user.into(),
            action: action.into(),
            resource_type: resource_type.into(),
            resource_id: resource_id.into(),
            started: Instant::now(),
            input: None,
            source_ip: None,
            user_agent: None,
            operations: Mutex::new(Vec::new()),
        }
    }

    pub fn with_input(mut self, input: serde_json::Value) -> Self {
        self.input = Some(input);
        self
    }

    pub fn with_source_ip(mut self, ip: Option<String>) -> Self {
        self.source_ip = ip;
        self
    }

    pub fn with_user_agent(mut self, ua: Option<String>) -> Self {
        self.user_agent = ua;
        self
    }

    #[allow(dead_code)]
    pub fn log_op(&self, name: &str, target: &str, status: &str, duration_ms: u64) {
        self.operations.lock().unwrap().push(OperationLog {
            name: name.into(),
            target: target.into(),
            status: status.into(),
            duration_ms,
            details: None,
        });
    }

    #[allow(dead_code)]
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn complete(self, status: AuditStatus, output: Option<serde_json::Value>, error: Option<String>) {
        let log = MecAuditLog {
            id: self.id,
            timestamp: Utc::now(),
            user: self.user,
            action: self.action,
            resource_type: self.resource_type,
            resource_id: self.resource_id,
            status,
            duration_ms: self.started.elapsed().as_millis() as u64,
            input: self.input,
            output,
            error,
            operations: self.operations.into_inner().unwrap_or_default(),
            source_ip: self.source_ip,
            user_agent: self.user_agent,
        };
        let _ = self.store.insert(&log);
    }

    pub fn success(self, output: Option<serde_json::Value>) {
        self.complete(AuditStatus::Success, output, None);
    }

    pub fn failure(self, error: String) {
        self.complete(AuditStatus::Failed, None, Some(error));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mec::db;

    #[test]
    fn logger_records_success() {
        let db = db::open_in_memory().unwrap();
        let store = Arc::new(AuditStore::new(db));
        let logger = AuditLogger::new(store.clone());
        let b = logger
            .begin("admin", "tenant_create", "tenant", "x")
            .with_input(serde_json::json!({"a": 1}));
        b.log_op("create_namespace", "k8s", "success", 100);
        b.success(Some(serde_json::json!({"ok": true})));
        let all = store.query(&Default::default()).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].status, AuditStatus::Success);
        assert_eq!(all[0].operations.len(), 1);
    }
}
