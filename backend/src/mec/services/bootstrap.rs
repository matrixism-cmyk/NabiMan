use super::axgate_mock::AxgateMock;
use super::axgate_real::{AxgateReal, AxgateRealConfig};
use super::harbor_mock::HarborMock;
use super::harbor_real::{HarborReal, HarborRealConfig};
use super::kube_mock::KubeMock;
use super::kube_real::{KubeReal, KubeRealConfig};
use super::rancher_mock::RancherMock;
use super::rancher_real::{RancherReal, RancherRealConfig};
use super::unavailable::UnavailableService;
use super::{AxgateServiceArc, HarborServiceArc, KubeServiceArc, RancherServiceArc, ServiceBundle};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceMode {
    Mock,
    Real,
    Auto,
}

impl ServiceMode {
    pub fn parse(value: Option<&str>) -> Result<Self, String> {
        match value {
            Some("real") => Ok(Self::Real),
            Some("mock") => Ok(Self::Mock),
            Some("auto") | None => Ok(Self::Auto),
            Some(_) => Err("NABIMAN_MEC_MODE must be real, auto, or mock".into()),
        }
    }
    pub fn from_env() -> Result<Self, String> {
        Self::parse(std::env::var("NABIMAN_MEC_MODE").ok().as_deref())
    }
}

/// Demo data requires explicit mock mode. Configured real clients remain live
/// across connection failures, allowing recovery without a server restart.
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
    kube_from_config(KubeRealConfig::from_env()).await
}
async fn kube_from_config(cfg: KubeRealConfig) -> KubeServiceArc {
    match KubeReal::connect(cfg).await {
        Ok(real) => {
            eprintln!("[mec] kube: real client configured; health is checked live");
            Arc::new(real)
        }
        Err(_) => {
            eprintln!("[mec] kube: configuration failed; demo fallback disabled");
            Arc::new(UnavailableService::new(
                "kubernetes",
                "configuration unavailable; check kubeconfig",
            ))
        }
    }
}

fn bootstrap_rancher(mode: ServiceMode) -> RancherServiceArc {
    if matches!(mode, ServiceMode::Mock) {
        return Arc::new(RancherMock::new());
    }
    rancher_from_config(RancherRealConfig::from_env())
}

fn rancher_from_config(config: Option<RancherRealConfig>) -> RancherServiceArc {
    match config {
        Some(cfg) => match RancherReal::new(cfg) {
            Ok(r) => {
                eprintln!("[mec] rancher: real client configured");
                Arc::new(r)
            }
            Err(_) => {
                eprintln!("[mec] rancher: configuration failed; demo fallback disabled");
                Arc::new(UnavailableService::new(
                    "rancher",
                    "configuration unavailable",
                ))
            }
        },
        None => Arc::new(UnavailableService::new(
            "rancher",
            "configure URL and token",
        )),
    }
}

fn bootstrap_axgate(mode: ServiceMode) -> AxgateServiceArc {
    if matches!(mode, ServiceMode::Mock) {
        return Arc::new(AxgateMock::new());
    }
    axgate_from_config(AxgateRealConfig::from_env())
}

fn axgate_from_config(config: Option<AxgateRealConfig>) -> AxgateServiceArc {
    match config {
        Some(cfg) => {
            eprintln!("[mec] axgate: real client configured");
            Arc::new(AxgateReal::new(cfg))
        }
        None => Arc::new(UnavailableService::new(
            "axgate",
            "configure SSH connection credentials",
        )),
    }
}

fn bootstrap_harbor(mode: ServiceMode) -> HarborServiceArc {
    if matches!(mode, ServiceMode::Mock) {
        return Arc::new(HarborMock::new());
    }
    harbor_from_config(HarborRealConfig::from_env())
}

fn harbor_from_config(config: Option<HarborRealConfig>) -> HarborServiceArc {
    match config {
        Some(cfg) => match HarborReal::new(cfg) {
            Ok(r) => {
                eprintln!("[mec] harbor: real client configured");
                Arc::new(r)
            }
            Err(_) => {
                eprintln!("[mec] harbor: configuration failed; demo fallback disabled");
                Arc::new(UnavailableService::new(
                    "harbor",
                    "configuration unavailable",
                ))
            }
        },
        None => Arc::new(UnavailableService::new(
            "harbor",
            "configure URL and credentials",
        )),
    }
}

#[cfg(test)]
#[path = "bootstrap_tests.rs"]
mod tests;
