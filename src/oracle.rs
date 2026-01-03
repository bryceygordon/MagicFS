// FILE: src/oracle.rs
use crate::state::{SharedState, EmbeddingRequest};
use crate::error::{Result, MagicError};
use crate::engine::indexer::Indexer;
use crate::engine::searcher::Searcher;

use tokio::task::JoinHandle;
use tokio::sync::{mpsc, Semaphore};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use fastembed::{TextEmbedding, InitOptions, EmbeddingModel};
use std::time::Duration;
use lru::LruCache;
use std::num::NonZeroUsize;
use std::collections::HashSet;

// Hardening: Concurrency Limits
const MAX_CONCURRENT_INDEXERS: usize = 2;
const MAX_CONCURRENT_SEARCHERS: usize = 2;

// Performance: Maximum number of pending search queries to debounce per tick.
// Prevents O(N^2) CPU explosion during flood attacks.
const MAX_SEARCH_BATCH_SIZE: usize = 100; 

/// Helper: Checks if two strings are "related" (one is a prefix of the other).
/// Returns true if `a` starts with `b` OR `b` starts with `a`.
fn are_related_queries(a: &str, b: &str) -> bool {
    a.starts_with(b) || b.starts_with(a)
}

pub struct Oracle {
    pub state: SharedState,
    pub task_handle: Option<JoinHandle<()>>,
}

impl Oracle {
    pub fn new(state: SharedState) -> Result<Self> {
        Ok(Self {
            state,
            task_handle: None,
        })
    }

    pub fn start(&mut self) -> Result<()> {
        self.start_embedding_actor()?;
        let state = Arc::clone(&self.state);
        
        let handle = tokio::spawn(async move {
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
            let state_guard = state.write().map_err(|_| MagicError::State("Poisoned lock".into())).unwrap();
            *state_guard.embedding_tx.write().unwrap() = Some(tx);
        }

        std::thread::spawn(move || {
            // --- UPGRADE: Nomic Embed v1.5 (768 dims) ---
            tracing::info!("[EmbeddingActor] Starting Nomic Embed v1.5 (SOTA)...");

            let model_result = TextEmbedding::try_new(InitOptions::new(EmbeddingModel::NomicEmbedTextV15));

            let mut model = match model_result {
                Ok(m) => {
                    tracing::info!("[EmbeddingActor] Nomic v1.5 loaded successfully");
                    m
                },
                Err(e) => {
                    tracing::error!("[EmbeddingActor] Failed to load model: {}", e);
                    return;
                }
            };

            while let Some(request) = rx.blocking_recv() {
                let EmbeddingRequest { content, is_query, respond_to } = request;

                let mut final_content = Vec::with_capacity(content.len());
                for text in content {
                    if is_query {
                        final_content.push(format!("search_query: {}", text));
                    } else {
                        final_content.push(format!("search_document: {}", text));
                    }
                }

                let result = model.embed(final_content, None)
                    .map_err(|e| MagicError::Embedding(format!("FastEmbed error: {}", e)));

                if let Err(_) = respond_to.send(result) {
                    tracing::warn!("[EmbeddingActor] Receiver dropped!");
                }
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

        let mut processed_queries: LruCache<String, ()> = LruCache::new(NonZeroUsize::new(1000).unwrap());
        let mut last_index_version = 0;
        let mut active_jobs: HashSet<String> = HashSet::new();
        let (completion_tx, mut completion_rx) = mpsc::channel::<String>(100);
        let index_semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_INDEXERS));
        let search_semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_SEARCHERS));

        loop {
            tokio::time::sleep(Duration::from_millis(50)).await;

            // 1. Check for Index Changes
            let current_index_version = {
                let state_guard = state.read().unwrap();
                state_guard.index_version.load(Ordering::Relaxed)
            };

            if current_index_version != last_index_version {
                tracing::debug!("[Oracle] Index version changed. Resetting processed_queries.");
                processed_queries.clear();
                last_index_version = current_index_version;
            }

            // 2. THE RADIO CHECK (Update The Ledger)
            while let Ok(finished_path) = completion_rx.try_recv() {
                if finished_path.contains("safe.txt") {
                    tracing::info!("[Oracle] Radio received: Worker finished '{}'. Removing lock.", finished_path);
                }
                active_jobs.remove(&finished_path);
            }

            // =========================================================
            // PRIORITY 1: INDEXING (Lockout/Tagout System)
            // =========================================================
            let mut unprocessed_files: Vec<String> = Vec::new();
            let mut tick_locked_files: HashSet<String> = HashSet::new();

            let files_to_process: Vec<String> = {
                let state_guard = state.read().map_err(|_| MagicError::State("Poisoned lock".into())).unwrap();
                let mut files_to_index_lock = match state_guard.files_to_index.lock() {
                    Ok(guard) => guard,
                    Err(poisoned) => poisoned.into_inner(),
                };
                files_to_index_lock.drain(..).collect()
            };

            for file_path in files_to_process {
                let (canonical_path, is_delete) = if file_path.starts_with("DELETE:") {
                    (file_path.trim_start_matches("DELETE:").to_string(), true)
                } else {
                    (file_path.clone(), false)
                };
                
                if active_jobs.contains(&canonical_path) || tick_locked_files.contains(&canonical_path) {
                    if canonical_path.contains("safe.txt") {
                        tracing::info!("[Oracle] Lockout active for '{}'. Re-queueing ticket.", canonical_path);
                    }
                    unprocessed_files.push(file_path);
                    continue;
                }
                
                if canonical_path.contains("safe.txt") {
                    tracing::info!("[Oracle] Locking '{}' for processing.", canonical_path);
                }
                active_jobs.insert(canonical_path.clone());
                tick_locked_files.insert(canonical_path.clone());
                
                let state_ref = Arc::clone(&state);
                let tx = completion_tx.clone();
                let path_for_radio = canonical_path.clone();

                if is_delete {
                    tokio::spawn(async move {
                        if let Err(e) = Indexer::remove_file(state_ref, path_for_radio.clone()).await {
                            tracing::error!("[Oracle] File removal failed: {}", e);
                        }
                        let _ = tx.send(path_for_radio).await;
                    });
                } else {
                    match index_semaphore.clone().try_acquire_owned() {
                        Ok(permit) => {
                             tokio::spawn(async move {
                                let _permit = permit; 
                                if let Err(e) = Indexer::index_file(state_ref, file_path).await {
                                    tracing::error!("[Oracle] Indexing failed: {}", e);
                                }
                                let _ = tx.send(path_for_radio).await;
                            });
                        },
                        Err(_) => {
                            active_jobs.remove(&canonical_path); 
                            unprocessed_files.push(file_path);
                        }
                    }
                }
            }

            if !unprocessed_files.is_empty() {
                 let files_to_index_arc = {
                    let state_guard = state.read().unwrap();
                    state_guard.files_to_index.clone()
                };
                let mut lock = files_to_index_arc.lock().unwrap_or_else(|e| e.into_inner());
                let new_items = std::mem::take(&mut *lock);
                *lock = unprocessed_files;
                lock.extend(new_items);
            }

            // =========================================================
            // PRIORITY 2: SEARCH DEBOUNCING & SMART WAITER NOTIFICATION
            // =========================================================

            // A. Accumulation
            let has_work = {
                let state_guard = state.read().unwrap();
                !state_guard.inode_store.active_queries().is_empty()
            };

            if has_work {
                // The "Accumulation Window"
                tokio::time::sleep(Duration::from_millis(20)).await;
            }

            // B. Snapshot & Filter
            let mut candidates: Vec<(u64, String)> = {
                let state_guard = state.read().unwrap();
                let mut list = state_guard.inode_store.active_queries();
                
                // FILTER: Ignore queries we have already processed/cached results for
                list.retain(|(inode, query)| {
                    !processed_queries.contains(query) && !state_guard.inode_store.has_results(*inode)
                });
                
                list
            };

            if candidates.is_empty() {
                continue;
            }

            // C. SORT BY TIME (Newest First)
            candidates.sort_by(|a, b| b.0.cmp(&a.0));

            // D. BATCH LIMITING
            if candidates.len() > MAX_SEARCH_BATCH_SIZE {
                candidates.truncate(MAX_SEARCH_BATCH_SIZE);
            }

            // E. The Sieve (Batch-Level Debouncing)
            let mut accepted_queries: Vec<(u64, String)> = Vec::new();

            for (inode, query) in candidates {
                let is_obsolete = accepted_queries.iter().any(|(_, accepted_q)| {
                    are_related_queries(&query, accepted_q)
                });

                if is_obsolete {
                    tracing::debug!("[Oracle] 👻 Ghost Busting (Sieve): Pruning obsolete intent '{}' (ID: {})", query, inode);
                    {
                        let state_guard = state.read().unwrap();
                        state_guard.inode_store.prune_inode(inode);

                        // --- NEW: NOTIFY WAITER ON PRUNE ---
                        // If we prune a query because it's obsolete (e.g., user typed "magic" then "magicfs"),
                        // and there is a thread waiting on "magic", we must wake it up so it doesn't timeout.
                        let mut waiters = state_guard.search_waiters.lock().unwrap();
                        if let Some(waiter) = waiters.remove(&inode) {
                            let mut finished = waiter.finished.lock().unwrap();
                            *finished = true; // Mark as done (even though empty/pruned)
                            waiter.cvar.notify_all();
                        }
                        // -----------------------------------
                    }
                } else {
                    accepted_queries.push((inode, query));
                }
            }

            // F. Dispatch
            for (inode, query) in accepted_queries {
                // Limit concurrency
                let permit = match search_semaphore.clone().try_acquire_owned() {
                    Ok(p) => p,
                    Err(_) => break, // Searcher saturated, try next tick
                };

                // --- RETROACTIVE CLEANUP (Highlander Mode) ---
                {
                    let inode_store = state.read().unwrap().inode_store.clone();
                    
                    let victims: Vec<u64> = inode_store.active_queries()
                        .iter()
                        .filter(|(other_id, other_q)| {
                            *other_id != inode && are_related_queries(&query, other_q)
                        })
                        .map(|(id, _)| *id)
                        .collect();

                    if !victims.is_empty() {
                         tracing::info!("[Oracle] 🧹 Cleaning up {} stale relatives for '{}'", victims.len(), query);
                         for vid in victims {
                             inode_store.prune_inode(vid);
                         }
                    }
                }

                tracing::info!("[Oracle] Dispatching search for: '{}'", query);
                processed_queries.put(query.clone(), ());

                let state_ref = Arc::clone(&state);
                tokio::spawn(async move {
                    let _permit = permit;
                    if let Err(e) = Searcher::perform_search(state_ref.clone(), query.clone(), inode).await {
                        tracing::error!("[Oracle] Search failed for '{}': {}", query, e);
                    }

                    // --- NEW: NOTIFY WAITER ---
                    // "Ring the Bell"
                    // The Searcher has finished writing results to the InodeStore.
                    // We now wake up the FUSE thread waiting in readdir.
                    let state_guard = state_ref.read().unwrap();
                    let mut waiters = state_guard.search_waiters.lock().unwrap();
                    
                    if let Some(waiter) = waiters.remove(&inode) {
                        let mut finished = waiter.finished.lock().unwrap();
                        *finished = true;
                        waiter.cvar.notify_all();
                        tracing::debug!("[Oracle] Notified waiter for Inode {}", inode);
                    }
                    // --------------------------
                });
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
