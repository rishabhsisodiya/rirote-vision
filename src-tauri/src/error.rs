//! One error type for every command, so the interface can tell failures apart
//! by `code` and show `message` as written.

use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    InvalidInput(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    NotAProject(String),
    #[error("{0}")]
    AlreadyExists(String),
    /// The engine understood the request and declined it, with a reason.
    #[error("{0}")]
    Refused(String),
    #[error("{0}")]
    ToolMissing(String),
    #[error("{0}")]
    ToolFailed(String),
    #[error("{0}")]
    Timeout(String),
    #[error("{0}")]
    Io(String),
}

impl AppError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidInput(_) => "invalid_input",
            Self::NotFound(_) => "not_found",
            Self::NotAProject(_) => "not_a_project",
            Self::AlreadyExists(_) => "already_exists",
            Self::Refused(_) => "refused",
            Self::ToolMissing(_) => "tool_missing",
            Self::ToolFailed(_) => "tool_failed",
            Self::Timeout(_) => "timeout",
            Self::Io(_) => "io",
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("AppError", 2)?;
        state.serialize_field("code", self.code())?;
        state.serialize_field("message", &self.to_string())?;
        state.end()
    }
}

pub type AppResult<T> = Result<T, AppError>;
