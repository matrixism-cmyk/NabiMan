use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MecAuditLog {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub user: String,
    pub action: String,
    pub resource_type: String,
    pub resource_id: String,
    pub status: AuditStatus,
    pub duration_ms: u64,
    pub input: Option<serde_json::Value>,
    pub output: Option<serde_json::Value>,
    pub error: Option<String>,
    pub operations: Vec<OperationLog>,
    pub source_ip: Option<String>,
    pub user_agent: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AuditStatus {
    Success,
    Failed,
    Partial,
}

impl AuditStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failed => "failed",
            Self::Partial => "partial",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "success" => Some(Self::Success),
            "failed" => Some(Self::Failed),
            "partial" => Some(Self::Partial),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationLog {
    pub name: String,
    pub target: String,
    pub status: String,
    pub duration_ms: u64,
    pub details: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AuditQuery {
    pub user: Option<String>,
    pub action: Option<String>,
    pub resource_type: Option<String>,
    pub resource_id: Option<String>,
    pub status: Option<String>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_roundtrip() {
        for s in [AuditStatus::Success, AuditStatus::Failed, AuditStatus::Partial] {
            assert_eq!(AuditStatus::from_str(s.as_str()), Some(s));
        }
    }

    #[test]
    fn status_unknown() {
        assert!(AuditStatus::from_str("bogus").is_none());
    }
}
