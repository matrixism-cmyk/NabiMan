mod client;
mod nodes;
mod namespaces;
mod quota;
mod services_ops;
mod rbac;
mod workloads;
mod ingress_ops;
mod pvcs;

pub use client::{KubeReal, KubeRealConfig};
