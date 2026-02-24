use serde::Serialize;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Network error: {0}")]
    Network(String, Option<u16>),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Tauri error: {0}")]
    Tauri(#[from] tauri::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

impl From<reqwest::Error> for AppError {
    fn from(err: reqwest::Error) -> Self {
        let status = err.status().map(|s| s.as_u16());
        AppError::Network(err.to_string(), status)
    }
}

// Implement Serialize so it can be used as a return value for Tauri commands
impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.to_string().as_str())
    }
}

// Implement alias for Result to simplify usage
pub type AppResult<T> = Result<T, AppError>;
