//! Per-console delegation: which non-admin accounts may open a given VM
//! console. Administrators always may; everyone else only a console they were
//! granted. Keyed by console key (a built-in bridge name or a registered id).

use std::collections::HashMap;
use std::path::PathBuf;

fn file() -> PathBuf {
    nabiman_server::data_dir().join("vnc_access.json")
}

/// console key → allowed user ids.
pub fn grants() -> HashMap<String, Vec<String>> {
    std::fs::read(file())
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn save(map: &HashMap<String, Vec<String>>) -> Result<(), String> {
    let json = serde_json::to_vec_pretty(map).map_err(|e| e.to_string())?;
    crate::private_storage::write(&file(), &json).map_err(|e| format!("Cannot save console access: {e}"))
}

/// The user ids granted access to one console.
pub fn allowed_users(console: &str) -> Vec<String> {
    grants().remove(console).unwrap_or_default()
}

/// Whether a specific account was granted this console.
pub fn allowed(user_id: &str, console: &str) -> bool {
    !user_id.is_empty() && grants().get(console).is_some_and(|u| u.iter().any(|x| x == user_id))
}

/// Replace the grant list for a console (empty clears it).
pub fn set(console: &str, users: Vec<String>) -> Result<(), String> {
    let mut map = grants();
    if users.is_empty() {
        map.remove(console);
    } else {
        map.insert(console.to_string(), users);
    }
    save(&map)
}

/// Drop a console's grants entirely (used when a registered console is deleted).
pub fn forget(console: &str) {
    let mut map = grants();
    if map.remove(console).is_some() {
        let _ = save(&map);
    }
}
