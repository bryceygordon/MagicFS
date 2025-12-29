//! Vector Index operations for sqlite-vec
//!
//! Handles embedding storage and retrieval:
//! - Insert new embeddings (Chunks)
//! - Delete embeddings (Clean up before re-indexing)

use crate::SharedState;
use crate::error::Result;
use rusqlite::params;
use bytemuck;

/// Insert a new embedding chunk into the vector index
/// Note: This does NOT delete existing embeddings. Call delete_embeddings_for_file first!
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

    let embedding_bytes: Vec<u8> = bytemuck::cast_slice(embedding).to_vec();

    // Insert the new chunk. We rely on rowid being auto-generated.
    // file_id is just a metadata column now.
    conn_ref.execute(
        "INSERT INTO vec_index (file_id, embedding) VALUES (?1, ?2)",
        params![file_id, embedding_bytes]
    )?;

    Ok(())
}

/// Delete ALL embeddings (chunks) for a specific file
pub fn delete_embeddings_for_file(state: &SharedState, file_id: u64) -> Result<bool> {
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

    tracing::debug!("Deleted chunks for file_id: {} (chunks affected: {})", file_id, rows_affected);
    Ok(rows_affected > 0)
}

/// Check if any embedding exists for a file_id
pub fn embedding_exists(state: &SharedState, file_id: u64) -> Result<bool> {
    let conn_opt = crate::storage::connection::get_connection(state)?;
    let conn_guard = conn_opt.lock()
        .map_err(|_| crate::error::MagicError::State("Poisoned lock".into()))?;
    
    if let Some(conn) = conn_guard.as_ref() {
        let count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM vec_index WHERE file_id = ?1",
            params![file_id],
            |row| row.get(0)
        )?;
        Ok(count > 0)
    } else {
        Ok(false)
    }
}
