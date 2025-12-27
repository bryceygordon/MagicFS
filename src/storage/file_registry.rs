//! File Registry CRUD operations
//!
//! Handles all operations on the file_registry table:
//! - Registering new files with their paths, inodes, and metadata
//! - Looking up files by path or inode
//! - Listing all registered files
//! - Updating file metadata (mtime, size)
//! - Deleting files from the registry

use std::sync::Arc;
use rusqlite::params;
use crate::{SharedState, error::Result};

/// Register a new file in the file registry
pub fn register_file(
    state: &SharedState,
    abs_path: &str,
    inode: u64,
    mtime: u64,
    size: u64,
    is_dir: bool,
) -> Result<u64> {
    let conn_opt = crate::storage::connection::get_connection(state)?;
    let conn_guard = conn_opt.lock()
        .map_err(|_| crate::error::MagicError::State("Poisoned lock".into()))?;
    let conn_ref = conn_guard.as_ref()
        .ok_or_else(|| crate::error::MagicError::Other(anyhow::anyhow!("Database not initialized")))?;

    let mut stmt = conn_ref.prepare(
        "INSERT INTO file_registry (abs_path, inode, mtime, size, is_dir)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(abs_path) DO UPDATE SET
             inode = excluded.inode,
             mtime = excluded.mtime,
             size = excluded.size,
             is_dir = excluded.is_dir,
             updated_at = CURRENT_TIMESTAMP
         RETURNING file_id"
    )?;

    let file_id = stmt.query_row(
        params![abs_path, inode, mtime, size, if is_dir { 1 } else { 0 }],
        |row| row.get::<_, u64>(0)
    )?;

    tracing::debug!("Registered file: {} (inode: {}, file_id: {})", abs_path, inode, file_id);
    Ok(file_id)
}

/// Get file info by absolute path
pub fn get_file_by_path(state: &SharedState, abs_path: &str) -> Result<Option<FileRecord>> {
    let conn_opt = crate::storage::connection::get_connection(state)?;
    let conn_guard = conn_opt.lock()
        .map_err(|_| crate::error::MagicError::State("Poisoned lock".into()))?;
    let Some(conn) = conn_guard.as_ref() else {
        return Ok(None);
    };

    let mut stmt = conn.prepare(
        "SELECT file_id, abs_path, inode, mtime, size, is_dir, created_at, updated_at
         FROM file_registry WHERE abs_path = ?1"
    )?;

    let result = stmt.query_row(params![abs_path], |row| {
        Ok(FileRecord {
            file_id: row.get(0)?,
            abs_path: row.get(1)?,
            inode: row.get(2)?,
            mtime: row.get(3)?,
            size: row.get(4)?,
            is_dir: row.get::<_, i32>(5)? != 0,
            created_at: row.get(6)?,
            updated_at: row.get(7)?,
        })
    });

    match result {
        Ok(record) => Ok(Some(record)),
        Err(e) if e.to_string().contains("NOT FOUND") || e.to_string().contains("No rows") => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Get file info by inode
pub fn get_file_by_inode(state: &SharedState, inode: u64) -> Result<Option<FileRecord>> {
    let conn_opt = crate::storage::connection::get_connection(state)?;
    let conn_guard = conn_opt.lock()
        .map_err(|_| crate::error::MagicError::State("Poisoned lock".into()))?;
    let Some(conn) = conn_guard.as_ref() else {
        return Ok(None);
    };

    let mut stmt = conn.prepare(
        "SELECT file_id, abs_path, inode, mtime, size, is_dir, created_at, updated_at
         FROM file_registry WHERE inode = ?1"
    )?;

    let result = stmt.query_row(params![inode], |row| {
        Ok(FileRecord {
            file_id: row.get(0)?,
            abs_path: row.get(1)?,
            inode: row.get(2)?,
            mtime: row.get(3)?,
            size: row.get(4)?,
            is_dir: row.get::<_, i32>(5)? != 0,
            created_at: row.get(6)?,
            updated_at: row.get(7)?,
        })
    });

    match result {
        Ok(record) => Ok(Some(record)),
        Err(e) if e.to_string().contains("NOT FOUND") || e.to_string().contains("No rows") => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// List all registered files
pub fn list_files(state: &SharedState) -> Result<Vec<FileRecord>> {
    let conn_opt = crate::storage::connection::get_connection(state)?;
    let conn_guard = conn_opt.lock()
        .map_err(|_| crate::error::MagicError::State("Poisoned lock".into()))?;
    let Some(conn) = conn_guard.as_ref() else {
        return Ok(Vec::new());
    };

    let mut stmt = conn.prepare(
        "SELECT file_id, abs_path, inode, mtime, size, is_dir, created_at, updated_at
         FROM file_registry
         ORDER BY abs_path"
    )?;

    let records_iter = stmt.query_map([], |row| {
        Ok(FileRecord {
            file_id: row.get(0)?,
            abs_path: row.get(1)?,
            inode: row.get(2)?,
            mtime: row.get(3)?,
            size: row.get(4)?,
            is_dir: row.get::<_, i32>(5)? != 0,
            created_at: row.get(6)?,
            updated_at: row.get(7)?,
        })
    })?;

    let mut records = Vec::new();
    for record in records_iter {
        records.push(record?);
    }

    Ok(records)
}

/// Update file modification time and size
pub fn update_file_mtime(
    state: &SharedState,
    abs_path: &str,
    mtime: u64,
    size: u64,
) -> Result<()> {
    let conn_opt = crate::storage::connection::get_connection(state)?;
    let conn_guard = conn_opt.lock()
        .map_err(|_| crate::error::MagicError::State("Poisoned lock".into()))?;
    if let Some(conn) = conn_guard.as_ref() {
        conn.execute(
            "UPDATE file_registry
             SET mtime = ?1, size = ?2, updated_at = CURRENT_TIMESTAMP
             WHERE abs_path = ?3",
            params![mtime, size, abs_path]
        )?;
    };

    tracing::debug!("Updated mtime for file: {}", abs_path);
    Ok(())
}

/// Delete a file from the registry
pub fn delete_file(state: &SharedState, abs_path: &str) -> Result<bool> {
    let conn_opt = crate::storage::connection::get_connection(state)?;
    let conn_guard = conn_opt.lock()
        .map_err(|_| crate::error::MagicError::State("Poisoned lock".into()))?;
    let rows_affected = if let Some(conn) = conn_guard.as_ref() {
        conn.execute(
            "DELETE FROM file_registry WHERE abs_path = ?1",
            params![abs_path]
        )?
    } else {
        0
    };

    tracing::debug!("Deleted file from registry: {} (rows affected: {})", abs_path, rows_affected);
    Ok(rows_affected > 0)
}

/// Get file count
pub fn get_file_count(state: &SharedState) -> Result<u64> {
    let conn_opt = crate::storage::connection::get_connection(state)?;
    let conn_guard = conn_opt.lock()
        .map_err(|_| crate::error::MagicError::State("Poisoned lock".into()))?;
    let count = if let Some(conn) = conn_guard.as_ref() {
        conn.query_row(
            "SELECT COUNT(*) FROM file_registry",
            [],
            |row| row.get::<_, u64>(0)
        )?
    } else {
        0
    };

    Ok(count)
}

/// Record representing a registered file
#[derive(Debug, Clone)]
pub struct FileRecord {
    pub file_id: u64,
    pub abs_path: String,
    pub inode: u64,
    pub mtime: u64,
    pub size: u64,
    pub is_dir: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl std::fmt::Display for FileRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} (inode: {}, size: {}, is_dir: {})",
            self.abs_path, self.inode, self.size, self.is_dir)
    }
}