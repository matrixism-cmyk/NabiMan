use serde::Serialize;
use chrono::{DateTime, Utc};

#[derive(Serialize)]
pub struct MecResponse<T: Serialize> {
    pub data: Option<T>,
    pub meta: MecMeta,
    pub error: Option<MecError>,
}

#[derive(Serialize)]
pub struct MecListResponse<T: Serialize> {
    pub data: Vec<T>,
    pub meta: MecListMeta,
}

#[derive(Serialize)]
pub struct MecMeta {
    pub timestamp: DateTime<Utc>,
    pub request_id: String,
}

#[derive(Serialize)]
pub struct MecListMeta {
    pub total: usize,
    pub timestamp: DateTime<Utc>,
    pub request_id: String,
}

#[derive(Serialize, Clone)]
pub struct MecError {
    pub code: String,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

impl MecError {
    pub fn new(code: &str, message: &str) -> Self {
        Self {
            code: code.to_string(),
            message: message.to_string(),
            details: None,
        }
    }

    pub fn with_details(code: &str, message: &str, details: serde_json::Value) -> Self {
        Self {
            code: code.to_string(),
            message: message.to_string(),
            details: Some(details),
        }
    }
}

impl<T: Serialize> MecResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            data: Some(data),
            meta: MecMeta::new(),
            error: None,
        }
    }

    pub fn error(err: MecError) -> Self {
        Self {
            data: None,
            meta: MecMeta::new(),
            error: Some(err),
        }
    }
}

impl<T: Serialize> MecListResponse<T> {
    pub fn new(data: Vec<T>) -> Self {
        Self {
            meta: MecListMeta {
                total: data.len(),
                timestamp: Utc::now(),
                request_id: new_request_id(),
            },
            data,
        }
    }
}

impl MecMeta {
    pub fn new() -> Self {
        Self {
            timestamp: Utc::now(),
            request_id: new_request_id(),
        }
    }
}

impl Default for MecMeta {
    fn default() -> Self {
        Self::new()
    }
}

fn new_request_id() -> String {
    format!("req-{}", &uuid::Uuid::new_v4().to_string()[..8])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_response_ok() {
        let r = MecResponse::ok(42);
        assert!(r.data.is_some());
        assert!(r.error.is_none());
    }

    #[test]
    fn test_response_error() {
        let r = MecResponse::<i32>::error(MecError::new("E", "msg"));
        assert!(r.data.is_none());
        assert!(r.error.is_some());
    }

    #[test]
    fn test_list_response_total() {
        let r = MecListResponse::new(vec![1u32, 2, 3]);
        assert_eq!(r.meta.total, 3);
    }
}
