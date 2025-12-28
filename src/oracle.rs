//! Oracle: The Async Brain
//!
//! Handles Vector Search (fastembed-rs) and SQLite (sqlite-vec).
//! Populates the Memory Cache for the Hollow Drive.
//!
//! CRITICAL RULE: Runs on Tokio async runtime + blocking compute threads.
//! Never blocks the FUSE loop.

use crate::state::{SharedState, SearchResult, EmbeddingRequest};
use crate::error::{Result, MagicError};
use tokio::task::JoinHandle;
use std::sync::Arc;
use fastembed::{TextEmbedding, InitOptions, EmbeddingModel};
use rusqlite::{Connection, params};
use bytemuck;
use anyhow;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};

/// The Oracle: async brain for semantic search
pub struct Oracle {
    /// Shared state for communicating with Hollow Drive and Librarian
    pub state: SharedState,

    /// Tokio runtime for async operations
    pub runtime: Arc<tokio::runtime::Runtime>,

    /// Handle to the Oracle task
    pub task_handle: Option<JoinHandle<()>>,
}

impl Oracle {
    /// Create a new Oracle instance
    pub fn new(state: SharedState) -> Result<Self> {
        let runtime = tokio::runtime::Runtime::new()
            .map_err(|e| anyhow::anyhow!("Failed to create tokio runtime: {}", e))?;

        Ok(Self {
            state,
            runtime: Arc::new(runtime),
            task_handle: None,
        })
    }

    /// Start the Oracle async task and the Embedding Actor
    pub fn start(&mut self) -> Result<()> {
        // Start the dedicated embedding actor thread
        self.start_embedding_actor()?;

        let state = Arc::clone(&self.state);
        let handle = self.runtime.spawn(async move {
            Oracle::run_task(state).await;
        });

        self.task_handle = Some(handle);
        tracing::info!("[Oracle] Started async task");

        Ok(())
    }

    /// Start the dedicated thread for the Embedding Model (Actor Pattern)
    fn start_embedding_actor(&self) -> Result<()> {
        let state = Arc::clone(&self.state);
        let (tx, mut rx) = mpsc::channel::<EmbeddingRequest>(100);

        // Store the sender in global state
        {
            let state_guard = state.write().map_err(|_| MagicError::State("Poisoned lock".into()))?;
            *state_guard.embedding_tx.write().unwrap() = Some(tx);
        }

        // Spawn a dedicated OS thread (std::thread)
        // This ensures the ONNX runtime stays pinned to ONE thread, preventing FFI race conditions
        std::thread::spawn(move || {
            tracing::info!("[EmbeddingActor] Starting dedicated model thread...");
            
            // Initialize model on this thread
            let model_result = TextEmbedding::try_new(InitOptions::new(EmbeddingModel::BGESmallENV15));
            
            let mut model = match model_result {
                Ok(m) => {
                    tracing::info!("[EmbeddingActor] Model loaded successfully");
                    m
                },
                Err(e) => {
                    tracing::error!("[EmbeddingActor] Failed to load model: {}", e);
                    return; // Exit thread if model fails
                }
            };

            // Process requests loop
            while let Some(request) = rx.blocking_recv() {
                let EmbeddingRequest { content, respond_to } = request;
                
                // Generate embedding
                let result = model.embed(vec![content], None)
                    .map(|mut res| res.remove(0))
                    .map_err(|e| MagicError::Embedding(format!("FastEmbed error: {}", e)));
                
                // Send response back
                let _ = respond_to.send(result);
            }

            tracing::info!("[EmbeddingActor] Shutting down");
        });

        Ok(())
    }

    /// Main async task loop
    async fn run_task(state: SharedState) {
        tracing::info!("[Oracle] Async task started");

        // Wait for embedding actor to be reachable
        tracing::info!("[Oracle] Waiting for embedding actor...");
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
        tracing::info!("[Oracle] Embedding actor ready");

        let mut processed_queries: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut processed_files: std::collections::HashSet<String> = std::collections::HashSet::new();

        loop {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;

            // Phase 4: Check for new searches
            let queries_to_process: Vec<(String, u64)> = {
                let state_guard = state.read().map_err(|_| MagicError::State("Poisoned lock".into())).unwrap();
                state_guard.active_searches.iter()
                    .filter_map(|entry| {
                        let query = entry.key().clone();
                        let inode_num = *entry.value();
                        if !processed_queries.contains(&query) && state_guard.search_results.get(&inode_num).is_none() {
                            Some((query, inode_num))
                        } else {
                            None
                        }
                    })
                    .collect()
            };

            // Phase 5: Check for new files to index
            let files_to_process: Vec<String> = {
                let state_guard = state.read().map_err(|_| MagicError::State("Poisoned lock".into())).unwrap();
                let mut files_to_index_lock = state_guard.files_to_index.lock().unwrap_or_else(|e| e.into_inner());
                
                // Only take files if we haven't processed them to avoid loops
                // Note: In production we'd want a more robust queue system
                files_to_index_lock.drain(..)
                    .filter(|file| !processed_files.contains(file))
                    .collect()
            };

            // Process queries
            for (query, _inode_num) in queries_to_process {
                let state_to_process = Arc::clone(&state);
                processed_queries.insert(query.clone());
                tokio::spawn(async move {
                    if let Err(e) = Oracle::process_search_query(state_to_process, query).await {
                        tracing::error!("[Oracle] Error processing search: {}", e);
                    }
                });
            }

            // Process files
            for file_path in files_to_process {
                let state_to_process = Arc::clone(&state);

                if file_path.starts_with("DELETE:") {
                    let actual_path = file_path.trim_start_matches("DELETE:").to_string();
                    processed_files.insert(file_path.clone());
                    tokio::spawn(async move {
                        if let Err(e) = Oracle::handle_file_delete(state_to_process, actual_path).await {
                            tracing::error!("[Oracle] Error handling file delete: {}", e);
                        }
                    });
                } else {
                    processed_files.insert(file_path.clone());
                    // NOTE: Indexing is now safe to spawn concurrently because the Actor serializes the 
                    // actual embedding generation on its own thread!
                    tokio::spawn(async move {
                        if let Err(e) = Oracle::index_file(state_to_process, file_path).await {
                            tracing::error!("[Oracle] Error indexing file: {}", e);
                        }
                    });
                }
            }
        }
    }

    /// Process a search query end-to-end
    async fn process_search_query(state: SharedState, query: String) -> Result<()> {
        tracing::info!("[Oracle] Processing search: {}", query);

        let results = Oracle::perform_vector_search(state.clone(), query.clone()).await?;

        // FIX: Lifetime issue resolved by extracting value immediately
        let inode_num = {
            let state_guard = state.read().map_err(|_| MagicError::State("Poisoned lock".into()))?;
            state_guard.active_searches.get(&query)
                .map(|v| *v.value())
                .ok_or_else(|| MagicError::State("Query not found".into()))?
        };

        {
            let state_guard = state.write().map_err(|_| MagicError::State("Poisoned lock".into()))?;
            state_guard.search_results.insert(inode_num, results.clone());
        }

        tracing::info!("[Oracle] Search complete: {} ({} results)", query, results.len());
        Ok(())
    }

    /// Perform vector similarity search
    async fn perform_vector_search(state: SharedState, query: String) -> Result<Vec<SearchResult>> {
        // Step 1: Generate embedding via Actor
        let query_embedding = Oracle::request_embedding(&state, query.clone()).await?;

        tracing::debug!("[Oracle] Generated embedding for query: {} ({} dims)", query, query_embedding.len());

        // Step 2: Database search
        let state_for_search = state.clone();
        let results = tokio::task::block_in_place(move || {
            let state_guard = state_for_search.read()
                .map_err(|_| MagicError::State("Poisoned lock".into()))?;
            let conn_lock = state_guard.db_connection.lock()
                .map_err(|_| MagicError::State("Poisoned lock".into()))?;
            let conn_ref = conn_lock.as_ref()
                .ok_or_else(|| MagicError::Other(anyhow::anyhow!("Database not initialized")))?;

            perform_sqlite_vector_search(conn_ref, &query_embedding)
        })?;

        Ok(results)
    }

    /// Request an embedding from the Actor
    async fn request_embedding(state: &SharedState, content: String) -> Result<Vec<f32>> {
        let tx = {
            let state_guard = state.read().map_err(|_| MagicError::State("Poisoned lock".into()))?;
            let tx_guard = state_guard.embedding_tx.read().unwrap();
            tx_guard.clone().ok_or_else(|| MagicError::Embedding("Actor not ready".into()))?
        };

        let (resp_tx, resp_rx) = oneshot::channel();
        let req = EmbeddingRequest {
            content,
            respond_to: resp_tx,
        };

        tx.send(req).await.map_err(|_| MagicError::Embedding("Actor channel closed".into()))?;

        resp_rx.await.map_err(|_| MagicError::Embedding("Actor dropped request".into()))?
    }

    /// Stop the Oracle
    pub async fn stop(&mut self) -> Result<()> {
        if let Some(handle) = self.task_handle.take() {
            handle.abort();
            let _ = handle.await;
            tracing::info!("[Oracle] Stopped");
        }
        Ok(())
    }

    /// Index a file
    async fn index_file(state: SharedState, file_path: String) -> Result<()> {
        tracing::info!("[Oracle] Indexing file: {}", file_path);
        let file_path_clone = file_path.clone();

        // Step 1: Extract text
        let text_content = tokio::task::spawn_blocking(move || {
            crate::storage::extract_text_from_file(std::path::Path::new(&file_path_clone))
        }).await.map_err(|_| MagicError::Other(anyhow::anyhow!("Text extraction task panic")))??;

        if text_content.trim().is_empty() {
            tracing::warn!("[Oracle] File has no text content: {}", file_path);
            return Ok(());
        }

        // Step 2: Generate embedding via Actor
        let embedding = Oracle::request_embedding(&state, text_content).await?;

        // Step 3: Register file in database
        let (file_id, _inode) = {
            let inode = file_path.len() as u64 + 0x100000;
            let metadata = std::fs::metadata(&file_path).map_err(MagicError::Io)?;
            let mtime = metadata.modified().map_err(MagicError::Io)?.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
            let size = metadata.len();
            let is_dir = metadata.is_dir();

            let file_id = crate::storage::register_file(
                &state, &file_path, inode, mtime, size, is_dir
            )?;
            (file_id, inode)
        };

        // Step 4: Insert embedding
        match crate::storage::insert_embedding(&state, file_id, &embedding) {
            Ok(_) => tracing::debug!("Inserted embedding for file_id: {}", file_id),
            Err(e) => tracing::warn!("Failed to store embedding: {}", e),
        }

        // Step 5: Invalidate caches
        Oracle::invalidate_caches_after_index(state.clone(), file_path.clone())?;

        tracing::info!("[Oracle] Successfully indexed file: {} (file_id: {})", file_path, file_id);
        Ok(())
    }

    // Helper functions for cache invalidation
    fn invalidate_caches_after_index(state: SharedState, file_path: String) -> Result<()> {
        let state_guard = state.write().map_err(|_| MagicError::State("Poisoned lock".into()))?;
        state_guard.search_results.clear();
        tracing::debug!("[Oracle] Cleared search results cache after indexing: {}", file_path);
        Ok(())
    }

    fn invalidate_caches_after_delete(state: SharedState, file_path: String) -> Result<()> {
        let state_guard = state.write().map_err(|_| MagicError::State("Poisoned lock".into()))?;
        state_guard.search_results.clear();
        tracing::debug!("[Oracle] Cleared search results cache after deletion: {}", file_path);
        Ok(())
    }

    async fn handle_file_delete(state: SharedState, file_path: String) -> Result<()> {
        tracing::info!("[Oracle] Handling file deletion: {}", file_path);
        let file_record = crate::storage::get_file_by_path(&state, &file_path)?;

        if let Some(file_record) = file_record {
            crate::storage::delete_embedding(&state, file_record.file_id)?;
            crate::storage::delete_file(&state, &file_path)?;
        } else {
            tracing::warn!("[Oracle] File not found in registry: {}", file_path);
        }

        Oracle::invalidate_caches_after_delete(state.clone(), file_path.clone())?;
        tracing::info!("[Oracle] Successfully handled file deletion: {}", file_path);
        Ok(())
    }
}

// FIX: Updated query to use subquery, ensuring LIMIT applies to vec0
fn perform_sqlite_vector_search(db_conn: &Connection, query_embedding: &[f32]) -> Result<Vec<SearchResult>> {
    let embedding_bytes: Vec<u8> = bytemuck::cast_slice(query_embedding).to_vec();
    
    // We must use a subquery to ensure the LIMIT applies directly to the vec0 virtual table
    // BEFORE joining, otherwise the query optimizer fails to push down the limit
    let query_sql = "
        SELECT
            fr.file_id,
            fr.abs_path,
            v.distance as score
        FROM (
            SELECT file_id, distance
            FROM vec_index
            WHERE embedding MATCH ?
            ORDER BY distance
            LIMIT 10
        ) v
        JOIN file_registry fr ON v.file_id = fr.file_id";

    let mut stmt = db_conn.prepare(query_sql)?;
    let results = stmt.query_map(params![embedding_bytes], |row| {
        let abs_path: String = row.get("abs_path")?;
        // Convert distance to similarity score (approximate)
        // distance is cosine distance (0 to 2), so 1 - distance/2 is roughly similarity?
        // Actually, let's just return 1.0 - distance for now as a simple metric
        let distance: f32 = row.get("score")?;
        let score = 1.0 - distance;

        let filename = std::path::Path::new(&abs_path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(&abs_path)
            .to_string();

        Ok(SearchResult {
            file_id: row.get("file_id")?,
            abs_path,
            score,
            filename,
        })
    })?.filter_map(|row| row.ok()).collect::<Vec<_>>();

    Ok(results)
}

impl Drop for Oracle {
    fn drop(&mut self) {
        if let Some(handle) = &self.task_handle {
            handle.abort();
        }
    }
}
