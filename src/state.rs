//! Shared state management - The Source of Truth
//!
//! Contains GlobalState and all data structures accessible across all Organs

use std::sync::{Arc, RwLock};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};

/// Global shared state accessible by all organs
pub struct GlobalState {
    /// Maps query strings to dynamic inode numbers
    pub active_searches: Arc<DashMap<String, u64>>,

    /// Maps dynamic inode numbers to search results
    pub search_results: Arc<DashMap<u64, Vec<SearchResult>>>,

    /// Database connection (created lazily)
    pub db_connection: Arc<std::sync::Mutex<Option<rusqlite::Connection>>>,

    /// Embedding model for Oracle
    pub embedding_model: Arc<std::sync::Mutex<Option<fastembed::TextEmbedding>>>,

    /// Queue of file paths waiting for indexing (added by Librarian, processed by Oracle)
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
            embedding_model: Arc::new(std::sync::Mutex::new(None)),
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