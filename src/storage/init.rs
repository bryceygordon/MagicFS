//! Database initialization module
//!
//! Handles creating the SQLite database at `.magicfs/index.db` with:
//! - WAL mode for concurrent access
//! - Three core tables: file_registry, vec_index, system_config

use std::path::Path;
use rusqlite::Connection;
use crate::error::{Result, MagicError};

/// Initialize SQLite database at the specified path
///
/// Creates the database file, enables WAL mode for better concurrency,
/// and ensures the .magicfs directory exists.
pub fn initialize_database(db_path: &str) -> Result<Connection> {
    let db_dir = Path::new(db_path).parent()
        .ok_or_else(|| MagicError::InvalidPath("Invalid database path".into()))?;

    std::fs::create_dir_all(db_dir)
        .map_err(MagicError::Io)?;

    let conn = Connection::open(db_path)
        .map_err(MagicError::Database)?;

    // Enable WAL mode for better concurrent access
    conn.pragma_journal_mode(WAL)?;

    // Enable foreign key constraints
    conn.pragma_foreign_keys(ON)?;

    // Optimize for performance
    conn.pragma_synchronous(NORMAL)?;

    tracing::info!("Database initialized at: {}", db_path);

    Ok(conn)
}

/// Create all required tables in the database
pub fn create_tables(conn: &Connection) -> Result<()> {
    create_file_registry_table(conn)?;
    create_vec_index_table(conn)?;
    create_system_config_table(conn)?;
    Ok(())
}

fn create_file_registry_table(conn: &Connection) -> Result<()> {
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
    "#)?;
    tracing::debug!("Created file_registry table");
    Ok(())
}

fn create_vec_index_table(conn: &Connection) -> Result<()> {
    conn.execute_batch(r#"
        -- Load sqlite-vec extension for vector operations
        SELECT load_extension('sqlite-vec');

        CREATE VIRTUAL TABLE IF NOT EXISTS vec_index USING vec0(
            file_id INTEGER,
            embedding float[384]
        );
    "#)?;
    tracing::debug!("Created vec_index virtual table with sqlite-vec");
    Ok(())
}

fn create_system_config_table(conn: &Connection) -> Result<()> {
    conn.execute_batch(r#"
        CREATE TABLE IF NOT EXISTS system_config (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
        );
    "#)?;
    tracing::debug!("Created system_config table");
    Ok(())
}

// SQL pragma constants
const WAL: &str = "WAL";
const ON: &str = "ON";
const NORMAL: &str = "NORMAL";