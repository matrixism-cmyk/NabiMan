mod client;
mod nodes;
mod namespaces;
mod quota;
mod services_ops;
mod rbac;
mod workloads;
mod ingress_ops;
mod pvcs;
mod live;

pub use client::{KubeReal, KubeRealConfig};
