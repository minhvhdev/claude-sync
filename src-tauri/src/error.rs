use serde::Serialize;
use std::fmt;

#[derive(Debug, Serialize)]
pub struct AppError {
    pub kind: String,
    pub message: String,
}

impl AppError {
    pub fn auth(msg: impl Into<String>) -> Self {
        Self {
            kind: "AuthError".to_string(),
            message: msg.into(),
        }
    }

    pub fn io(msg: impl Into<String>) -> Self {
        Self {
            kind: "IoError".to_string(),
            message: msg.into(),
        }
    }

    pub fn git(msg: impl Into<String>) -> Self {
        Self {
            kind: "GitError".to_string(),
            message: msg.into(),
        }
    }

    pub fn network(msg: impl Into<String>) -> Self {
        Self {
            kind: "NetworkError".to_string(),
            message: msg.into(),
        }
    }

    pub fn system(msg: impl Into<String>) -> Self {
        Self {
            kind: "SystemError".to_string(),
            message: msg.into(),
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.kind, self.message)
    }
}

// Map standard errors to AppError
impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::io(err.to_string())
    }
}

impl From<git2::Error> for AppError {
    fn from(err: git2::Error) -> Self {
        AppError::git(err.message())
    }
}
