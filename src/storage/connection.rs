//! Database connection management
//!
//! Provides lazy initialization and connection management for the database.
//! Works with GlobalState's db_connection field.

use std::sync::{Arc, RwLock};
use rusqlite::Connection;
use crate::{GlobalState, SharedState};

/// Initialize the database connection in GlobalState
///
/// This function is called once at startup to create the database and establish
/// the connection. Future operations will use the initialized connection.
pub fn init_connection(state: &SharedState, db_path: &str) -> crate::error::Result<()> {
    let db_dir = std::path::Path::new(db_path).parent()
        .ok_or_else(|| crate::error::MagicError::InvalidPath("Invalid database path".into()))?;

    std::fs::create_dir_all(db_dir)
        .map_err(crate::error::MagicError::Io)?;

    // Create connection with WAL mode
    let conn = Connection::open(db_path)
        .map_err(crate::error::MagicError::Database)?;

    // Enable WAL mode for better concurrent access
    conn.pragma_update(None, "journal_mode", WAL)?;

    // Enable foreign key constraints
    conn.pragma_update(None, "foreign_keys", ON)?;

    // Allow extension loading (needed for sqlite-vec)
    conn.pragma_update(None, "enable_load_extension", ON)?;

    // Optimize for performance
    conn.pragma_update(None, "synchronous", NORMAL)?;

    // Check if we're creating a new database
    let initialized_db = !std::path::Path::new(db_path).exists()
        || conn.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='file_registry'", [], |row| row.get::<_, i32>(0)).unwrap_or(0) == 0;

    // Initialize tables if needed
    if initialized_db {
        // Try to load sqlite-vec extension first
        if let Err(e) = conn.execute_batch("SELECT load_extension('sqlite-vec');") {
            tracing::warn!("Failed to load sqlite-vec extension: {}", e);
        }

        conn.execute_batch(r#"
            CREATE TABLE IF NOT EXISTS file_registry (
                file_id INTEGER PRIMARY KEY AUTOINCREMENT,
                abs_path TEXT NOT NULL UNIQUE,
                inode INTEGER NOT NULL UNIQUE,
                mtime INTEGER NOT NULL,
                size INTEGER NOT NULL DEFAULT 0,
                is_dir INTEGER NOT NULL DEFAULT 0,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS system_config (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );
        "#)?;

        // Try to create vec_index table
        match conn.execute_batch(r#"
            CREATE VIRTUAL TABLE IF NOT EXISTS vec_index USING vec0(
                file_id INTEGER PRIMARY KEY,
                embedding FLOAT[384] NOT NULL
            )
        "#) {
            Ok(_) => tracing::info!("Created vec_index table successfully"),
            Err(e) => tracing::warn!("Failed to create vec_index table: {}", e),
        }

        tracing::info!("Initialized new database with all tables");
    } else {
        tracing::info!("Loaded existing database");

        // Also try to create vec_index table for existing databases that might not have it
        match conn.execute_batch(r#"
            CREATE VIRTUAL TABLE IF NOT EXISTS vec_index USING vec0(
                file_id INTEGER PRIMARY KEY,
                embedding FLOAT[384] NOT NULL
            )
        "#) {
            Ok(_) => tracing::info!("Created/verified vec_index table for existing database"),
            Err(e) => tracing::warn!("vec_index table not available (sqlite-vec extension required): {}", e),
        }
    }

    // Store connection in state (wrap in Arc<Mutex<Option<Connection>>>)
    let conn_arc = {
        let state_guard = state.read()
            .map_err(|_| crate::error::MagicError::State("Poisoned lock".into()))?;
        Arc::clone(&state_guard.db_connection)
    };
    // Lock the inner mutex and set the option
    {
        let mut conn_guard = conn_arc.lock()
            .map_err(|_| crate::error::MagicError::State("Poisoned lock".into()))?;
        *conn_guard = Some(conn);
    }

    Ok(())
}

/// Get a reference to the database connection Arc<Mutex<Option<Connection>>>
///
/// Returns the connection from GlobalState for use in other modules.
pub fn get_connection(state: &SharedState) -> crate::error::Result<Arc<std::sync::Mutex<Option<Connection>>>> {
    let state_guard = state.read()
        .map_err(|_| crate::error::MagicError::State("Poisoned lock".into()))?;

    Ok(state_guard.db_connection.clone())
}

// SQL pragma constants
const WAL: &str = "WAL";
const ON: &str = "ON";
const NORMAL: &str = "NORMAL";