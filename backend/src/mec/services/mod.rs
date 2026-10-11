pub mod axgate_mock;
pub mod axgate_real;
pub mod axgate_service;
pub mod bootstrap;
pub mod error;
pub mod gpu_dcgm;
pub mod harbor_mock;
pub mod harbor_real;
pub mod harbor_service;
pub mod kube_mock;
pub mod kube_mock_fixtures;
pub mod kube_real;
pub mod kube_service;
pub mod kubeconfig_expiry;
pub mod rancher_mock;
pub mod rancher_real;
pub mod rancher_service;
pub mod unavailable;

pub use axgate_service::AxgateServiceArc;
pub use error::{ServiceError, ServiceResult};
pub use harbor_service::HarborServiceArc;
pub use kube_service::KubeServiceArc;
pub use rancher_service::RancherServiceArc;

pub use axgate_mock::AxgateMock;
pub use harbor_mock::HarborMock;
pub use kube_mock::KubeMock;
pub use rancher_mock::RancherMock;

use std::sync::Arc;

pub struct ServiceBundle {
    pub kube: KubeServiceArc,
    pub rancher: RancherServiceArc,
    pub axgate: AxgateServiceArc,
    pub harbor: HarborServiceArc,
}

impl ServiceBundle {
    #[allow(dead_code)]
    pub fn mocks() -> Self {
        Self {
            kube: Arc::new(KubeMock::new()),
            rancher: Arc::new(RancherMock::new()),
            axgate: Arc::new(AxgateMock::new()),
            harbor: Arc::new(HarborMock::new()),
        }
    }
}
