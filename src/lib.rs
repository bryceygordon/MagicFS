//! MagicFS: Semantic Virtual Filesystem
//!
//! A single-process binary implementing three isolated "Organs":
//! - Hollow Drive (FUSE loop - synchronous, never blocks >10ms)
//! - Oracle (async brain - handles vector search & embeddings)
//! - Librarian (background watcher - updates index)

pub mod hollow_drive;
pub mod oracle;
pub mod librarian;
pub mod state;
pub mod error;
pub mod storage;

pub use state::{GlobalState, SharedState, SearchResult};
pub use error::{Result, MagicError};
// Export the common storage functions
pub use storage::{
    init_connection, 
    register_file, 
    get_file_by_path, 
    get_file_by_inode, 
    list_files, 
    update_file_mtime, 
    delete_file, 
    get_file_count, 
    FileRecord
};
// Note: We don't strictly need to export vec_index functions here if Oracle uses them internally,
// but if we do, they must match the new names.
