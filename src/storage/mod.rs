//! Storage module - SQLite database and file registry
//!
//! Handles all persistent storage operations for MagicFS:
//! - Database initialization with WAL mode
//! - file_registry table (maps physical files to inodes)
//! - vec_index table (vector embeddings via sqlite-vec)
//! - system_config table (key-value metadata)

pub mod connection;
pub mod file_registry;
pub mod text_extraction;
pub mod vec_index;

pub use connection::init_connection;
pub use file_registry::{register_file, get_file_by_path, get_file_by_inode, list_files, update_file_mtime, delete_file, get_file_count, FileRecord};
pub use text_extraction::extract_text_from_file;
// FIX: Updated exports to match Phase 6 Chunking API
// - Removed update_embedding (we now delete + insert)
// - Renamed delete_embedding -> delete_embeddings_for_file
pub use vec_index::{insert_embedding, delete_embeddings_for_file};
