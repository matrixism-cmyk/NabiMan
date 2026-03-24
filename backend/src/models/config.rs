use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct ServiceDefinition {
    pub id: String,
    pub display_name: String,
    pub config_paths: Vec<String>,
    #[serde(skip_deserializing)]
    pub systemd_names: Vec<String>,
    #[serde(skip_deserializing)]
    pub process_name: String,
    #[serde(skip_deserializing)]
    pub binary_names: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_running: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_installed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_found: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detected_by: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ServiceConfig {
    pub service_name: String,
    pub config_path: String,
    pub content: String,
    pub is_running: bool,
}

#[derive(Deserialize)]
pub struct UpdateConfigRequest {
    pub content: String,
}
