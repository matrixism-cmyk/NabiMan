//! Saved-target approval bound to a terminal, including restored attachments.
use crate::models::RemoteServer;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct RemoteBinding {
    server_id: String,
    host: String,
    port: u16,
    user: String,
    auth_method: String,
    approval_version: String,
    public_key: String,
    fingerprint: String,
}
impl RemoteBinding {
    pub(crate) fn from_server(server: &RemoteServer) -> Result<Self, String> {
        let trust = server
            .host_trust
            .as_ref()
            .ok_or("Host key is not verified")?;
        if server.host_trust_version.is_empty() {
            return Err(
                "Review and approve this server key again before opening a terminal".into(),
            );
        }
        let (public_key, fingerprint) = crate::remote_host_trust::parse_key(&trust.public_key)?;
        if fingerprint != trust.fingerprint {
            return Err("Stored host trust is inconsistent".into());
        }
        Ok(Self {
            server_id: server.id.clone(),
            host: server.host.clone(),
            port: server.port,
            user: server.user.clone(),
            auth_method: server.auth_method.clone(),
            approval_version: server.host_trust_version.clone(),
            public_key,
            fingerprint,
        })
    }
    fn matches(&self, current: &RemoteServer) -> bool {
        let Ok(other) = Self::from_server(current) else {
            return false;
        };
        self.server_id == other.server_id
            && self.host == other.host
            && self.port == other.port
            && self.user == other.user
            && self.auth_method == other.auth_method
            && self.approval_version == other.approval_version
            && self.public_key == other.public_key
            && self.fingerprint == other.fingerprint
    }
    pub(crate) fn valid(&self) -> bool {
        crate::remote_servers::find_server(&self.server_id)
            .ok()
            .flatten()
            .is_some_and(|server| self.matches(&server))
    }
}

pub(crate) fn allowed(session: &super::session::TmuxSession) -> bool {
    match &session.remote_binding {
        Some(binding) => binding.valid(),
        None => session.ssh_cmd.is_none() && session.label == "local", // Never infer a legacy remote target from a label/command.
    }
}

pub(crate) fn attachment_allowed(store: &super::session::PtyStore, id: &str, name: &str) -> bool {
    let session = store.lock().ok().and_then(|map| map.get(id).cloned());
    session.is_some_and(|session| session.tmux_name == name && allowed(&session))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn server() -> RemoteServer {
        let key =
            russh::keys::PrivateKey::random(&mut rand10::rng(), russh::keys::Algorithm::Ed25519)
                .unwrap();
        use russh::keys::PublicKeyBase64;
        let public = key.public_key();
        let (public_key, fingerprint) = crate::remote_host_trust::parse_key(&format!(
            "{} {}",
            public.algorithm().as_str(),
            public.public_key_base64()
        ))
        .unwrap();
        serde_json::from_value(serde_json::json!({"id":"remote","name":"remote","host":"localhost","port":22,"user":"root","auth_method":"key","tags":[],"memo":"","created_at":"now","host_trust_version":uuid::Uuid::new_v4().to_string(),"host_trust":{"public_key":public_key,"fingerprint":fingerprint,"verified_by":"admin","verified_at":"now","reason":"console"}})).unwrap()
    }
    #[test]
    fn approval_and_exact_target_changes_deny_old_binding() {
        let server = server();
        let binding = RemoteBinding::from_server(&server).unwrap();
        assert!(binding.matches(&server));
        for change in 0..7 {
            let mut candidate = server.clone();
            match change {
                0 => candidate.host_trust = None,
                1 => candidate.host_trust_version = uuid::Uuid::new_v4().to_string(),
                2 => candidate.host.push_str("-new"),
                3 => candidate.port += 1,
                4 => candidate.user = "other".into(),
                5 => candidate.auth_method = "password".into(),
                _ => candidate.host_trust.as_mut().unwrap().public_key = "invalid".into(),
            }
            assert!(!binding.matches(&candidate));
        }
        let mut renamed = server.clone();
        renamed.name = "Renamed".into();
        renamed.memo = "Memo".into();
        assert!(binding.matches(&renamed));
        let restored: RemoteBinding =
            serde_json::from_str(&serde_json::to_string(&binding).unwrap()).unwrap();
        assert!(restored.matches(&server));
    }
    #[test]
    fn legacy_approval_and_legacy_remote_session_are_denied() {
        let mut record = server();
        record.host_trust_version.clear();
        assert!(RemoteBinding::from_server(&record).is_err());
        let mut session: super::super::session::TmuxSession = serde_json::from_value(serde_json::json!({"tmux_name":"test","owner":"admin","label":"local","ssh_cmd":"ssh old"})).unwrap();
        assert!(!allowed(&session));
        session.ssh_cmd = None;
        assert!(allowed(&session));
    }
}
