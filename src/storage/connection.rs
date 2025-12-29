//! Database connection management
//!
//! Provides lazy initialization and connection management for the database.
//! Works with GlobalState's db_connection field.
//! Uses Repository for schema initialization.

use std::sync::Arc;
use rusqlite::Connection;
use crate::SharedState;
use crate::storage::Repository;

/// Register sqlite-vec extension with SQLite
fn register_sqlite_vec_extension() -> crate::error::Result<()> {
    unsafe {
        let result = rusqlite::ffi::sqlite3_auto_extension(Some(
            std::mem::transmute(sqlite_vec::sqlite3_vec_init as *const ())
        ));

        if result != rusqlite::ffi::SQLITE_OK {
            let err = rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(result),
                Some("Failed to register sqlite-vec extension".to_string())
            );
            return Err(crate::error::MagicError::Database(err));
        }
        tracing::info!("Successfully registered sqlite-vec extension");
    }
    Ok(())
}

/// Initialize the database connection in GlobalState
pub fn init_connection(state: &SharedState, db_path: &str) -> crate::error::Result<()> {
    register_sqlite_vec_extension()?;

    let db_dir = std::path::Path::new(db_path).parent()
        .ok_or_else(|| crate::error::MagicError::InvalidPath("Invalid database path".into()))?;

    std::fs::create_dir_all(db_dir)
        .map_err(crate::error::MagicError::Io)?;

    let conn = Connection::open(db_path)
        .map_err(crate::error::MagicError::Database)?;

    // Performance Pragma
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;

    // Use Repository to init schema
    // We create a temporary Repository just for this init step
    let repo = Repository::new(&conn);
    repo.initialize()?;

    // Store in global state
    let mut guard = state.write().map_err(|_| crate::error::MagicError::State("Poisoned lock".into()))?;
    guard.db_connection = Arc::new(std::sync::Mutex::new(Some(conn)));
    
    Ok(())
}

pub fn get_connection(state: &SharedState) -> crate::error::Result<Arc<std::sync::Mutex<Option<Connection>>>> {
    let guard = state.read().map_err(|_| crate::error::MagicError::State("Poisoned lock".into()))?;
    Ok(guard.db_connection.clone())
}
