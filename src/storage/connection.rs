//! Database connection management
//!
//! Provides lazy initialization and connection management for the database.
//! Works with GlobalState's db_connection field.

use std::sync::Arc;
use rusqlite::Connection;
use crate::SharedState;

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

    conn.pragma_update(None, "journal_mode", WAL)?;
    conn.pragma_update(None, "foreign_keys", ON)?;
    conn.pragma_update(None, "synchronous", NORMAL)?;

    let initialized_db = !std::path::Path::new(db_path).exists()
        || conn.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='file_registry'", [], |row| row.get::<_, i32>(0)).unwrap_or(0) == 0;

    if initialized_db {
        conn.execute_batch(r#"
            CREATE TABLE IF NOT EXISTS file_registry (
                file_id INTEGER PRIMARY KEY AUTOINCREMENT,
                abs_path TEXT NOT NULL UNIQUE,
                inode INTEGER NOT NULL,
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

        // Phase 6 Schema: Support multiple chunks per file
        conn.execute("DROP TABLE IF EXISTS vec_index", [])?;
        match conn.execute_batch(r#"
            CREATE VIRTUAL TABLE IF NOT EXISTS vec_index USING vec0(
                file_id INTEGER,          -- Link to file_registry
                embedding float[384]      -- The vector
            )
        "#) {
            Ok(_) => tracing::info!("Created vec_index table (Chunking enabled)"),
            Err(e) => tracing::warn!("Failed to create vec_index table: {}", e),
        }

        tracing::info!("Initialized new database with Chunking schema");
    } else {
        tracing::info!("Loaded existing database");

        // Migration: Ensure vec_index supports chunking (has file_id column)
        // For development simplicity in Phase 6, we recreate it if needed.
        // In prod, check PRAGMA table_info('vec_index').
        tracing::info!("Ensuring vec_index supports chunking...");
        let _ = conn.execute("DROP TABLE IF EXISTS vec_index", []);
         
        match conn.execute_batch(r#"
            CREATE VIRTUAL TABLE IF NOT EXISTS vec_index USING vec0(
                file_id INTEGER,
                embedding float[384]
            )
        "#) {
            Ok(_) => tracing::info!("Recreated vec_index table for Chunking"),
            Err(e) => tracing::warn!("Failed to recreate vec_index: {}", e),
        }
    }

    let conn_arc = {
        let state_guard = state.read()
            .map_err(|_| crate::error::MagicError::State("Poisoned lock".into()))?;
        Arc::clone(&state_guard.db_connection)
    };
    {
        let mut conn_guard = conn_arc.lock()
            .map_err(|_| crate::error::MagicError::State("Poisoned lock".into()))?;
        *conn_guard = Some(conn);
    }

    Ok(())
}

pub fn get_connection(state: &SharedState) -> crate::error::Result<Arc<std::sync::Mutex<Option<Connection>>>> {
    let state_guard = state.read()
        .map_err(|_| crate::error::MagicError::State("Poisoned lock".into()))?;
    Ok(state_guard.db_connection.clone())
}

const WAL: &str = "WAL";
const ON: &str = "ON";
const NORMAL: &str = "NORMAL";
