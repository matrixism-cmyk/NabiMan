pub mod templates;
pub mod orchestrator;

pub use orchestrator::StarterKitOrchestrator;
pub use templates::{ubuntu_ssh_manifests, vscode_manifests};
