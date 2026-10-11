mod client;
mod ingress_ops;
mod live;
mod namespaces;
mod nodes;
mod pvcs;
mod quota;
mod rbac;
mod services_ops;
mod workloads;

pub use client::{KubeReal, KubeRealConfig};
