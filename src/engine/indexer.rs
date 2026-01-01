// FILE: src/engine/indexer.rs
use crate::state::SharedState;
use crate::error::{Result, MagicError};
use crate::storage::Repository;
use crate::engine::request_embedding;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::io;

pub struct Indexer;

impl Indexer {
    pub async fn index_file(state: SharedState, file_path: String) -> Result<()> {
        tracing::info!("[Indexer] Processing: {}", file_path);
        
        let text_content = match Self::extract_with_retry(&file_path).await {
            Ok(t) => t,
            Err(e) => return Err(e),
        };

        if text_content.trim().is_empty() {
            // ... (Diagnostic logging omitted for brevity, logic same as before)
            return Ok(());
        }

        let chunks = crate::storage::text_extraction::chunk_text(&text_content);
        if chunks.is_empty() { return Ok(()); }

        let (file_id, _inode) = {
            let state_guard = state.read().map_err(|_| MagicError::State("Poisoned lock".into()))?;
            let conn_lock = state_guard.db_connection.lock().map_err(|_| MagicError::State("Poisoned lock".into()))?;
            let conn = conn_lock.as_ref().ok_or_else(|| MagicError::Other(anyhow::anyhow!("Database not initialized")))?;
            let repo = Repository::new(conn);

            let inode = file_path.len() as u64 + 0x100000; 
            let metadata = std::fs::metadata(&file_path).map_err(MagicError::Io)?;
            let mtime = metadata.modified().map_err(MagicError::Io)?.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
            let size = metadata.len();
            let is_dir = metadata.is_dir();

            let fid = repo.register_file(&file_path, inode, mtime, size, is_dir)?;
            repo.delete_embeddings_for_file(fid)?;
            (fid, inode)
        };

        for chunk in chunks {
            // --- REVERT: No Prefix for Snowflake Documents ---
            let embedding = request_embedding(&state, chunk).await?;
            
            let state_guard = state.read().map_err(|_| MagicError::State("Poisoned lock".into()))?;
            let conn_lock = state_guard.db_connection.lock().map_err(|_| MagicError::State("Poisoned lock".into()))?;
            let conn = conn_lock.as_ref().ok_or_else(|| MagicError::Other(anyhow::anyhow!("Database not initialized")))?;
            let repo = Repository::new(conn);

            if let Err(e) = repo.insert_embedding(file_id, &embedding) {
                tracing::warn!("[Indexer] Failed to insert chunk: {}", e);
            }
        }

        Self::invalidate_cache(state)?;
        tracing::info!("[Indexer] Indexed {} (ID: {})", file_path, file_id);
        Ok(())
    }

    // ... (remove_file, extract_with_retry, invalidate_cache same as before)
    // To save space, assume the rest of the file is identical to previous version
    // If you need the full file again, I can print it, but this is the only logic change.
    pub async fn remove_file(state: SharedState, file_path: String) -> Result<()> {
        if std::path::Path::new(&file_path).exists() {
            return Self::index_file(state, file_path).await;
        }
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
        Ok(())
    }

    async fn extract_with_retry(path: &str) -> Result<String> {
        let max_retries = 20; 
        let mut last_error = None;
        for attempt in 1..=max_retries {
            let path_owned = path.to_string();
            if let Ok(m) = std::fs::metadata(path) {
                if m.len() == 0 { 
                    if attempt < max_retries {
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        continue;
                    } else {
                        return Ok(String::new());
                    }
                }
            }
            let result = tokio::task::spawn_blocking(move || {
                crate::storage::extract_text_from_file(std::path::Path::new(&path_owned))
            }).await.map_err(|_| MagicError::Other(anyhow::anyhow!("Task panic")))?;

            match result {
                Ok(content) => return Ok(content),
                Err(MagicError::Io(e)) if e.kind() == io::ErrorKind::PermissionDenied => {
                    if attempt < max_retries {
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        continue;
                    } else { return Err(MagicError::Io(e)); }
                },
                Err(e) => { last_error = Some(e); tokio::time::sleep(Duration::from_millis(100)).await; }
            }
        }
        if let Some(e) = last_error { Err(e) } else { Ok(String::new()) }
    }

    fn invalidate_cache(state: SharedState) -> Result<()> {
        let state_guard = state.read().map_err(|_| MagicError::State("Poisoned lock".into()))?;
        state_guard.inode_store.clear_results();
        state_guard.index_version.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
}
