//! Vector Index operations for sqlite-vec
//!
//! Handles embedding storage and retrieval:
//! - Insert new embeddings
//! - Update existing embeddings
//! - Delete embeddings
//!

use crate::SharedState;
use crate::error::Result;
use rusqlite::params;
use bytemuck;

/// Insert a new embedding into the vector index
pub fn insert_embedding(
    state: &SharedState,
    file_id: u64,
    embedding: &[f32],
) -> Result<()> {
    let conn_opt = crate::storage::connection::get_connection(state)?;
    let conn_guard = conn_opt.lock()
        .map_err(|_| crate::error::MagicError::State("Poisoned lock".into()))?;
    let conn_ref = conn_guard.as_ref()
        .ok_or_else(|| crate::error::MagicError::Other(anyhow::anyhow!("Database not initialized")))?;

    // Convert embedding to bytes for sqlite-vec
    let embedding_bytes: Vec<u8> = bytemuck::cast_slice(embedding).to_vec();

    conn_ref.execute(
        "INSERT INTO vec_index (file_id, embedding) VALUES (?1, ?2)",
        params![file_id, embedding_bytes]
    )?;

    tracing::debug!("Inserted embedding for file_id: {}", file_id);
    Ok(())
}

/// Update an existing embedding in the vector index
pub fn update_embedding(
    state: &SharedState,
    file_id: u64,
    embedding: &[f32],
) -> Result<()> {
    let conn_opt = crate::storage::connection::get_connection(state)?;
    let conn_guard = conn_opt.lock()
        .map_err(|_| crate::error::MagicError::State("Poisoned lock".into()))?;
    let conn_ref = conn_guard.as_ref()
        .ok_or_else(|| crate::error::MagicError::Other(anyhow::anyhow!("Database not initialized")))?;

    // Convert embedding to bytes for sqlite-vec
    let embedding_bytes: Vec<u8> = bytemuck::cast_slice(embedding).to_vec();

    conn_ref.execute(
        "UPDATE vec_index SET embedding = ?1 WHERE file_id = ?2",
        params![embedding_bytes, file_id]
    )?;

    tracing::debug!("Updated embedding for file_id: {}", file_id);
    Ok(())
}

/// Delete an embedding from the vector index
pub fn delete_embedding(state: &SharedState, file_id: u64) -> Result<bool> {
    let conn_opt = crate::storage::connection::get_connection(state)?;
    let conn_guard = conn_opt.lock()
        .map_err(|_| crate::error::MagicError::State("Poisoned lock".into()))?;
    let rows_affected = if let Some(conn) = conn_guard.as_ref() {
        conn.execute(
            "DELETE FROM vec_index WHERE file_id = ?1",
            params![file_id]
        )?
    } else {
        0
    };

    tracing::debug!("Deleted embedding for file_id: {} (rows affected: {})", file_id, rows_affected);
    Ok(rows_affected > 0)
}

/// Check if an embedding exists for a file_id
pub fn embedding_exists(state: &SharedState, file_id: u64) -> Result<bool> {
    let conn_opt = crate::storage::connection::get_connection(state)?;
    let conn_guard = conn_opt.lock()
        .map_err(|_| crate::error::MagicError::State("Poisoned lock".into()))?;
    let exists = if let Some(conn) = conn_guard.as_ref() {
        let count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM vec_index WHERE file_id = ?1",
            params![file_id],
            |row| row.get(0)
        )?;
        count > 0
    } else {
        false
    };

    Ok(exists)
}

/// Get embedding count
pub fn get_embedding_count(state: &SharedState) -> Result<u64> {
    let conn_opt = crate::storage::connection::get_connection(state)?;
    let conn_guard = conn_opt.lock()
        .map_err(|_| crate::error::MagicError::State("Poisoned lock".into()))?;
    let count = if let Some(conn) = conn_guard.as_ref() {
        conn.query_row(
            "SELECT COUNT(*) FROM vec_index",
            [],
            |row| row.get::<_, u64>(0)
        )?
    } else {
        0
    };

    Ok(count)
}