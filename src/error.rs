//! Error types for MagicFS

use thiserror::Error;

#[derive(Error, Debug)]
pub enum MagicError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("FUSE error: {0}")]
    Fuse(String),

    #[error("Embedding error: {0}")]
    Embedding(String),

    #[error("Invalid path: {0}")]
    InvalidPath(String),

    #[error("State error: {0}")]
    State(String),

    #[error("Other error: {0}")]
    Other(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, MagicError>;