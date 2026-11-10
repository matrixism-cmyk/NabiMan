//! What an administrative change changed, for the audit record: the
//! resource, its id, the action, and each changed field with its old and
//! new value — or only that it changed, for a secret. Handlers note it on
//! the request; the audit middleware puts it in the record (and so in the
//! hash chain) when the request succeeded.

use actix_web::{HttpMessage, HttpRequest};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Longest value kept, in characters.
const MAX_VALUE: usize = 120;
/// Fields that change by themselves or say nothing to a reviewer.
const IGNORED: &[&str] = &["updated_at", "created_at", "last_login", "last_seen", "share_authorization_version", "next_run", "last_slot"];
/// Fields that name the thing, kept when it is deleted.
const LABELS: &[&str] = &["username", "name", "host", "domain", "role", "user", "port"];

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct FieldChange {
    pub field: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub old: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new: Option<String>,
    /// A secret: the values are not kept, only that it changed.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub secret: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Change {
    pub resource: String,
    pub id: String,
    /// `create`, `update`, `delete`, or what else was done (`close`).
    pub action: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<FieldChange>,
}

fn secret(name: &str) -> bool {
    crate::observability::redact_key(name) || name.ends_with("_enc") || name.ends_with("_hash")
}

fn shown(v: &Value) -> Option<String> {
    let text = match v {
        Value::Null => return None,
        Value::String(s) => s.clone(),
        other => other.to_string(),
    };
    let text = crate::observability::redact_text(&text);
    Some(if text.chars().count() > MAX_VALUE { format!("{}…", text.chars().take(MAX_VALUE).collect::<String>()) } else { text })
}

fn empty(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::Bool(b) => !b,
        Value::String(s) => s.is_empty(),
        Value::Array(a) => a.is_empty(),
        Value::Object(o) => o.is_empty(),
        Value::Number(_) => false,
    }
}

impl Change {
    pub fn new(resource: &str, id: &str, action: &str) -> Self {
        Self { resource: resource.into(), id: id.into(), action: action.into(), fields: vec![] }
    }

    /// One field, old → new (`None`: absent); a secret keeps no values.
    pub fn field(mut self, name: &str, old: Option<&str>, new: Option<&str>) -> Self {
        if old == new {
            return self;
        }
        let is_secret = secret(name);
        let keep = |v: Option<&str>| if is_secret { None } else { v.map(|v| crate::observability::redact_text(v)) };
        self.fields.push(FieldChange { field: name.into(), old: keep(old), new: keep(new), secret: is_secret });
        self
    }

    /// The difference between two records (top-level fields): created when
    /// there was none before, deleted when there is none after.
    pub fn between<T: Serialize>(resource: &str, id: &str, before: Option<&T>, after: Option<&T>) -> Self {
        let json = |r: Option<&T>| r.and_then(|r| serde_json::to_value(r).ok()).and_then(|v| v.as_object().cloned());
        let (old, new) = (json(before), json(after));
        let action = match (&old, &new) {
            (None, Some(_)) => "create",
            (Some(_), None) => "delete",
            _ => "update",
        };
        let mut change = Change::new(resource, id, action);
        let (old, new) = (old.unwrap_or_default(), new.unwrap_or_default());
        let mut names: Vec<&String> = old.keys().chain(new.keys()).collect();
        names.sort();
        names.dedup();
        for name in names {
            if IGNORED.contains(&name.as_str()) || (action == "delete" && !LABELS.contains(&name.as_str())) {
                continue;
            }
            let (o, n) = (old.get(name).unwrap_or(&Value::Null), new.get(name).unwrap_or(&Value::Null));
            if o == n || (action == "create" && empty(n)) {
                continue;
            }
            let is_secret = secret(name);
            change.fields.push(FieldChange {
                field: name.clone(),
                old: if is_secret || action == "create" { None } else { shown(o) },
                new: if is_secret || action == "delete" { None } else { shown(n) },
                secret: is_secret,
            });
        }
        change
    }
}

/// Put `change` on the request for the audit record.
pub fn note(req: &HttpRequest, change: Change) {
    req.extensions_mut().insert(change);
}

/// What a request noted, for the audit middleware.
pub(crate) fn noted(req: &HttpRequest) -> Option<Change> {
    req.extensions().get::<Change>().cloned()
}

#[cfg(test)]
#[path = "audit_change_tests.rs"]
mod tests;
