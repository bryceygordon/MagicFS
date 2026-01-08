// FILE: src/state.rs

use std::sync::{Arc, RwLock, Mutex, Condvar};
use std::sync::atomic::{AtomicUsize, AtomicBool, Ordering};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use crate::error::MagicError;
use crate::core::inode_store::InodeStore;
use std::collections::HashMap;
use std::time::SystemTime; // Added import
use std::sync::atomic::AtomicU8;

/// System state enumeration for War Mode management
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemState {
    /// Initial boot phase, database setup
    Booting = 0,
    /// Initial bulk indexing in progress (War Mode active)
    Indexing = 1,
    /// Steady-state monitoring (Peace Mode active)
    Monitoring = 2,
}

impl SystemState {
    pub fn from_u8(value: u8) -> Self {
        match value {
            0 => SystemState::Booting,
            1 => SystemState::Indexing,
            _ => SystemState::Monitoring,
        }
    }

    pub fn as_u8(&self) -> u8 {
        *self as u8
    }

    /// Returns true if the system is in War Mode (Indexing)
    pub fn is_war_mode(&self) -> bool {
        matches!(self, SystemState::Indexing)
    }

    /// Returns true if the system is in Peace Mode (Monitoring)
    pub fn is_peace_mode(&self) -> bool {
        matches!(self, SystemState::Monitoring)
    }

    /// Human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            SystemState::Booting => "System booting up",
            SystemState::Indexing => "War Mode: Bulk indexing (max performance)",
            SystemState::Monitoring => "Peace Mode: Steady-state monitoring",
        }
    }
}

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

    /// System state for War Mode management (atomic)
    pub system_state: Arc<AtomicU8>,
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
            system_state: Arc::new(AtomicU8::new(SystemState::Booting.as_u8())),
        }
    }
}

impl GlobalState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the current system state
    pub fn get_system_state(&self) -> SystemState {
        SystemState::from_u8(self.system_state.load(Ordering::Relaxed))
    }

    /// Set the system state
    pub fn set_system_state(&self, state: SystemState) {
        self.system_state.store(state.as_u8(), Ordering::Relaxed);
    }

    /// Check if system is in War Mode
    pub fn is_war_mode(&self) -> bool {
        self.get_system_state().is_war_mode()
    }

    /// Check if system is in Peace Mode
    pub fn is_peace_mode(&self) -> bool {
        self.get_system_state().is_peace_mode()
    }
}
