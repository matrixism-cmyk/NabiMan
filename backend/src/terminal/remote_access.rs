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
    /// `ssh` or `telnet`; bindings saved before Telnet existed are SSH.
    #[serde(default = "ssh")]
    terminal: String,
}
fn ssh() -> String {
    "ssh".into()
}

/// The approved host key of a server reached over SSH, checked for being
/// current and consistent.
fn approved_key(server: &RemoteServer) -> Result<(String, String), String> {
    let trust = server.host_trust.as_ref().ok_or("Host key is not verified")?;
    if server.host_trust_version.is_empty() {
        return Err("Review and approve this server key again before opening a terminal".into());
    }
    let (public_key, fingerprint) = crate::remote_host_trust::parse_key(&trust.public_key)?;
    if fingerprint != trust.fingerprint {
        return Err("Stored host trust is inconsistent".into());
    }
    Ok((public_key, fingerprint))
}

/// Whether SSH to this server may be used at all (its files over SFTP).
pub(crate) fn ssh_approved(server: &RemoteServer) -> Result<(), String> {
    approved_key(server).map(drop)
}

impl RemoteBinding {
    pub(crate) fn from_server(server: &RemoteServer) -> Result<Self, String> {
        // Telnet has no host key to approve; it is bound to the address alone.
        let (public_key, fingerprint) = match server.terminal.as_str() {
            "ssh" => approved_key(server)?,
            "telnet" => (String::new(), String::new()),
            _ => return Err("This server has no terminal access set up".into()),
        };
        Ok(Self {
            server_id: server.id.clone(),
            host: server.host.clone(),
            port: server.port,
            user: server.user.clone(),
            auth_method: server.auth_method.clone(),
            approval_version: server.host_trust_version.clone(),
            public_key,
            fingerprint,
            terminal: server.terminal.clone(),
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
            && self.terminal == other.terminal
    }
    /// A one-off Telnet target typed in the ad-hoc dialog, not a saved server.
    /// Telnet has no host key to verify, so it is bound to the address alone.
    pub(crate) fn adhoc_telnet(host: &str, port: u16, user: &str) -> Self {
        Self {
            server_id: String::new(),
            host: host.to_string(),
            port,
            user: user.to_string(),
            auth_method: "none".into(),
            approval_version: String::new(),
            public_key: String::new(),
            fingerprint: String::new(),
            terminal: "telnet".into(),
        }
    }
    pub(crate) fn server_id(&self) -> &str {
        &self.server_id
    }
    pub(crate) fn valid(&self) -> bool {
        if self.server_id.is_empty() {
            // An ad-hoc target has no saved server to drift from; only Telnet
            // may be ad-hoc (SSH/SFTP always bind to a verified saved server).
            return self.terminal == "telnet";
        }
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

/// Whether `user` may see and use `session`: anyone who could before, except
/// that a remote-only member gets only their own sessions on servers they may
/// use — not one on this machine, nor anybody else's shared one. An assignment
/// switched off or expired ends them.
pub(crate) fn member_may_use(user: &crate::models::NabimanUser, session: &super::session::TmuxSession) -> bool {
    if !crate::member_servers::is_member(user) {
        return true;
    }
    session.owned_by(&user.id)
        && session.remote_binding.as_ref().is_some_and(|b| {
            crate::remote_servers::find_server(&b.server_id)
                .ok()
                .flatten()
                .is_some_and(|s| crate::member_servers::usable(user, &s))
        })
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
    fn telnet_binds_to_the_address_without_a_key_and_none_cannot_bind() {
        let mut telnet = server();
        telnet.terminal = "telnet".into();
        telnet.host_trust = None;
        let binding = RemoteBinding::from_server(&telnet).unwrap();
        assert!(binding.matches(&telnet));
        let mut moved = telnet.clone();
        moved.port = 2323;
        assert!(!binding.matches(&moved));
        let mut files_only = server();
        files_only.terminal = "none".into();
        assert!(RemoteBinding::from_server(&files_only).is_err());
        // A binding saved before protocols existed is an SSH one.
        let ssh = server();
        let mut old = serde_json::to_value(RemoteBinding::from_server(&ssh).unwrap()).unwrap();
        old.as_object_mut().unwrap().remove("terminal");
        let old: RemoteBinding = serde_json::from_value(old).unwrap();
        assert!(old.matches(&ssh));
    }
    #[test]
    fn approval_and_exact_target_changes_deny_old_binding() {
        let server = server();
        let binding = RemoteBinding::from_server(&server).unwrap();
        assert!(binding.matches(&server));
        for change in 0..8 {
            let mut candidate = server.clone();
            match change {
                0 => candidate.host_trust = None,
                1 => candidate.host_trust_version = uuid::Uuid::new_v4().to_string(),
                2 => candidate.host.push_str("-new"),
                3 => candidate.port += 1,
                4 => candidate.user = "other".into(),
                5 => candidate.auth_method = "password".into(),
                6 => candidate.terminal = "telnet".into(),
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
