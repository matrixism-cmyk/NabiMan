pub mod error;
pub mod kube_service;
pub mod kube_mock;
pub mod kube_mock_fixtures;
pub mod kube_real;
pub mod rancher_service;
pub mod rancher_mock;
pub mod rancher_real;
pub mod axgate_service;
pub mod axgate_mock;
pub mod axgate_real;
pub mod harbor_service;
pub mod harbor_mock;
pub mod harbor_real;
pub mod bootstrap;

pub use error::{ServiceError, ServiceResult};
pub use kube_service::{KubeService, KubeServiceArc};
pub use rancher_service::{RancherService, RancherServiceArc};
pub use axgate_service::{AxgateService, AxgateServiceArc};
pub use harbor_service::{HarborService, HarborServiceArc};

pub use kube_mock::KubeMock;
pub use rancher_mock::RancherMock;
pub use axgate_mock::AxgateMock;
pub use harbor_mock::HarborMock;

use std::sync::Arc;

pub struct ServiceBundle {
    pub kube: KubeServiceArc,
    pub rancher: RancherServiceArc,
    pub axgate: AxgateServiceArc,
    pub harbor: HarborServiceArc,
}

impl ServiceBundle {
    pub fn mocks() -> Self {
        Self {
            kube: Arc::new(KubeMock::new()),
            rancher: Arc::new(RancherMock::new()),
            axgate: Arc::new(AxgateMock::new()),
            harbor: Arc::new(HarborMock::new()),
        }
    }
}
