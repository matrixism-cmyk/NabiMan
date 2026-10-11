//! Policy and its recent changes share one atomic commit; not an independent audit archive.
use super::ScopeSettings;
use crate::models::NabimanUser;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Change {
    pub revision: String,
    pub previous_revision: String,
    pub at: String,
    pub actor_id: String,
    pub actor: String,
    pub reason: String,
    pub fields: Vec<FieldChange>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FieldChange {
    pub field: String,
    pub before: serde_json::Value,
    pub after: serde_json::Value,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Record {
    pub policy: ScopeSettings,
    pub revision: String,
    pub history: Vec<Change>,
}
impl Record {
    pub fn legacy(key: &str, policy: ScopeSettings) -> Self {
        use sha2::{Digest, Sha256};
        let bytes = serde_json::to_vec(&(key, &policy)).unwrap();
        Self {
            policy,
            revision: format!("legacy:{}", hex::encode(Sha256::digest(bytes))),
            history: vec![],
        }
    }
    pub fn view(&self) -> serde_json::Value {
        let mut value = serde_json::to_value(&self.policy).unwrap();
        value["revision"] = self.revision.clone().into();
        value
    }
    pub fn replace(&mut self, policy: ScopeSettings, actor: &NabimanUser, reason: String) {
        let before = serde_json::to_value(&self.policy).unwrap();
        let after = serde_json::to_value(&policy).unwrap();
        let fields = after
            .as_object()
            .unwrap()
            .iter()
            .filter_map(|(field, value)| {
                (before[field] != *value).then(|| FieldChange {
                    field: field.clone(),
                    before: before[field].clone(),
                    after: value.clone(),
                })
            })
            .collect();
        let revision = uuid::Uuid::new_v4().to_string();
        self.history.push(Change {
            revision: revision.clone(),
            previous_revision: self.revision.clone(),
            at: chrono::Utc::now().to_rfc3339(),
            actor_id: actor.id.clone(),
            actor: actor.username.clone(),
            reason,
            fields,
        });
        if self.history.len() > 100 {
            self.history.drain(..self.history.len() - 100);
        }
        self.revision = revision;
        self.policy = policy;
    }
}
pub(super) fn load() -> Result<BTreeMap<String, Record>, String> {
    let bytes = match std::fs::read(super::path()) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(_) => return Err("소속 설정 파일을 읽을 수 없습니다".into()),
    };
    let decode = || -> Result<BTreeMap<String, Record>, serde_json::Error> {
        let raw: BTreeMap<String, serde_json::Value> = serde_json::from_slice(&bytes)?;
        raw.into_iter()
            .map(|(key, value)| {
                let record = if value.get("policy").is_some() {
                    serde_json::from_value(value)?
                } else {
                    Record::legacy(&key, serde_json::from_value(value)?)
                };
                Ok((key, record))
            })
            .collect()
    };
    let records = decode().map_err(|_| "소속 설정 파일을 읽을 수 없습니다".to_string())?;
    if records.values().any(|r| {
        r.revision.is_empty()
            || r.history.len() > 100
            || r.history.last().is_some_and(|c| c.revision != r.revision)
    }) {
        return Err("소속 설정 이력이 올바르지 않습니다".into());
    }
    Ok(records)
}
pub(super) fn save(records: &BTreeMap<String, Record>) -> Result<(), String> {
    serde_json::to_vec_pretty(records)
        .map_err(|e| e.to_string())
        .and_then(|bytes| {
            crate::private_storage::write(&super::path(), &bytes).map_err(|e| e.to_string())
        })
}

#[cfg(test)]
#[path = "scope_policy_store_tests.rs"]
mod tests;
