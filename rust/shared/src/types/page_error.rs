use std::fmt;
use std::path::PathBuf;

use hyper;
use thiserror::Error;

/// Errors that can occur when delivering static pages or building HTTP responses.
#[derive(Error, Debug)]
pub enum PageError {
    /// Path contained invalid UTF-8 and could not be processed.
    InvalidUtf8(PathBuf),
    /// The `HOME` environment variable was not set; tilde expansion failed.
    HomeMissing,
    /// The file at the given path could not be read from disk.
    ReadFailed(PathBuf, #[source] std::io::Error),
    /// `hyper`'s response builder returned an error.
    ResponseBuildFailed(#[source] hyper::http::Error),
}

impl PageError {
    pub fn to_code(&self) -> &'static str {
        match self {
            Self::InvalidUtf8(_) => "INVALID_UTF8_PATH",
            Self::HomeMissing => "HOME_NOT_SET",
            Self::ReadFailed(_, _) => "FILE_READ_FAILED",
            Self::ResponseBuildFailed(_) => "RESPONSE_BUILD_FAILED",
        }
    }

    pub fn to_message(&self) -> String {
        match self {
            Self::InvalidUtf8(path) => format!("Path contains invalid UTF-8: {}", path.display()),
            Self::HomeMissing => "HOME environment variable is not set".to_string(),
            Self::ReadFailed(path, _) => format!("Failed to read static file: {}", path.display()),
            Self::ResponseBuildFailed(e) => format!("Failed to build HTTP response: {}", e),
        }
    }
}

impl fmt::Display for PageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "code={}, message={}", self.to_code(), self.to_message())
    }
}
