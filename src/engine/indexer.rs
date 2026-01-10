// FILE: src/engine/indexer.rs
use crate::state::SharedState;
use crate::error::{Result, MagicError};
use crate::storage::Repository;
use crate::engine::request_embedding_batch;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::io;

pub struct Indexer;

impl Indexer {
    /// Orchestrates the indexing of a single file:
    /// 1. Extract text (with retries)
    /// 2. Chunk text
    /// 3. Register file in DB
    /// 4. Generate embeddings (BATCHED)
    /// 5. Insert embeddings (TRANSACTIONAL)
    /// 6. Phase 17: Auto-apply Tag ID 1 (Inbox) if file is in system inbox
    pub async fn index_file(state: SharedState, file_path: String) -> Result<()> {
        tracing::info!("[Indexer] Processing: {}", file_path);

        // 1. Extraction (with retries for file locks AND partial writes)
        let text_content = match Self::extract_with_retry(&file_path).await {
            Ok(t) => t,
            Err(e) => return Err(e),
        };

        // 2. Chunking (even empty files produce empty chunks)
        let chunks = crate::storage::text_extraction::chunk_text(&text_content);

        // 3. Register File & Clean Old Data (ALWAYS RUN - even for empty files)
        let (file_id, _inode) = {
            let state_guard = state.read().map_err(|_| MagicError::State("Poisoned lock".into()))?;
            // Acquire mutable lock for registration
            let mut conn_lock = state_guard.db_connection.lock().map_err(|_| MagicError::State("Poisoned lock".into()))?;
            let conn = conn_lock.as_mut().ok_or_else(|| MagicError::Other(anyhow::anyhow!("Database not initialized")))?;

            let repo = Repository::new(conn);

            // Mock inode logic
            let inode = file_path.len() as u64 + 0x100000;
            let metadata = std::fs::metadata(&file_path).map_err(MagicError::Io)?;
            let mtime = metadata.modified().map_err(MagicError::Io)?.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
            let size = metadata.len();
            let is_dir = metadata.is_dir();

            let fid = repo.register_file(
                &file_path, inode, mtime, size, is_dir
            )?;

            // Always clear old embeddings
            repo.delete_embeddings_for_file(fid)?;

            // Validate what we just learned from the metadata
            if size == 0 {
                tracing::debug!("[Indexer] Zero-byte file registered: {}", file_path);
            } else if text_content.trim().is_empty() {
                tracing::warn!("[Indexer] Non-empty file produced no text: {}", file_path);
            }

            (fid, inode)
        };

        // 4. Generate Embeddings ONLY if we have chunks
        if !chunks.is_empty() {
            tracing::debug!("[Indexer] {} split into {} chunks", file_path, chunks.len());

            // is_query = false (We are indexing DOCUMENTS)
            // This sends ALL chunks to the GPU/CPU in one go.
            let embeddings_batch = request_embedding_batch(&state, chunks, false).await?;

            // 5. Insert Embeddings (TRANSACTIONAL)
            {
                let state_guard = state.read().map_err(|_| MagicError::State("Poisoned lock".into()))?;
                let mut conn_lock = state_guard.db_connection.lock().map_err(|_| MagicError::State("Poisoned lock".into()))?;
                // We need a mutable connection for transactions
                let conn = conn_lock.as_mut().ok_or_else(|| MagicError::Other(anyhow::anyhow!("Database not initialized")))?;

                // 'repo' needs to be mut because insert_embeddings_batch is &mut self
                let mut repo = Repository::new(conn);

                if let Err(e) = repo.insert_embeddings_batch(file_id, embeddings_batch) {
                    tracing::warn!("[Indexer] Failed to insert batch: {}", e);
                }
            }
        } else {
            tracing::debug!("[Indexer] No chunks for {} - skipping embedding generation", file_path);
            // The transaction that cleared old embeddings (line 69) is sufficient
            // This file will have file_registry entry but empty vec_index
        }

        // 6. Apply Phase 17 Auto-Tagging (for ALL files, including empty)
        {
            let state_guard = state.read().map_err(|_| MagicError::State("Poisoned lock".into()))?;
            let mut conn_lock = state_guard.db_connection.lock().map_err(|_| MagicError::State("Poisoned lock".into()))?;
            let conn = conn_lock.as_mut().ok_or_else(|| MagicError::Other(anyhow::anyhow!("Database not initialized")))?;
            let mut repo = Repository::new(conn);

            // Check if file is in system inbox and auto-link to Tag ID 1
            let is_in_inbox = {
                let inbox_path_guard = state_guard.system_inbox_path.lock().unwrap();
                if let Some(inbox_path) = inbox_path_guard.as_ref() {
                    file_path.starts_with(inbox_path)
                } else {
                    false
                }
            };

            if is_in_inbox {
                tracing::info!("[Indexer] File in system inbox, auto-linking to Tag ID 1 (inbox)");
                let filename = std::path::Path::new(&file_path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown");

                if let Err(e) = repo.link_file(file_id, 1, filename) {
                    tracing::warn!("[Indexer] Failed to auto-link file to inbox tag: {}", e);
                } else {
                    tracing::debug!("[Indexer] Successfully linked file_id={} to tag_id=1", file_id);
                }
            }
        }

        // 7. Invalidate Caches
        Self::invalidate_cache(state)?;
        tracing::info!("[Indexer] Indexed {} (ID: {})", file_path, file_id);
        Ok(())
    }

    pub async fn remove_file(state: SharedState, file_path: String) -> Result<()> {
        if std::path::Path::new(&file_path).exists() {
            tracing::warn!("[Arbitrator] Delete request for '{}' rejected - file exists on disk. Re-indexing instead.", file_path);
            return Self::index_file(state, file_path).await;
        }

        {
            let state_guard = state.read().map_err(|_| MagicError::State("Poisoned lock".into()))?;
            // Acquire mutable lock for deletion
            let mut conn_lock = state_guard.db_connection.lock().map_err(|_| MagicError::State("Poisoned lock".into()))?;
            let conn = conn_lock.as_mut().ok_or_else(|| MagicError::Other(anyhow::anyhow!("Database not initialized")))?;
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
        let max_retries = 20;
        let mut last_error = None;

        for attempt in 1..=max_retries {
            let path_owned = path.to_string();
            let path_obj = std::path::Path::new(path);

            // --- NEW CHECK: ABORT IF VANISHED ---
            if !path_obj.exists() {
                tracing::debug!("[Indexer] File vanished during processing (transient?): {}", path);
                return Ok(String::new()); // Treat as empty, do not error
            }
            // ------------------------------------

            // PHASE 24 FIX: Zero-Byte Citizenship
            // Remove the sleep/retry loop for 0-byte files. Treat them as valid final state.
            if let Ok(m) = std::fs::metadata(path) {
                if m.len() == 0 {
                    // ZERO-BYTE FILES ARE CITIZENS TOO!
                    tracing::debug!("[Indexer] File {} is 0 bytes - treating as valid empty content", path);
                    return Ok(String::new());
                }
            }

            let result = tokio::task::spawn_blocking(move || {
                crate::storage::extract_text_from_file(std::path::Path::new(&path_owned))
            }).await.map_err(|_| MagicError::Other(anyhow::anyhow!("Task panic")))?;

            match result {
                Ok(content) => return Ok(content),
                Err(MagicError::Io(e)) if e.kind() == io::ErrorKind::PermissionDenied => {
                    if attempt < max_retries {
                        tracing::warn!("[Indexer] Locked/PermissionDenied for {} (attempt {}/{}), waiting...", path, attempt, max_retries);
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        continue;
                    } else {
                        return Err(MagicError::Io(e));
                    }
                }
                Err(e) => {
                    last_error = Some(e);
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }

        tracing::warn!("[Indexer] Failed to extract from {} after retries. Last error: {:?}", path, last_error);
        if let Some(e) = last_error { Err(e) } else { Ok(String::new()) }
    }

    fn invalidate_cache(state: SharedState) -> Result<()> {
        let state_guard = state.read().map_err(|_| MagicError::State("Poisoned lock".into()))?;
        state_guard.inode_store.clear_results();
        state_guard.index_version.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
}
