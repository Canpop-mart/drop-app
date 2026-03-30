use std::fmt::Display;

use serde_with::SerializeDisplay;

#[derive(Debug, SerializeDisplay)]

pub enum BackupError {
    InvalidSystem,

    NotFound,

    ParseError,

    IoError(String),

    SerializationError(String),
}

impl Display for BackupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            BackupError::InvalidSystem => "Attempted to generate path for invalid system".to_string(),

            BackupError::NotFound => "Could not generate or find path".to_string(),

            BackupError::ParseError => "Failed to parse path".to_string(),

            BackupError::IoError(e) => format!("IO error: {}", e),

            BackupError::SerializationError(e) => format!("Serialization error: {}", e),
        };

        write!(f, "{}", s)
    }
}

impl From<std::io::Error> for BackupError {
    fn from(e: std::io::Error) -> Self {
        BackupError::IoError(e.to_string())
    }
}

impl From<serde_json::Error> for BackupError {
    fn from(e: serde_json::Error) -> Self {
        BackupError::SerializationError(e.to_string())
    }
}
