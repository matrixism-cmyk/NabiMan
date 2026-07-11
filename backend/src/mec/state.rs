use super::audit_logger::AuditLogger;
use super::db::{
    audit_store::AuditStore, docs_store::DocsStore, job_store::JobStore, tenant_store::TenantStore,
    DbHandle,
};
use super::job_runner::JobRunner;
use super::services::ServiceBundle;
use crate::models::mec::LiveSnapshot;
use crate::notifications::ChannelStore;
use std::sync::Arc;
use tokio::sync::watch;

pub struct MecState {
    #[allow(dead_code)]
    pub db: DbHandle,
    pub services: Arc<ServiceBundle>,
    pub audit: Arc<AuditLogger>,
    pub jobs: Arc<JobRunner>,
    pub tenants: Arc<TenantStore>,
    pub job_store: Arc<JobStore>,
    pub audit_store: Arc<AuditStore>,
    pub docs_store: Arc<DocsStore>,
    pub channels: Option<ChannelStore>,
    /// Latest dashboard snapshot, refreshed by one shared background poller and
    /// fanned out to every SSE subscriber (see handlers::dashboard_sse). `None`
    /// until the first successful poll.
    pub live_tx: watch::Sender<Option<LiveSnapshot>>,
}

impl MecState {
    pub fn new(db: DbHandle, services: ServiceBundle) -> Self {
        let services = Arc::new(services);
        let audit_store = Arc::new(AuditStore::new(db.clone()));
        let job_store = Arc::new(JobStore::new(db.clone()));
        let tenants = Arc::new(TenantStore::new(db.clone()));
        let docs_store = Arc::new(DocsStore::new(db.clone()));
        let audit = Arc::new(AuditLogger::new(audit_store.clone()));
        let jobs = Arc::new(JobRunner::new(job_store.clone()));
        let (live_tx, _) = watch::channel(None);
        Self {
            db,
            services,
            audit,
            jobs,
            tenants,
            job_store,
            audit_store,
            docs_store,
            channels: None,
            live_tx,
        }
    }

    pub fn with_channels(mut self, channels: ChannelStore) -> Self {
        self.channels = Some(channels);
        self
    }

    #[allow(dead_code)]
    pub fn mocks() -> Self {
        let db = super::db::open_in_memory().expect("in-memory db");
        Self::new(db, ServiceBundle::mocks())
    }
}
