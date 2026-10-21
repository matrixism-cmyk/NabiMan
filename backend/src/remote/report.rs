//! Remote's parts of the support report: how many terminals and servers,
//! never their names, hosts, commands or credentials.

use std::path::Path;

use actix_web::{web, HttpRequest};
use serde::Serialize;
use serde_json::Value;

use crate::support_report::{bounded, Section, State};
use crate::terminal::session::PtyStore;

type Collect = Box<dyn FnOnce() -> Value + Send>;

fn json<T: Serialize>(section: Section<T>) -> Value {
    serde_json::to_value(section).unwrap_or(Value::Null)
}

/// The terminal registry, under `terminal_registry`.
pub(super) fn terminals(req: &HttpRequest) -> Collect {
    let store = req.app_data::<web::Data<PtyStore>>().cloned();
    Box::new(move || json(store.map_or(Section::missing(State::Unavailable), |s| terminals_of(&s))))
}

/// The server inventory, under `server_inventory`.
pub(super) fn servers(_: &HttpRequest) -> Collect {
    Box::new(|| json(inventory(&nabiman_server::data_dir().join("remote_servers.json"))))
}

#[derive(Serialize)]
struct Terminals {
    registered: usize,
    local: usize,
    remote: usize,
    attached_websockets: u64,
}
fn terminals_of(store: &PtyStore) -> Section<Terminals> {
    let Ok(terminals) = store.try_lock() else {
        return Section::missing(State::Busy);
    };
    let remote = terminals.values().filter(|t| t.ssh_cmd.is_some()).count();
    Section::ready(Terminals {
        registered: terminals.len(),
        local: terminals.len() - remote,
        remote,
        attached_websockets: terminals
            .values()
            .fold(0u64, |sum, t| sum.saturating_add(t.ws_count as u64)),
    })
}
#[derive(Serialize)]
struct Servers {
    registered: usize,
    host_trust_configured: usize,
    password_configured: usize,
}
fn inventory(path: &Path) -> Section<Servers> {
    let bytes = match bounded(path, 8 * 1024 * 1024) {
        Ok(b) => b,
        Err(state) => return Section::missing(state),
    };
    let servers: Vec<crate::models::RemoteServer> = match serde_json::from_slice(&bytes) {
        Ok(rows) => rows,
        Err(_) => return Section::missing(State::Unavailable),
    };
    Section::ready(Servers {
        registered: servers.len(),
        host_trust_configured: servers.iter().filter(|s| s.host_trust.is_some()).count(),
        password_configured: servers
            .iter()
            .filter(|s| !s.password_enc.is_empty())
            .count(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn inventory_summary_omits_identifiers_credentials_and_raw_errors() {
        let path = std::env::temp_dir().join(format!("nabiman-support-{}.json", uuid::Uuid::new_v4()));
        let private = "CANARY_PRIVATE_HOST_USER_PASSWORD";
        let raw = serde_json::json!([{"id":private,"name":private,"host":private,"port":22,"user":private,"auth_method":"password","password_enc":private,"tags":[private],"memo":private,"created_at":private}]);
        std::fs::write(&path, serde_json::to_vec(&raw).unwrap()).unwrap();
        let summary = serde_json::to_string(&inventory(&path)).unwrap();
        assert!(!summary.contains(private));
        assert!(summary.contains("\"registered\":1"));
        assert!(summary.contains("\"password_configured\":1"));
        std::fs::write(&path, private).unwrap();
        assert_eq!(
            serde_json::to_value(inventory(&path)).unwrap(),
            serde_json::json!({"state":"unavailable","data":null})
        );
        std::fs::write(&path, vec![b'x'; 8 * 1024 * 1024 + 1]).unwrap();
        assert_eq!(
            serde_json::to_value(inventory(&path)).unwrap(),
            serde_json::json!({"state":"too_large","data":null})
        );
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn terminal_metadata_never_enters_the_summary() {
        let terminal=serde_json::from_value(serde_json::json!({"tmux_name":"CANARY_TERMINAL","owner":"CANARY_OWNER","owner_id":"CANARY_ID","label":"CANARY_LABEL","ssh_cmd":"CANARY_COMMAND","title":"CANARY_TITLE"})).unwrap();
        let terminals = Arc::new(Mutex::new(std::collections::HashMap::from([(
            "CANARY_SESSION".into(),
            terminal,
        )])));
        let summary = serde_json::to_string(&terminals_of(&terminals)).unwrap();
        assert!(!summary.contains("CANARY"));
        assert!(summary.contains("\"remote\":1"));
    }
}
