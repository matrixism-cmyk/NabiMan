use super::axgate_mock::AxgateMock;
use super::axgate_real::{AxgateReal, AxgateRealConfig};
use super::harbor_mock::HarborMock;
use super::harbor_real::{HarborReal, HarborRealConfig};
use super::kube_mock::KubeMock;
use super::kube_real::{KubeReal, KubeRealConfig};
use super::rancher_mock::RancherMock;
use super::rancher_real::{RancherReal, RancherRealConfig};
use super::{
    AxgateServiceArc, HarborServiceArc, KubeService, KubeServiceArc, RancherServiceArc,
    ServiceBundle,
};
use std::sync::Arc;

#[derive(Debug, Clone, Copy)]
pub enum ServiceMode {
    Mock,
    Real,
    Auto,
}

impl ServiceMode {
    pub fn from_env() -> Self {
        match std::env::var("NABIMAN_MEC_MODE").ok().as_deref() {
            Some("real") => Self::Real,
            Some("mock") => Self::Mock,
            _ => Self::Auto,
        }
    }
}

/// Build a service bundle, preferring real clients when their env credentials
/// are present, and falling back to mocks otherwise.
pub async fn bootstrap(mode: ServiceMode) -> ServiceBundle {
    let kube = bootstrap_kube(mode).await;
    let rancher = bootstrap_rancher(mode);
    let axgate = bootstrap_axgate(mode);
    let harbor = bootstrap_harbor(mode);
    ServiceBundle {
        kube,
        rancher,
        axgate,
        harbor,
    }
}

async fn bootstrap_kube(mode: ServiceMode) -> KubeServiceArc {
    let force_mock = matches!(mode, ServiceMode::Mock);
    if force_mock {
        return Arc::new(KubeMock::new());
    }
    let cfg = KubeRealConfig::from_env();
    match KubeReal::connect(cfg).await {
        // Building the client only reads the kubeconfig; it never authenticates.
        // Probe with a lightweight authed call so an expired/invalid token falls
        // back to mock at boot instead of 401-ing on every dashboard request.
        Ok(real) => match real.health_check().await {
            Ok(_) => {
                eprintln!("[mec] kube: real client connected");
                Arc::new(real)
            }
            Err(e) => {
                kube_fallback_warn(mode, "auth check failed", &e);
                Arc::new(KubeMock::new())
            }
        },
        Err(e) => {
            kube_fallback_warn(mode, "connect failed", &e);
            Arc::new(KubeMock::new())
        }
    }
}

fn kube_fallback_warn(mode: ServiceMode, what: &str, e: &impl std::fmt::Display) {
    if matches!(mode, ServiceMode::Real) {
        eprintln!("[mec] kube real mode forced but {}: {}", what, e);
    }
    eprintln!("[mec] kube: {}, falling back to mock ({})", what, e);
}

fn bootstrap_rancher(mode: ServiceMode) -> RancherServiceArc {
    if matches!(mode, ServiceMode::Mock) {
        return Arc::new(RancherMock::new());
    }
    match RancherRealConfig::from_env() {
        Some(cfg) => match RancherReal::new(cfg) {
            Ok(r) => {
                eprintln!("[mec] rancher: real client configured");
                Arc::new(r)
            }
            Err(e) => {
                eprintln!("[mec] rancher real build failed: {}; using mock", e);
                Arc::new(RancherMock::new())
            }
        },
        None => {
            if matches!(mode, ServiceMode::Real) {
                eprintln!("[mec] rancher real mode forced but env missing; using mock");
            }
            Arc::new(RancherMock::new())
        }
    }
}

fn bootstrap_axgate(mode: ServiceMode) -> AxgateServiceArc {
    if matches!(mode, ServiceMode::Mock) {
        return Arc::new(AxgateMock::new());
    }
    match AxgateRealConfig::from_env() {
        Some(cfg) => {
            eprintln!("[mec] axgate: real client configured");
            Arc::new(AxgateReal::new(cfg))
        }
        None => {
            if matches!(mode, ServiceMode::Real) {
                eprintln!("[mec] axgate real mode forced but env missing; using mock");
            }
            Arc::new(AxgateMock::new())
        }
    }
}

fn bootstrap_harbor(mode: ServiceMode) -> HarborServiceArc {
    if matches!(mode, ServiceMode::Mock) {
        return Arc::new(HarborMock::new());
    }
    match HarborRealConfig::from_env() {
        Some(cfg) => match HarborReal::new(cfg) {
            Ok(r) => {
                eprintln!("[mec] harbor: real client configured");
                Arc::new(r)
            }
            Err(e) => {
                eprintln!("[mec] harbor real build failed: {}; using mock", e);
                Arc::new(HarborMock::new())
            }
        },
        None => {
            if matches!(mode, ServiceMode::Real) {
                eprintln!("[mec] harbor real mode forced but env missing; using mock");
            }
            Arc::new(HarborMock::new())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_mode_returns_mocks() {
        let b = bootstrap(ServiceMode::Mock).await;
        let ips = b.axgate.list_public_ips().await.unwrap();
        assert!(!ips.is_empty());
    }
}
