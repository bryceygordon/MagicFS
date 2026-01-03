// FILE: src/state.rs

use std::sync::{Arc, RwLock, Mutex, Condvar};
use std::sync::atomic::{AtomicUsize, AtomicBool};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use crate::error::MagicError;
use crate::core::inode_store::InodeStore;
use std::collections::HashMap;
use std::time::SystemTime; // Added import

// Type alias for embedding results
type EmbeddingResult = std::result::Result<Vec<Vec<f32>>, MagicError>;

/// Request sent to the Embedding Actor
pub struct EmbeddingRequest {
    pub content: Vec<String>,
    pub is_query: bool,
    pub respond_to: oneshot::Sender<EmbeddingResult>,
}

/// A synchronization primitive for the "Smart Waiter"
pub struct SearchWaiter {
    pub finished: Mutex<bool>,
    pub cvar: Condvar,
}

impl SearchWaiter {
    pub fn new() -> Self {
        Self {
            finished: Mutex::new(false),
            cvar: Condvar::new(),
        }
    }
}

/// Global shared state accessible by all organs
pub struct GlobalState {
    /// The centralized authority on Inodes and Virtual Files
    pub inode_store: Arc<InodeStore>,

    /// Database connection (created lazily)
    pub db_connection: Arc<std::sync::Mutex<Option<rusqlite::Connection>>>,

    /// Channel to the dedicated Embedding Actor thread
    pub embedding_tx: Arc<RwLock<Option<mpsc::Sender<EmbeddingRequest>>>>,

    /// Queue of file paths waiting for indexing
    pub files_to_index: Arc<std::sync::Mutex<Vec<String>>>,

    /// Version counter for the index/cache state.
    pub index_version: Arc<AtomicUsize>,
    
    /// Manual Override Signal (Atomic Flag)
    pub refresh_signal: Arc<AtomicBool>,

    /// List of watched root directories (for Mirror Mode)
    pub watch_paths: Arc<std::sync::Mutex<Vec<String>>>,

    /// Registry of active waiters (Inode -> Waiter)
    pub search_waiters: Arc<Mutex<HashMap<u64, Arc<SearchWaiter>>>>,

    /// The Anchor: When did this filesystem start?
    /// Used to provide stable mtime for virtual directories.
    pub start_time: SystemTime,
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
            inode_store: Arc::new(InodeStore::new()),
            db_connection: Arc::new(std::sync::Mutex::new(None)),
            embedding_tx: Arc::new(RwLock::new(None)),
            files_to_index: Arc::new(std::sync::Mutex::new(Vec::new())),
            index_version: Arc::new(AtomicUsize::new(0)),
            refresh_signal: Arc::new(AtomicBool::new(false)),
            watch_paths: Arc::new(std::sync::Mutex::new(Vec::new())),
            search_waiters: Arc::new(Mutex::new(HashMap::new())),
            start_time: SystemTime::now(), // Anchor established on boot
        }
    }
}

impl GlobalState {
    pub fn new() -> Self {
        Self::default()
    }
}
