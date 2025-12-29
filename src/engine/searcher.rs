// FILE: src/engine/searcher.rs
use crate::state::SharedState;
use crate::error::{Result, MagicError};
use crate::storage::Repository;
use crate::engine::request_embedding;

pub struct Searcher;

impl Searcher {
    /// Perform a semantic search and update the InodeStore
    /// GUARANTEE: This function MUST update InodeStore, even on failure.
    /// If it fails, it writes an empty result set to break the EAGAIN loop.
    pub async fn perform_search(state: SharedState, query: String, expected_inode: u64) -> Result<()> {
        // 1. Generate Embedding
        // If this fails, we can't even search, so we return empty results immediately.
        let query_embedding = match request_embedding(&state, query.clone()).await {
            Ok(e) => e,
            Err(e) => {
                tracing::error!("[Searcher] Embedding failed for '{}': {}", query, e);
                Self::write_empty_results(&state, expected_inode);
                return Err(e);
            }
        };

        // 2. Search Database (Blocking)
        let state_for_search = state.clone();
        let results_result = tokio::task::block_in_place(move || {
            let state_guard = state_for_search.read()
                .map_err(|_| MagicError::State("Poisoned lock".into()))?;
            let conn_lock = state_guard.db_connection.lock()
                .map_err(|_| MagicError::State("Poisoned lock".into()))?;
            let conn_ref = conn_lock.as_ref()
                .ok_or_else(|| MagicError::Other(anyhow::anyhow!("Database not initialized")))?;
            
            let repo = Repository::new(conn_ref);
            // Limit results to 20
            repo.search(&query_embedding, 20)
        });

        match results_result {
            Ok(results) => {
                let state_guard = state.read().map_err(|_| MagicError::State("Poisoned lock".into()))?;
                let current_inode = state_guard.inode_store.get_or_create_inode(&query);
                
                if current_inode != expected_inode {
                     tracing::warn!("[Searcher] Inode mismatch for '{}'. Expected: {}, Got: {}", query, expected_inode, current_inode);
                }

                let count = results.len();
                state_guard.inode_store.put_results(current_inode, results);
                tracing::info!("[Searcher] Completed search for '{}' ({} results)", query, count);
                Ok(())
            },
            Err(e) => {
                tracing::error!("[Searcher] DB Search failed for '{}': {}", query, e);
                Self::write_empty_results(&state, expected_inode);
                Err(e)
            }
        }
    }

    fn write_empty_results(state: &SharedState, inode: u64) {
        if let Ok(guard) = state.read() {
            guard.inode_store.put_results(inode, Vec::new());
            tracing::warn!("[Searcher] Wrote EMPTY results for Inode {} to break EAGAIN loop", inode);
        }
    }
}
