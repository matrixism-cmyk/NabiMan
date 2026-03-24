use serde::Serialize;

#[derive(Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    pub data: Option<T>,
    pub message: String,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            message: "OK".to_string(),
        }
    }

    pub fn error(msg: &str) -> Self {
        Self {
            success: false,
            data: None,
            message: msg.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_response_ok() {
        let resp = ApiResponse::ok("hello");
        assert!(resp.success);
        assert_eq!(resp.data.unwrap(), "hello");
        assert_eq!(resp.message, "OK");
    }

    #[test]
    fn test_api_response_error() {
        let resp = ApiResponse::<String>::error("something went wrong");
        assert!(!resp.success);
        assert!(resp.data.is_none());
        assert_eq!(resp.message, "something went wrong");
    }

    #[test]
    fn test_api_response_serializes() {
        let resp = ApiResponse::ok(42);
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"success\":true"));
        assert!(json.contains("\"data\":42"));
    }
}
