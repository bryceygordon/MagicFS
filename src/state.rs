// src/state.rs

use std::sync::{Arc, RwLock};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use crate::error::MagicError;

// Type alias for embedding results to keep signatures clean
type EmbeddingResult = std::result::Result<Vec<f32>, MagicError>;

/// Request sent to the Embedding Actor
pub struct EmbeddingRequest {
    pub content: String,
    pub respond_to: oneshot::Sender<EmbeddingResult>,
}

/// Global shared state accessible by all organs
pub struct GlobalState {
    /// Maps query strings to dynamic inode numbers
    pub active_searches: Arc<DashMap<String, u64>>,

    /// Maps dynamic inode numbers to search results
    pub search_results: Arc<DashMap<u64, Vec<SearchResult>>>,

    /// Database connection (created lazily)
    pub db_connection: Arc<std::sync::Mutex<Option<rusqlite::Connection>>>,

    /// Channel to the dedicated Embedding Actor thread (replaces the Mutex<Model>)
    /// Wrapped in RwLock<Option> to allow lazy initialization
    pub embedding_tx: Arc<RwLock<Option<mpsc::Sender<EmbeddingRequest>>>>,

    /// Queue of file paths waiting for indexing
    pub files_to_index: Arc<std::sync::Mutex<Vec<String>>>,
}

/// Result of a semantic search operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub file_id: u64,
    pub abs_path: String,
    pub score: f32,
    pub filename: String,
}

/// Shared state wrapper for easy cloning and sharing
pub type SharedState = Arc<RwLock<GlobalState>>;

impl Default for GlobalState {
    fn default() -> Self {
        Self {
            active_searches: Arc::new(DashMap::new()),
            search_results: Arc::new(DashMap::new()),
            db_connection: Arc::new(std::sync::Mutex::new(None)),
            embedding_tx: Arc::new(RwLock::new(None)),
            files_to_index: Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }
}

impl GlobalState {
    /// Create new empty global state
    pub fn new() -> Self {
        Self::default()
    }
}
