use serde::Serialize;

#[derive(Serialize)]
pub struct CliResponse<T: Serialize> {
    pub status: String,
    pub data: T,
}

impl<T: Serialize> CliResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            status: "ok".into(),
            data,
        }
    }
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub status: String,
    pub message: String,
}

impl ErrorResponse {
    pub fn new(message: &str) -> Self {
        Self {
            status: "error".into(),
            message: message.into(),
        }
    }
}
