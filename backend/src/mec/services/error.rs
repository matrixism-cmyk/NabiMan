use std::fmt;

#[derive(Debug, Clone)]
pub enum ServiceError {
    NotFound(String),
    Conflict(String),
    Upstream { source: String, message: String },
    #[allow(dead_code)]
    Unauthorized(String),
    InvalidInput(String),
    Unavailable(String),
    Internal(String),
}

pub type ServiceResult<T> = Result<T, ServiceError>;

impl fmt::Display for ServiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(s) => write!(f, "not found: {}", s),
            Self::Conflict(s) => write!(f, "conflict: {}", s),
            Self::Upstream { source, message } => {
                write!(f, "upstream {} error: {}", source, message)
            }
            Self::Unauthorized(s) => write!(f, "unauthorized: {}", s),
            Self::InvalidInput(s) => write!(f, "invalid input: {}", s),
            Self::Unavailable(s) => write!(f, "unavailable: {}", s),
            Self::Internal(s) => write!(f, "internal: {}", s),
        }
    }
}

impl std::error::Error for ServiceError {}

impl ServiceError {
    pub fn http_status(&self) -> u16 {
        match self {
            Self::NotFound(_) => 404,
            Self::Conflict(_) => 409,
            Self::Unauthorized(_) => 401,
            Self::InvalidInput(_) => 400,
            Self::Unavailable(_) => 503,
            Self::Upstream { .. } => 502,
            Self::Internal(_) => 500,
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound(_) => "NOT_FOUND",
            Self::Conflict(_) => "CONFLICT",
            Self::Upstream { .. } => "UPSTREAM_ERROR",
            Self::Unauthorized(_) => "UNAUTHORIZED",
            Self::InvalidInput(_) => "INVALID_INPUT",
            Self::Unavailable(_) => "UNAVAILABLE",
            Self::Internal(_) => "INTERNAL_ERROR",
        }
    }

    #[allow(dead_code)]
    pub fn upstream(source: &str, message: impl Into<String>) -> Self {
        Self::Upstream {
            source: source.to_string(),
            message: message.into(),
        }
    }
}

impl From<rusqlite::Error> for ServiceError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Internal(format!("sqlite: {}", e))
    }
}

impl From<serde_json::Error> for ServiceError {
    fn from(e: serde_json::Error) -> Self {
        Self::Internal(format!("json: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_status_mapping() {
        assert_eq!(ServiceError::NotFound("x".into()).http_status(), 404);
        assert_eq!(ServiceError::Conflict("x".into()).http_status(), 409);
        assert_eq!(ServiceError::Internal("x".into()).http_status(), 500);
        assert_eq!(
            ServiceError::Upstream { source: "k8s".into(), message: "m".into() }.http_status(),
            502
        );
    }

    #[test]
    fn code_mapping() {
        assert_eq!(ServiceError::NotFound("x".into()).code(), "NOT_FOUND");
    }
}
