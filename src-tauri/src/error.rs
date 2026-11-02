//! Application error type returned by Tauri commands.

use serde::{Serialize, Serializer};
use specta::datatype::DataType;
use specta::{Type, Types};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("migration error: {0}")]
    Migration(#[from] rusqlite_migration::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("tauri error: {0}")]
    Tauri(#[from] tauri::Error),
    #[error("{0}")]
    Other(String),
}

pub type AppResult<T> = Result<T, AppError>;

// Errors cross the IPC boundary as plain strings.
impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl Type for AppError {
    fn definition(types: &mut Types) -> DataType {
        String::definition(types)
    }
}
