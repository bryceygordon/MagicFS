// FILE: src/engine/indexer.rs
use crate::state::SharedState;
use crate::error::{Result, MagicError};
use crate::storage::Repository;
use crate::engine::request_embedding;
use std::sync::atomic::Ordering;
use std::time::Duration;

pub struct Indexer;

impl Indexer {
    /// Orchestrates the indexing of a single file:
    /// 1. Extract text (with retries)
    /// 2. Chunk text
    /// 3. Register file in DB
    /// 4. Generate embeddings
    /// 5. Insert embeddings
    pub async fn index_file(state: SharedState, file_path: String) -> Result<()> {
        tracing::info!("[Indexer] Processing: {}", file_path);
        
        // 1. Extraction (with retries for file locks AND partial writes)
        let text_content = Self::extract_with_retry(&file_path).await?;
        if text_content.trim().is_empty() {
            tracing::debug!("[Indexer] Skipping empty or binary file: {}", file_path);
            return Ok(());
        }

        // 2. Chunking
        let chunks = crate::storage::text_extraction::chunk_text(&text_content);
        if chunks.is_empty() { return Ok(()); }

        tracing::debug!("[Indexer] {} split into {} chunks", file_path, chunks.len());

        // 3. Register File & Clean Old Data
        let (file_id, _inode) = {
            let state_guard = state.read().map_err(|_| MagicError::State("Poisoned lock".into()))?;
            let conn_lock = state_guard.db_connection.lock().map_err(|_| MagicError::State("Poisoned lock".into()))?;
            let conn = conn_lock.as_ref().ok_or_else(|| MagicError::Other(anyhow::anyhow!("Database not initialized")))?;
            let repo = Repository::new(conn);

            // Mock inode logic (same as before)
            let inode = file_path.len() as u64 + 0x100000; 
            let metadata = std::fs::metadata(&file_path).map_err(MagicError::Io)?;
            let mtime = metadata.modified().map_err(MagicError::Io)?.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
            let size = metadata.len();
            let is_dir = metadata.is_dir();

            let fid = repo.register_file(
                &file_path, inode, mtime, size, is_dir
            )?;
            
            repo.delete_embeddings_for_file(fid)?;
            (fid, inode)
        };

        // 4. Generate & Insert Embeddings
        for chunk in chunks {
            // Note: This calls the Async Embedding Actor
            let embedding = request_embedding(&state, chunk).await?;
            
            let state_guard = state.read().map_err(|_| MagicError::State("Poisoned lock".into()))?;
            let conn_lock = state_guard.db_connection.lock().map_err(|_| MagicError::State("Poisoned lock".into()))?;
            let conn = conn_lock.as_ref().ok_or_else(|| MagicError::Other(anyhow::anyhow!("Database not initialized")))?;
            let repo = Repository::new(conn);

            if let Err(e) = repo.insert_embedding(file_id, &embedding) {
                tracing::warn!("[Indexer] Failed to insert chunk: {}", e);
            }
        }

        // 5. Invalidate Caches
        Self::invalidate_cache(state)?;
        tracing::info!("[Indexer] Indexed {} (ID: {})", file_path, file_id);
        Ok(())
    }

    pub async fn remove_file(state: SharedState, file_path: String) -> Result<()> {
        {
            let state_guard = state.read().map_err(|_| MagicError::State("Poisoned lock".into()))?;
            let conn_lock = state_guard.db_connection.lock().map_err(|_| MagicError::State("Poisoned lock".into()))?;
            let conn = conn_lock.as_ref().ok_or_else(|| MagicError::Other(anyhow::anyhow!("Database not initialized")))?;
            let repo = Repository::new(conn);

            if let Some(record) = repo.get_file_by_path(&file_path)? {
                repo.delete_embeddings_for_file(record.file_id)?;
                repo.delete_file(&file_path)?;
            }
        }
        
        Self::invalidate_cache(state)?;
        tracing::info!("[Indexer] Removed {}", file_path);
        Ok(())
    }

    async fn extract_with_retry(path: &str) -> Result<String> {
        let max_retries = 10; // Increased retries
        let mut last_error = None;

        for attempt in 1..=max_retries {
            let path_owned = path.to_string();
            
            // Check metadata size first
            // CRITICAL FIX: If size is 0, we treat it as a "Retry" condition, not a success.
            // Files are often created as 0 bytes before content is flushed.
            if let Ok(m) = std::fs::metadata(path) {
                if m.len() == 0 { 
                    if attempt < max_retries {
                        tracing::debug!("[Indexer] File {} is 0 bytes (attempt {}/{}), waiting for data...", path, attempt, max_retries);
                        tokio::time::sleep(Duration::from_millis(50)).await;
                        continue;
                    } else {
                        // Truly empty file after 10 attempts (500ms)
                        return Ok(String::new());
                    }
                }
            }

            let result = tokio::task::spawn_blocking(move || {
                crate::storage::extract_text_from_file(std::path::Path::new(&path_owned))
            }).await.map_err(|_| MagicError::Other(anyhow::anyhow!("Task panic")))?;

            match result {
                Ok(content) => {
                    if !content.trim().is_empty() { 
                        return Ok(content); 
                    }
                    // If content is empty (but size wasn't 0?), it might be binary or just whitespace.
                    // We can accept it, but if it was a read error, we retry.
                },
                Err(e) => {
                    last_error = Some(e);
                }
            }
            
            // Backoff
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        
        // If we failed after all retries, return empty string (soft fail) or error
        tracing::warn!("[Indexer] Failed to extract text from {} after retries. Last error: {:?}", path, last_error);
        Ok(String::new())
    }

    fn invalidate_cache(state: SharedState) -> Result<()> {
        let state_guard = state.read().map_err(|_| MagicError::State("Poisoned lock".into()))?;
        state_guard.inode_store.clear_results();
        state_guard.index_version.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
}
