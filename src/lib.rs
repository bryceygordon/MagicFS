// FILE: src/lib.rs
pub mod core;
pub mod engine; // Added
pub mod hollow_drive;
pub mod oracle;
pub mod librarian;
pub mod state;
pub mod error;
pub mod storage;

pub use state::{GlobalState, SharedState, SearchResult};
pub use error::{Result, MagicError};
pub use storage::{init_connection, Repository};
