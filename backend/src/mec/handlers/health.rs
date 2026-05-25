use super::util::ok_response;
use crate::mec::MecState;
use actix_web::{web, HttpResponse};
use serde::Serialize;

#[derive(Serialize)]
pub struct HealthSummary {
    pub kubernetes: HealthEntry,
    pub rancher: HealthEntry,
    pub axgate: HealthEntry,
    pub harbor: HealthEntry,
    pub mode: String,
}

#[derive(Serialize)]
pub struct HealthEntry {
    pub connected: bool,
    pub endpoint: String,
    pub latency_ms: Option<u64>,
    pub error: Option<String>,
}

pub async fn summary(state: web::Data<MecState>) -> HttpResponse {
    let (kube, rancher, axgate, harbor) = tokio::join!(
        state.services.kube.health_check(),
        state.services.rancher.health_check(),
        state.services.axgate.health_check(),
        state.services.harbor.health_check(),
    );

    let summary = HealthSummary {
        kubernetes: kube
            .map(|h| HealthEntry {
                connected: h.connected,
                endpoint: h.endpoint,
                latency_ms: h.latency_ms,
                error: h.error,
            })
            .unwrap_or_else(|e| error_entry(&e.to_string())),
        rancher: rancher
            .map(|h| HealthEntry {
                connected: h.connected,
                endpoint: h.endpoint,
                latency_ms: h.latency_ms,
                error: h.error,
            })
            .unwrap_or_else(|e| error_entry(&e.to_string())),
        axgate: axgate
            .map(|h| HealthEntry {
                connected: h.connected,
                endpoint: h.endpoint,
                latency_ms: h.latency_ms,
                error: h.error,
            })
            .unwrap_or_else(|e| error_entry(&e.to_string())),
        harbor: harbor
            .map(|h| HealthEntry {
                connected: h.connected,
                endpoint: h.endpoint,
                latency_ms: h.latency_ms,
                error: h.error,
            })
            .unwrap_or_else(|e| error_entry(&e.to_string())),
        mode: std::env::var("NABIMAN_MEC_MODE").unwrap_or_else(|_| "auto".into()),
    };

    ok_response(summary)
}

fn error_entry(msg: &str) -> HealthEntry {
    HealthEntry {
        connected: false,
        endpoint: "-".into(),
        latency_ms: None,
        error: Some(msg.to_string()),
    }
}
