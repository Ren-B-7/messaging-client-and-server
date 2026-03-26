use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Standard error response structure
#[derive(Error, Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub status: String,
    pub code: String,
    pub message: String,
}

impl ErrorResponse {
    pub fn new(code: &str, message: &str) -> Self {
        Self {
            status: "error".to_string(),
            code: code.to_string(),
            message: message.to_string(),
        }
    }
}

impl fmt::Display for ErrorResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "code={}, message={}", self.code, self.message)
    }
}
