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
