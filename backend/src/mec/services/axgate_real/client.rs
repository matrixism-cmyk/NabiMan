use super::session::AxgateSession;
use crate::mec::services::axgate_service::{AxgateHealth, AxgateService, SyncResult};
use crate::mec::services::{ServiceError, ServiceResult};
use crate::models::mec::{
    CreateNatRuleRequest, NatRule, PublicIp, SecurityPolicy,
};
use async_trait::async_trait;
use chrono::Utc;
use std::sync::Mutex;
use std::time::Duration;

pub struct AxgateRealConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub timeout_secs: u64,
}

impl AxgateRealConfig {
    pub fn from_env() -> Option<Self> {
        let host = std::env::var("NABIMAN_MEC_AXGATE_HOST").ok()?;
        let username = std::env::var("NABIMAN_MEC_AXGATE_USER").ok()?;
        let password = std::env::var("NABIMAN_MEC_AXGATE_PASSWORD").ok()?;
        let port = std::env::var("NABIMAN_MEC_AXGATE_PORT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(2222);
        let timeout_secs = std::env::var("NABIMAN_MEC_AXGATE_TIMEOUT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(30);
        Some(Self {
            host,
            port,
            username,
            password,
            timeout_secs,
        })
    }
}

pub struct AxgateReal {
    config: AxgateRealConfig,
    cache: Mutex<AxgateCache>,
}

#[derive(Default)]
struct AxgateCache {
    running_config: Option<String>,
    cached_at: Option<std::time::Instant>,
}

const CACHE_TTL: Duration = Duration::from_secs(300);

impl AxgateReal {
    pub fn new(config: AxgateRealConfig) -> Self {
        Self {
            config,
            cache: Mutex::new(AxgateCache::default()),
        }
    }

    pub(super) async fn fetch_config(&self) -> ServiceResult<String> {
        let now = std::time::Instant::now();
        if let Some(config) = self.cached_config(now) {
            return Ok(config);
        }
        let cfg = AxgateSession::connect(
            &self.config.host,
            self.config.port,
            &self.config.username,
            &self.config.password,
            Duration::from_secs(self.config.timeout_secs),
        )
        .await?
        .fetch_running_config()
        .await?;
        self.cache.lock().unwrap().running_config = Some(cfg.clone());
        self.cache.lock().unwrap().cached_at = Some(now);
        Ok(cfg)
    }

    fn cached_config(&self, now: std::time::Instant) -> Option<String> {
        let cache = self.cache.lock().unwrap();
        cache
            .cached_at
            .filter(|t| now.duration_since(*t) < CACHE_TTL)
            .and(cache.running_config.clone())
    }

    fn invalidate_cache(&self) {
        let mut cache = self.cache.lock().unwrap();
        cache.running_config = None;
        cache.cached_at = None;
    }

    pub(super) fn config(&self) -> &AxgateRealConfig {
        &self.config
    }
}

#[async_trait]
impl AxgateService for AxgateReal {
    async fn list_public_ips(&self) -> ServiceResult<Vec<PublicIp>> {
        let cfg = self.fetch_config().await?;
        let cleaned = super::preprocess::clean(&cfg);
        Ok(super::parser::parse_public_ips(&cleaned))
    }

    async fn list_nat_rules(&self) -> ServiceResult<Vec<NatRule>> {
        let cfg = self.fetch_config().await?;
        let cleaned = super::preprocess::clean(&cfg);
        let rules = super::parser::parse_nat_rules(&cleaned);
        if std::env::var("NABIMAN_MEC_AXGATE_DEBUG").is_ok() {
            eprintln!(
                "[axgate:debug] raw_len={} cleaned_len={} rules={}",
                cfg.len(),
                cleaned.len(),
                rules.len(),
            );
        }
        if std::env::var("NABIMAN_MEC_AXGATE_DUMP_CONFIG").is_ok() {
            let path = "/tmp/axgate-running-config.txt";
            let _ = std::fs::write(path, &cleaned);
            eprintln!("[axgate:debug] cleaned config written to {}", path);
        }
        Ok(rules)
    }

    async fn add_nat_rule(&self, req: &CreateNatRuleRequest) -> ServiceResult<NatRule> {
        let rule = super::ops::add_nat_rule(self, req).await?;
        self.invalidate_cache();
        Ok(rule)
    }

    async fn delete_nat_rule(&self, id: &str) -> ServiceResult<()> {
        super::ops::delete_nat_rule(self, id).await?;
        self.invalidate_cache();
        Ok(())
    }

    async fn toggle_nat_rule(&self, id: &str, enabled: bool) -> ServiceResult<()> {
        super::ops::toggle_nat_rule(self, id, enabled).await?;
        self.invalidate_cache();
        Ok(())
    }

    async fn list_security_policies(&self) -> ServiceResult<Vec<SecurityPolicy>> {
        let cfg = self.fetch_config().await?;
        let cleaned = super::preprocess::clean(&cfg);
        Ok(super::parser::parse_security_policies(&cleaned))
    }

    async fn sync_running_config(&self) -> ServiceResult<SyncResult> {
        self.invalidate_cache();
        let cfg = self.fetch_config().await?;
        let cleaned = super::preprocess::clean(&cfg);
        Ok(SyncResult {
            nat_rules_count: super::parser::parse_nat_rules(&cleaned).len(),
            security_policies_count: super::parser::parse_security_policies(&cleaned).len(),
            public_ips_count: super::parser::parse_public_ips(&cleaned).len(),
            synced_at: Utc::now(),
        })
    }

    async fn health_check(&self) -> ServiceResult<AxgateHealth> {
        let start = std::time::Instant::now();
        match AxgateSession::connect(
            &self.config.host,
            self.config.port,
            &self.config.username,
            &self.config.password,
            Duration::from_secs(self.config.timeout_secs),
        )
        .await
        {
            Ok(_) => Ok(AxgateHealth {
                connected: true,
                endpoint: format!("{}:{}", self.config.host, self.config.port),
                latency_ms: Some(start.elapsed().as_millis() as u64),
                error: None,
            }),
            Err(e) => Ok(AxgateHealth {
                connected: false,
                endpoint: format!("{}:{}", self.config.host, self.config.port),
                latency_ms: None,
                error: Some(e.to_string()),
            }),
        }
    }
}

pub(super) fn upstream<E: std::fmt::Display>(e: E) -> ServiceError {
    ServiceError::Upstream {
        source: "axgate".into(),
        message: e.to_string(),
    }
}
