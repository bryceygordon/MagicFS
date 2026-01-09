// Create a test database to demonstrate missing indices
// Run with: rustc create_test_db.rs -o create_test_db && ./create_test_db

use rusqlite::{Connection, Result};

fn main() -> Result<()> {
    let db_path = "/tmp/.magicfs_nomic/index.db";
    let mut conn = Connection::open(db_path)?;

    // Enable foreign keys
    conn.execute("PRAGMA foreign_keys = ON", [])?;

    // Create schema EXACTLY as repository.rs does (without indices)
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

    conn.execute_batch(r#"
        CREATE VIRTUAL TABLE IF NOT EXISTS vec_index USING vec0(
            file_id INTEGER,
            embedding float[768] distance_metric=cosine
        )
    "#)?;

    conn.execute_batch(r#"
        CREATE TABLE IF NOT EXISTS tags (
            tag_id INTEGER PRIMARY KEY AUTOINCREMENT,
            parent_tag_id INTEGER,
            name TEXT NOT NULL,
            color TEXT,
            icon TEXT,
            UNIQUE(parent_tag_id, name),
            FOREIGN KEY(parent_tag_id) REFERENCES tags(tag_id) ON DELETE CASCADE
        );
    "#)?;

    conn.execute_batch(r#"
        CREATE TABLE IF NOT EXISTS file_tags (
            file_id INTEGER NOT NULL,
            tag_id INTEGER NOT NULL,
            display_name TEXT,
            added_at INTEGER DEFAULT (unixepoch()),
            PRIMARY KEY (file_id, tag_id),
            FOREIGN KEY (file_id) REFERENCES file_registry(file_id) ON DELETE CASCADE,
            FOREIGN KEY (tag_id) REFERENCES tags(tag_id) ON DELETE CASCADE
        );
    "#)?;

    println!("✅ Test database created at: {}", db_path);
    println!("Schema created WITHOUT the Phase 16 indices.");
    Ok(())
}