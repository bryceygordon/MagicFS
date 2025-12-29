//! Oracle: The Async Brain
//!
//! Handles Vector Search (fastembed-rs) and SQLite (sqlite-vec).
//! Populates the Memory Cache for the Hollow Drive.
//!
//! HARDENING:
//! - Implements 1-to-Many Chunking logic (Index Loop).
//! - Implements Score Aggregation (Max Chunk Score).

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
            Oracle::run_task(state).await;
        });
        self.task_handle = Some(handle);
        tracing::info!("[Oracle] Started async task");
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

    async fn run_task(state: SharedState) {
        tracing::info!("[Oracle] Async task started");

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

        loop {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;

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

            let files_to_process: Vec<String> = {
                let state_guard = state.read().map_err(|_| MagicError::State("Poisoned lock".into())).unwrap();
                let mut files_to_index_lock = state_guard.files_to_index.lock().unwrap_or_else(|e| e.into_inner());
                files_to_index_lock.drain(..)
                    .filter(|file| !processed_files.contains(file))
                    .collect()
            };

            for (query, _inode_num) in queries_to_process {
                let state_to_process = Arc::clone(&state);
                processed_queries.insert(query.clone());
                tokio::spawn(async move {
                    if let Err(e) = Oracle::process_search_query(state_to_process, query).await {
                        tracing::error!("[Oracle] Error processing search: {}", e);
                    }
                });
            }

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
                    tokio::spawn(async move {
                        if let Err(e) = Oracle::index_file(state_to_process, file_path).await {
                            tracing::error!("[Oracle] Error indexing file: {}", e);
                        }
                    });
                }
            }
        }
    }

    async fn process_search_query(state: SharedState, query: String) -> Result<()> {
        let results = Oracle::perform_vector_search(state.clone(), query.clone()).await?;

        let inode_num = {
            let state_guard = state.read().map_err(|_| MagicError::State("Poisoned lock".into()))?;
            state_guard.active_searches.get(&query)
                .map(|v| *v.value())
                .ok_or_else(|| MagicError::State("Query not found".into()))?
        };

        {
            let state_guard = state.write().map_err(|_| MagicError::State("Poisoned lock".into()))?;
            state_guard.search_results.insert(inode_num, results);
        }
        Ok(())
    }

    async fn perform_vector_search(state: SharedState, query: String) -> Result<Vec<SearchResult>> {
        let query_embedding = Oracle::request_embedding(&state, query.clone()).await?;

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

    async fn request_embedding(state: &SharedState, content: String) -> Result<Vec<f32>> {
        let tx = {
            let state_guard = state.read().map_err(|_| MagicError::State("Poisoned lock".into()))?;
            let tx_guard = state_guard.embedding_tx.read().unwrap();
            tx_guard.clone().ok_or_else(|| MagicError::Embedding("Actor not ready".into()))?
        };
        let (resp_tx, resp_rx) = oneshot::channel();
        let req = EmbeddingRequest { content, respond_to: resp_tx };
        tx.send(req).await.map_err(|_| MagicError::Embedding("Actor channel closed".into()))?;
        resp_rx.await.map_err(|_| MagicError::Embedding("Actor dropped request".into()))?
    }

    pub async fn stop(&mut self) -> Result<()> {
        if let Some(handle) = self.task_handle.take() {
            handle.abort();
            let _ = handle.await;
        }
        Ok(())
    }

    async fn index_file(state: SharedState, file_path: String) -> Result<()> {
        tracing::info!("[Oracle] Indexing file: {}", file_path);
        
        let mut text_content = String::new();
        let max_retries = 5;
        let mut success = false;

        for attempt in 1..=max_retries {
            let path_for_task = file_path.clone();
            let file_size = match std::fs::metadata(&file_path) {
                Ok(m) => m.len(),
                Err(_) => 0,
            };

            let extraction_result = tokio::task::spawn_blocking(move || {
                crate::storage::extract_text_from_file(std::path::Path::new(&path_for_task))
            }).await.map_err(|_| MagicError::Other(anyhow::anyhow!("Text extraction task panic")))?;

            match extraction_result {
                Ok(content) => {
                    if !content.trim().is_empty() {
                        text_content = content;
                        success = true;
                        break; 
                    }
                    if file_size == 0 { return Ok(()); } 
                    
                    if attempt < max_retries {
                        tokio::time::sleep(Duration::from_millis(50)).await;
                        continue;
                    }
                },
                Err(_) => {
                    if attempt < max_retries {
                        tokio::time::sleep(Duration::from_millis(50)).await;
                        continue;
                    }
                    return Ok(()); 
                }
            }
        }

        if !success || text_content.trim().is_empty() {
            return Ok(());
        }

        // --- NEW LOGIC: Chunking ---
        let chunks = crate::storage::text_extraction::chunk_text(&text_content);
        if chunks.is_empty() { return Ok(()); }

        tracing::debug!("[Oracle] File {} split into {} chunks", file_path, chunks.len());

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

        crate::storage::vec_index::delete_embeddings_for_file(&state, file_id)?;

        for chunk in chunks {
            let embedding = Oracle::request_embedding(&state, chunk).await?;
            if let Err(e) = crate::storage::insert_embedding(&state, file_id, &embedding) {
                tracing::warn!("[Oracle] Failed to insert chunk: {}", e);
            }
        }

        Oracle::invalidate_caches_after_index(state.clone(), file_path.clone())?;
        tracing::info!("[Oracle] Indexed {} ({} chunks)", file_path, file_id);
        Ok(())
    }

    fn invalidate_caches_after_index(state: SharedState, _file_path: String) -> Result<()> {
        let state_guard = state.write().map_err(|_| MagicError::State("Poisoned lock".into()))?;
        state_guard.search_results.clear();
        Ok(())
    }

    fn invalidate_caches_after_delete(state: SharedState, _file_path: String) -> Result<()> {
        let state_guard = state.write().map_err(|_| MagicError::State("Poisoned lock".into()))?;
        state_guard.search_results.clear();
        Ok(())
    }

    async fn handle_file_delete(state: SharedState, file_path: String) -> Result<()> {
        let file_record = crate::storage::get_file_by_path(&state, &file_path)?;
        if let Some(file_record) = file_record {
            crate::storage::vec_index::delete_embeddings_for_file(&state, file_record.file_id)?;
            crate::storage::delete_file(&state, &file_path)?;
        }
        Oracle::invalidate_caches_after_delete(state.clone(), file_path)?;
        Ok(())
    }
}

// FIX: Added debugging to inspect DB state
fn perform_sqlite_vector_search(db_conn: &Connection, query_embedding: &[f32]) -> Result<Vec<SearchResult>> {
    let embedding_bytes: Vec<u8> = bytemuck::cast_slice(query_embedding).to_vec();

    // DEBUG: Check total chunks in DB
    let count: i64 = db_conn.query_row("SELECT count(*) FROM vec_index", [], |r| r.get(0)).unwrap_or(-1);
    tracing::debug!("[Oracle] Total chunks in vec_index before search: {}", count);
    
    // Aggregation Query
    let query_sql = "
        SELECT 
            fr.file_id, 
            fr.abs_path, 
            MIN(v.distance) as best_distance
        FROM (
            SELECT file_id, distance 
            FROM vec_index 
            WHERE embedding MATCH ?
            ORDER BY distance ASC
            LIMIT 100
        ) v
        JOIN file_registry fr ON v.file_id = fr.file_id
        GROUP BY fr.file_id
        ORDER BY best_distance ASC
        LIMIT 20";

    let mut stmt = db_conn.prepare(query_sql)?;
    let results = stmt.query_map(params![embedding_bytes], |row| {
        let abs_path: String = row.get("abs_path")?;
        let distance: f32 = row.get("best_distance")?;
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

    tracing::debug!("[Oracle] Search returned {} aggregated results", results.len());

    Ok(results)
}

impl Drop for Oracle {
    fn drop(&mut self) {
        if let Some(handle) = &self.task_handle {
            handle.abort();
        }
    }
}
