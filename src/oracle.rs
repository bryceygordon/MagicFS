// FILE: src/oracle.rs
//! Oracle: The Async Brain (Orchestrator)
//!
//! Refactored in Phase 6.3.
//! Role:
//! 1. Manages the Thread/Actor Lifecycle.
//! 2. Monitors the Event Loop.
//! 3. Delegates work to `Indexer` and `Searcher`.

use crate::state::{SharedState, EmbeddingRequest};
use crate::error::{Result, MagicError};
use crate::engine::indexer::Indexer;
use crate::engine::searcher::Searcher;

use tokio::task::JoinHandle;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use fastembed::{TextEmbedding, InitOptions, EmbeddingModel};
use anyhow;
use std::time::Duration;
use tokio::sync::mpsc;

pub struct Oracle {
    pub state: SharedState,
    pub runtime: Arc<tokio::runtime::Runtime>,
    pub task_handle: Option<JoinHandle<()>>,
}

impl Oracle {
    pub fn new(state: SharedState) -> Result<Self> {
        let runtime = tokio::runtime::Runtime::new()
            .map_err(|e| anyhow::anyhow!("Failed to create tokio runtime: {}", e))?;

        Ok(Self {
            state,
            runtime: Arc::new(runtime),
            task_handle: None,
        })
    }

    pub fn start(&mut self) -> Result<()> {
        self.start_embedding_actor()?;
        let state = Arc::clone(&self.state);
        let handle = self.runtime.spawn(async move {
            Oracle::run_event_loop(state).await;
        });
        self.task_handle = Some(handle);
        tracing::info!("[Oracle] Started Orchestrator loop");
        Ok(())
    }

    fn start_embedding_actor(&self) -> Result<()> {
        let state = Arc::clone(&self.state);
        let (tx, mut rx) = mpsc::channel::<EmbeddingRequest>(100);

        {
            let state_guard = state.write().map_err(|_| MagicError::State("Poisoned lock".into()))?;
            *state_guard.embedding_tx.write().unwrap() = Some(tx);
        }

        std::thread::spawn(move || {
            tracing::info!("[EmbeddingActor] Starting dedicated model thread...");
            let model_result = TextEmbedding::try_new(InitOptions::new(EmbeddingModel::BGESmallENV15));
            let mut model = match model_result {
                Ok(m) => {
                    tracing::info!("[EmbeddingActor] Model loaded successfully");
                    m
                },
                Err(e) => {
                    tracing::error!("[EmbeddingActor] Failed to load model: {}", e);
                    return; 
                }
            };

            while let Some(request) = rx.blocking_recv() {
                let EmbeddingRequest { content, respond_to } = request;
                let result = model.embed(vec![content], None)
                    .map(|mut res| res.remove(0))
                    .map_err(|e| MagicError::Embedding(format!("FastEmbed error: {}", e)));
                let _ = respond_to.send(result);
            }
            tracing::info!("[EmbeddingActor] Shutting down");
        });
        Ok(())
    }

    async fn run_event_loop(state: SharedState) {
        tracing::info!("[Oracle] Event loop active");

        // Wait for actor
        let mut actor_ready = false;
        for _ in 0..100 { 
            tokio::time::sleep(Duration::from_millis(100)).await;
            let state_guard = state.read().unwrap();
            if state_guard.embedding_tx.read().unwrap().is_some() {
                actor_ready = true;
                break;
            }
        }
        if !actor_ready {
            tracing::error!("[Oracle] Embedding actor failed to initialize!");
            return;
        }

        let mut processed_queries: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut processed_files: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut last_index_version = 0;

        loop {
            tokio::time::sleep(Duration::from_millis(100)).await;

            // 1. Check for Index Changes (Cache Invalidation)
            let current_index_version = {
                let state_guard = state.read().unwrap();
                state_guard.index_version.load(Ordering::Relaxed)
            };

            if current_index_version != last_index_version {
                tracing::debug!("[Oracle] Index version changed. Resetting processed_queries.");
                processed_queries.clear();
                
                // Note: The InodeStore is cleared by the Indexer, but we ensure consistency here
                last_index_version = current_index_version;
            }

            // 2. Identify Pending Searches
            let queries_to_process: Vec<(String, u64)> = {
                let state_guard = state.read().map_err(|_| MagicError::State("Poisoned lock".into())).unwrap();
                state_guard.inode_store.active_queries()
                    .into_iter()
                    .filter(|(inode, query)| {
                        !processed_queries.contains(query) && !state_guard.inode_store.has_results(*inode)
                    })
                    .map(|(inode, query)| (query, inode))
                    .collect()
            };

            // 3. Identify Pending Files
            let files_to_process: Vec<String> = {
                let state_guard = state.read().map_err(|_| MagicError::State("Poisoned lock".into())).unwrap();
                let mut files_to_index_lock = state_guard.files_to_index.lock().unwrap_or_else(|e| e.into_inner());
                files_to_index_lock.drain(..)
                    .filter(|file| !processed_files.contains(file))
                    .collect()
            };

            // 4. Dispatch Search Tasks
            for (query, inode_num) in queries_to_process {
                let state_ref = Arc::clone(&state);
                processed_queries.insert(query.clone());
                
                tokio::spawn(async move {
                    if let Err(e) = Searcher::perform_search(state_ref, query, inode_num).await {
                        tracing::error!("[Oracle] Search failed: {}", e);
                    }
                });
            }

            // 5. Dispatch Indexing Tasks
            for file_path in files_to_process {
                let state_ref = Arc::clone(&state);
                
                if file_path.starts_with("DELETE:") {
                    let actual_path = file_path.trim_start_matches("DELETE:").to_string();
                    processed_files.insert(file_path.clone());
                    
                    tokio::spawn(async move {
                        if let Err(e) = Indexer::remove_file(state_ref, actual_path).await {
                            tracing::error!("[Oracle] File removal failed: {}", e);
                        }
                    });
                } else {
                    processed_files.insert(file_path.clone());
                    
                    tokio::spawn(async move {
                        if let Err(e) = Indexer::index_file(state_ref, file_path).await {
                            tracing::error!("[Oracle] Indexing failed: {}", e);
                        }
                    });
                }
            }
        }
    }

    pub async fn stop(&mut self) -> Result<()> {
        if let Some(handle) = self.task_handle.take() {
            handle.abort();
            let _ = handle.await;
        }
        Ok(())
    }
}

impl Drop for Oracle {
    fn drop(&mut self) {
        if let Some(handle) = &self.task_handle {
            handle.abort();
        }
    }
}
