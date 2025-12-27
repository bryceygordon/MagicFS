//! Oracle: The Async Brain
//!
//! Handles Vector Search (fastembed-rs) and SQLite (sqlite-vec).
//! Populates the Memory Cache for the Hollow Drive.
//!
//! CRITICAL RULE: Runs on Tokio async runtime + blocking compute threads.
//! Never blocks the FUSE loop.

use crate::state::{SharedState, SearchResult};
use crate::error::{Result, MagicError};
use tokio::task::JoinHandle;
use std::sync::Arc;
use fastembed::{TextEmbedding, InitOptions, EmbeddingModel};
use rusqlite::{Connection, params};
use bytemuck;
use anyhow;
use std::time::Duration;

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

    /// Start the Oracle async task
    pub fn start(&mut self) -> Result<()> {
        let state = Arc::clone(&self.state);

        let handle = self.runtime.spawn(async move {
            Oracle::run_task(state).await;
        });

        self.task_handle = Some(handle);
        tracing::info!("[Oracle] Started async task");

        Ok(())
    }

    /// Main async task loop
    async fn run_task(state: SharedState) {
        tracing::info!("[Oracle] Async task started");

        // Wait for embedding model to initialize before processing anything
        tracing::info!("[Oracle] Waiting for embedding model to initialize...");
        let mut model_ready = false;
        for _ in 0..100 { // Wait up to 10 seconds
            tokio::time::sleep(Duration::from_millis(100)).await;

            let state_guard = state.read().map_err(|_| MagicError::State("Poisoned lock".into())).unwrap();
            let model_lock = state_guard.embedding_model.lock().map_err(|_| MagicError::State("Poisoned lock".into())).unwrap();
            if model_lock.is_some() {
                model_ready = true;
                break;
            }
        }

        if !model_ready {
            tracing::warn!("[Oracle] Embedding model not ready after 10 seconds, continuing anyway");
        } else {
            tracing::info!("[Oracle] Embedding model ready, proceeding with indexing");
        }

        // Phase 4: Monitor active_searches for new queries
        // Phase 5: Monitor files_to_index for new files to index

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

                        // Check if this query hasn't been processed yet and has no results
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

                // Check if model is ready before processing files
                let model_lock = state_guard.embedding_model.lock().map_err(|_| MagicError::State("Poisoned lock".into())).unwrap();
                let model_ready = model_lock.is_some();
                drop(model_lock);

                if !model_ready {
                    tracing::debug!("[Oracle] Model not ready, skipping file indexing for this iteration");
                    Vec::new()
                } else {
                    let mut files_to_index_lock = state_guard.files_to_index.lock().unwrap_or_else(|e| e.into_inner());
                    let files_to_process: Vec<String> = files_to_index_lock.drain(..)
                        .filter(|file| !processed_files.contains(file))
                        .collect();
                    files_to_process
                }
            };

            // Process any new queries
            for (query, inode_num) in queries_to_process {
                let state_to_process = Arc::clone(&state);
                processed_queries.insert(query.clone());

                // Spawn task to process this search
                tokio::spawn(async move {
                    if let Err(e) = Oracle::process_search_query(state_to_process, query).await {
                        tracing::error!("[Oracle] Error processing search: {}", e);
                    }
                });
            }

            // Phase 5: Process any new files
            for file_path in files_to_process {
                let state_to_process = Arc::clone(&state);

                // Check if this is a delete marker
                if file_path.starts_with("DELETE:") {
                    let actual_path = file_path.trim_start_matches("DELETE:").to_string();
                    processed_files.insert(file_path.clone());

                    // Spawn task to handle delete
                    tokio::spawn(async move {
                        if let Err(e) = Oracle::handle_file_delete(state_to_process, actual_path).await {
                            tracing::error!("[Oracle] Error handling file delete: {}", e);
                        }
                    });
                } else {
                    processed_files.insert(file_path.clone());

                    // Spawn task to index this file
                    tokio::spawn(async move {
                        if let Err(e) = Oracle::index_file(state_to_process, file_path).await {
                            tracing::error!("[Oracle] Error indexing file: {}", e);
                        }
                    });
                }
            }
        }
    }

    /// Spawn a blocking task to initialize embedding model
    pub fn init_embedding_model(&self) -> Result<()> {
        let state = Arc::clone(&self.state);

        self.runtime.spawn_blocking(move || {
            tracing::info!("[Oracle] Initializing embedding model...");

            // Load the BAAI/bge-small-en-v1.5 model (384 dimensions)
            let model_result = TextEmbedding::try_new(InitOptions::new(EmbeddingModel::BGESmallENV15));

            match model_result {
                Ok(model) => {
                    tracing::info!("[Oracle] Embedding model loaded successfully");

                    // Store model in global state
                    let mut model_guard = state.write().map_err(|_| MagicError::State("Poisoned lock".into())).unwrap();
                    *model_guard.embedding_model.lock().map_err(|_| MagicError::State("Poisoned lock".into())).unwrap() = Some(model);

                    tracing::info!("[Oracle] Embedding model initialization complete");
                }
                Err(e) => {
                    tracing::error!("[Oracle] Failed to initialize model: {}", e);
                }
            }
        });

        Ok(())
    }

    /// Handle a new search query (called by Hollow Drive)
    pub fn handle_new_search(&self, query: String) -> Result<()> {
        tracing::info!("[Oracle] Handling new search: {}", query);

        let state = Arc::clone(&self.state);

        // Spawn async task to process the search
        self.runtime.spawn(async move {
            if let Err(e) = Oracle::process_search_query(state, query).await {
                tracing::error!("[Oracle] Error processing search: {}", e);
            }
        });

        Ok(())
    }

    /// Process a search query end-to-end
    async fn process_search_query(state: SharedState, query: String) -> Result<()> {
        tracing::info!("[Oracle] Processing search: {}", query);

        // Phase 3: Generate embedding for query and search for similar files
        let results = Oracle::perform_vector_search(state.clone(), query.clone()).await?;

        // Update cache
        let inode_num = {
            let state_guard = state.read().map_err(|_| MagicError::State("Poisoned lock".into()))?;
            let inode_num = match state_guard.active_searches.get(&query) {
                Some(entry) => *entry.value(),
                None => return Err(MagicError::State("Query not found".into())),
            };
            drop(state_guard);
            inode_num
        };

        {
            let mut state_guard = state.write().map_err(|_| MagicError::State("Poisoned lock".into()))?;
            state_guard.search_results.insert(inode_num, results.clone());
        }

        tracing::info!("[Oracle] Search complete: {} ({} results)", query, results.len());
        Ok(())
    }

    /// Perform vector similarity search using FastEmbed + sqlite-vec
    async fn perform_vector_search(state: SharedState, query: String) -> Result<Vec<SearchResult>> {
        // Clone the state Arc to share with blocking tasks
        let state_for_embedding = state.clone();
        let query_clone = query.clone();

        // Generate embedding for query in a blocking task
        let query_embedding: Vec<f32> = tokio::task::spawn_blocking(move || {
            let state_guard = state_for_embedding.read()
                .map_err(|_| MagicError::State("Poisoned lock".into()))?;
            let mut model_lock = state_guard.embedding_model.lock()
                .map_err(|_| MagicError::State("Poisoned lock".into()))?;
            let model_opt = model_lock.take()
                .ok_or_else(|| MagicError::Embedding("Model not initialized".into()))?;

            let mut model = model_opt;
            let embedding_vec = model.embed(vec![query_clone.as_str()], None)
                .map_err(|e| MagicError::Embedding(format!("Failed to embed query: {}", e)))?;

            Ok::<Vec<f32>, MagicError>(embedding_vec[0].clone())
        }).await.map_err(|_| MagicError::Other(anyhow::anyhow!("Embed task panic")))??;

        tracing::debug!("[Oracle] Generated embedding for query: {} ({} dims)", query, query_embedding.len());

        // Perform vector similarity search using sqlite-vec
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

    /// Shutdown the Oracle and cleanup resources
    pub async fn shutdown(&mut self) -> Result<()> {
        self.stop().await
    }

    /// Generate embedding for file content (Phase 5)
    /// Called by Librarian when files are created/modified
    pub async fn generate_file_embedding(&self, content: &str) -> Result<Vec<f32>> {
        let state = Arc::clone(&self.state);
        let content_owned = content.to_owned();

        // Use spawn_blocking for embedding generation
        let handle = self.runtime.spawn_blocking(move || {
            let state_guard = state.read()
                .map_err(|_| MagicError::State("Poisoned lock".into()))?;
            let mut model_lock = state_guard.embedding_model.lock()
                .map_err(|_| MagicError::State("Poisoned lock".into()))?;
            let model_opt = model_lock.take()
                .ok_or_else(|| MagicError::Embedding("Model not initialized".into()))?;

            let mut model = model_opt;
            let embedding_vec = model.embed(vec![content_owned.as_str()], None)
                .map_err(|e| MagicError::Embedding(format!("Failed to embed content: {}", e)))?;

            // Put the model back
            *model_lock = Some(model);

            Ok::<Vec<f32>, MagicError>(embedding_vec[0].clone())
        });

        handle.await.map_err(|_| MagicError::Other(anyhow::anyhow!("Embed task panic")))?
    }

    /// Index a file: extract text, generate embedding, update database (Phase 5)
    async fn index_file(state: SharedState, file_path: String) -> Result<()> {
        tracing::info!("[Oracle] Indexing file: {}", file_path);

        // Clone file_path for use in spawning tasks
        let file_path_clone = file_path.clone();

        // Step 1: Extract text from file
        let text_content = tokio::task::spawn_blocking(move || {
            crate::storage::extract_text_from_file(std::path::Path::new(&file_path_clone))
        }).await.map_err(|_| MagicError::Other(anyhow::anyhow!("Text extraction task panic")))??;

        if text_content.trim().is_empty() {
            tracing::warn!("[Oracle] File has no text content: {}", file_path);
            return Ok(());
        }

        // Step 2: Generate embedding
        let embedding = Oracle::generate_embedding_for_content(state.clone(), text_content.clone()).await?;

        // Step 3: Register file in database
        let (file_id, inode) = {
            // Generate a simple inode (in production, use actual filesystem inodes)
            let inode = file_path.len() as u64 + 0x100000; // Simple hash-based inode

            // Get file metadata
            let metadata = std::fs::metadata(&file_path)
                .map_err(|e| MagicError::Io(e))?;

            let mtime = metadata.modified()
                .map_err(|e| MagicError::Io(e))?
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();

            let size = metadata.len();
            let is_dir = metadata.is_dir();

            // Register file
            let file_id = crate::storage::register_file(
                &state,
                &file_path,
                inode,
                mtime,
                size,
                is_dir
            )?;

            (file_id, inode)
        };

        // Step 4: Insert embedding into vec_index (gracefully handle missing table)
        match crate::storage::insert_embedding(&state, file_id, &embedding) {
            Ok(_) => tracing::debug!("Inserted embedding for file_id: {}", file_id),
            Err(e) => tracing::warn!("Failed to store embedding (vec_index may not be available): {}. File still registered.", e),
        }

        // Step 8: Invalidate caches
        Oracle::invalidate_caches_after_index(state.clone(), file_path.clone())?;

        tracing::info!("[Oracle] Successfully indexed file: {} (file_id: {})", file_path, file_id);
        Ok(())
    }

    /// Invalidate caches after file indexing (Phase 5 Step 8)
    fn invalidate_caches_after_index(state: SharedState, file_path: String) -> Result<()> {
        // Clear all search results caches since the index has changed
        // This ensures that new searches will pick up the newly indexed file
        {
            let mut state_guard = state.write()
                .map_err(|_| MagicError::State("Poisoned lock".into()))?;
            state_guard.search_results.clear();
        }

        tracing::debug!("[Oracle] Cleared search results cache after indexing: {}", file_path);
        Ok(())
    }

    /// Invalidate caches after file deletion (Phase 5 Step 8)
    fn invalidate_caches_after_delete(state: SharedState, file_path: String) -> Result<()> {
        // Clear all search results caches since the index has changed
        {
            let mut state_guard = state.write()
                .map_err(|_| MagicError::State("Poisoned lock".into()))?;
            state_guard.search_results.clear();
        }

        tracing::debug!("[Oracle] Cleared search results cache after deletion: {}", file_path);
        Ok(())
    }

    /// Handle file deletion: remove from vec_index and file_registry (Phase 5)
    async fn handle_file_delete(state: SharedState, file_path: String) -> Result<()> {
        tracing::info!("[Oracle] Handling file deletion: {}", file_path);

        // Step 1: Get file info before deletion
        let file_record = crate::storage::get_file_by_path(&state, &file_path)?;

        if let Some(file_record) = file_record {
            // Step 2: Delete from vec_index
            crate::storage::delete_embedding(&state, file_record.file_id)?;

            // Step 3: Delete from file_registry (already done by Librarian, but double-check)
            crate::storage::delete_file(&state, &file_path)?;
        } else {
            tracing::warn!("[Oracle] File not found in registry, skipping deletion: {}", file_path);
        }

        // Step 4: Invalidate caches
        Oracle::invalidate_caches_after_delete(state.clone(), file_path.clone())?;

        tracing::info!("[Oracle] Successfully handled file deletion: {}", file_path);
        Ok(())
    }

    /// Generate embedding for arbitrary content (helper for index_file)
    async fn generate_embedding_for_content(state: SharedState, content: String) -> Result<Vec<f32>> {
        let state_for_embedding = state.clone();

        // Generate embedding in a blocking task
        let query_embedding: Vec<f32> = tokio::task::spawn_blocking(move || {
            let state_guard = state_for_embedding.read()
                .map_err(|_| MagicError::State("Poisoned lock".into()))?;
            let mut model_lock = state_guard.embedding_model.lock()
                .map_err(|_| MagicError::State("Poisoned lock".into()))?;
            let model_opt = model_lock.take()
                .ok_or_else(|| MagicError::Embedding("Model not initialized".into()))?;

            let mut model = model_opt;
            let embedding_vec = model.embed(vec![content.as_str()], None)
                .map_err(|e| MagicError::Embedding(format!("Failed to embed content: {}", e)))?;

            // Put the model back
            *model_lock = Some(model);

            Ok::<Vec<f32>, MagicError>(embedding_vec[0].clone())
        }).await.map_err(|_| MagicError::Other(anyhow::anyhow!("Embed task panic")))??;

        Ok(query_embedding)
    }
}

fn perform_sqlite_vector_search(db_conn: &Connection, query_embedding: &[f32]) -> Result<Vec<SearchResult>> {
    // Convert embedding to bytes for sqlite-vec
    let embedding_bytes: Vec<u8> = bytemuck::cast_slice(query_embedding).to_vec();

    // Use MATCH clause for vector similarity search with sqlite-vec
    let query_sql = "
        SELECT
            fr.file_id,
            fr.abs_path,
            distance as score
        FROM vec_index v
        JOIN file_registry fr ON v.file_id = fr.file_id
        WHERE v.embedding MATCH ?
        LIMIT 10";

    let mut stmt = db_conn.prepare(query_sql)?;

    let results = stmt.query_map(params![embedding_bytes], |row| {
        let abs_path: String = row.get("abs_path")?;
        let score: f32 = row.get("score")?;

        // Extract filename from path
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
    })?.filter_map(|row| row.ok())
     .collect::<Vec<_>>();

    Ok(results)
}

impl Oracle {
    /// Stop the Oracle task
    pub async fn stop(&mut self) -> Result<()> {
        if let Some(handle) = self.task_handle.take() {
            handle.abort();
            let _ = handle.await;
            tracing::info!("[Oracle] Stopped");
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