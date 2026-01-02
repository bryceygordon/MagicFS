// FILE: src/engine/mod.rs
pub mod indexer;
pub mod searcher;

use crate::state::{SharedState, EmbeddingRequest};
use crate::error::MagicError;
use tokio::sync::oneshot;

/// Helper: Standardized way to request an embedding from the Actor
/// UPDATED: Accepts Vec<String>, returns Vec<Vec<f32>>
pub async fn request_embedding_batch(state: &SharedState, content: Vec<String>, is_query: bool) -> crate::error::Result<Vec<Vec<f32>>> {
    let tx = {
        let state_guard = state.read().map_err(|_| MagicError::State("Poisoned lock".into()))?;
        let tx_guard = state_guard.embedding_tx.read().unwrap();
        tx_guard.clone().ok_or_else(|| MagicError::Embedding("Actor not ready".into()))?
    };

    let (resp_tx, resp_rx) = oneshot::channel();
    
    let req = EmbeddingRequest { content, is_query, respond_to: resp_tx };

    tx.send(req).await.map_err(|_| MagicError::Embedding("Actor channel closed".into()))?;
    resp_rx.await.map_err(|_| MagicError::Embedding("Actor dropped request".into()))?
}
