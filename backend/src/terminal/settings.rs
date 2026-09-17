//! Per-user terminal preferences, stored next to the session registry.

use std::collections::HashMap;

use super::session::data_dir;

pub(crate) fn default_scrollback() -> u32 { 5000 }
fn default_true() -> bool { true }
fn default_ka_interval() -> u64 { 30 }
fn default_font_size() -> u32 { 14 }

/// Preferences that outlive a browser: how much history to keep, whether the
/// session is kept alive server-side, and how big the text is.
#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct TerminalSettings {
    #[serde(default = "default_scrollback")]
    pub scrollback_lines: u32,
    #[serde(default = "default_true")]
    pub keepalive: bool,
    #[serde(default = "default_ka_interval")]
    pub keepalive_interval_secs: u64,
    /// Seconds a detached session survives; 0 = never auto-closed.
    #[serde(default)]
    pub idle_timeout_secs: u64,
    #[serde(default = "default_font_size")]
    pub font_size: u32,
    /// Replay the server-side history when a window reopens with an empty buffer.
    #[serde(default = "default_true")]
    pub restore_scrollback: bool,
}

impl Default for TerminalSettings {
    fn default() -> Self {
        Self {
            scrollback_lines: default_scrollback(),
            keepalive: true,
            keepalive_interval_secs: default_ka_interval(),
            idle_timeout_secs: 0,
            font_size: default_font_size(),
            restore_scrollback: true,
        }
    }
}

fn settings_file() -> String { format!("{}/terminal_settings.json", data_dir()) }

fn load_all_settings() -> HashMap<String, TerminalSettings> {
    std::fs::read_to_string(settings_file())
        .ok()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or_default()
}

pub fn settings_for(user: &str) -> TerminalSettings {
    load_all_settings().remove(user).unwrap_or_default()
}

pub(crate) fn save_settings_for(user: &str, settings: &TerminalSettings) -> Result<(), String> {
    let mut all = load_all_settings();
    all.insert(user.to_string(), settings.clone());
    let json = serde_json::to_string_pretty(&all).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(data_dir()).map_err(|e| e.to_string())?;
    std::fs::write(settings_file(), json).map_err(|e| e.to_string())
}
