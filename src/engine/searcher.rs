// FILE: src/engine/searcher.rs
use crate::state::SharedState;
use crate::error::{Result, MagicError};
use crate::storage::Repository;
use crate::engine::request_embedding;

pub struct Searcher;

impl Searcher {
    /// Perform a semantic search and update the InodeStore
    pub async fn perform_search(state: SharedState, query: String, expected_inode: u64) -> Result<()> {
        // 1. Generate Embedding
        let query_embedding = request_embedding(&state, query.clone()).await?;

        // 2. Search Database (Blocking)
        let state_for_search = state.clone();
        let results = tokio::task::block_in_place(move || {
            let state_guard = state_for_search.read()
                .map_err(|_| MagicError::State("Poisoned lock".into()))?;
            let conn_lock = state_guard.db_connection.lock()
                .map_err(|_| MagicError::State("Poisoned lock".into()))?;
            let conn_ref = conn_lock.as_ref()
                .ok_or_else(|| MagicError::Other(anyhow::anyhow!("Database not initialized")))?;
            
            let repo = Repository::new(conn_ref);
            // Limit results to 20
            repo.search(&query_embedding, 20)
        })?;

        // 3. Update InodeStore
        let state_guard = state.read().map_err(|_| MagicError::State("Poisoned lock".into()))?;
        let current_inode = state_guard.inode_store.get_or_create_inode(&query);
        
        if current_inode != expected_inode {
             tracing::warn!("[Searcher] Inode mismatch for '{}'. Expected: {}, Got: {}", query, expected_inode, current_inode);
        }

        state_guard.inode_store.put_results(current_inode, results);
        tracing::info!("[Searcher] Completed search for '{}' ({} results)", query, state_guard.inode_store.get_results(current_inode).map(|v| v.len()).unwrap_or(0));
        Ok(())
    }
}
