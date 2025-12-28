//! Librarian: The Background Watcher
//!
//! Watches physical directories, updates SQLite Index,
//! ensures VFS consistency.
//!
//! Runs as a background thread (not async).
//!
//! CRITICAL RULE: Never blocks FUSE loop or Oracle.
//! Completely isolated from Hollow Drive.

use crate::state::SharedState;
use crate::error::Result;
use notify::{RecommendedWatcher, Watcher, Event, EventKind, RecursiveMode};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};
use std::collections::HashMap;

/// The Librarian: background watcher thread
pub struct Librarian {
    /// Shared state for coordination
    pub state: SharedState,

    /// Paths being watched
    pub watch_paths: Arc<Mutex<Vec<String>>>,

    /// Handle to the watcher thread
    pub thread_handle: Option<thread::JoinHandle<()>>,
}

impl Librarian {
    /// Create a new Librarian instance
    pub fn new(state: SharedState) -> Self {
        Self {
            state,
            watch_paths: Arc::new(Mutex::new(Vec::new())),
            thread_handle: None,
        }
    }

    /// Check if a path should be ignored (dotfiles, hidden directories)
    fn is_ignored_path(path: &std::path::Path) -> bool {
        path.components().any(|component| {
            component.as_os_str().to_string_lossy().starts_with('.')
        })
    }

    /// Start the watcher thread
    pub fn start(&mut self) -> Result<()> {
        let watch_paths = Arc::clone(&self.watch_paths);
        let state = Arc::clone(&self.state);

        let handle = thread::spawn(move || {
            Self::watcher_loop(watch_paths, state);
        });

        self.thread_handle = Some(handle);
        tracing::info!("[Librarian] Started watcher thread");

        Ok(())
    }

    /// Add a path to the watch list
    pub fn add_watch_path(&self, path: String) -> Result<()> {
        let mut paths = self.watch_paths.lock().map_err(|_| crate::error::MagicError::State("Poisoned lock".into()))?;
        paths.push(path);
        tracing::info!("[Librarian] Added watch path");
        Ok(())
    }

    /// Main watcher loop (runs on dedicated thread)
    fn watcher_loop(watch_paths: Arc<Mutex<Vec<String>>>, state: SharedState) {
        tracing::info!("[Librarian] Watcher loop started");

        // Perform initial scan of existing files before setting up watcher
        // This ensures files that already exist are indexed
        {
            let paths = match watch_paths.lock() {
                Ok(guard) => guard.clone(),
                Err(_) => {
                    tracing::error!("[Librarian] Failed to lock watch_paths");
                    Vec::new()
                }
            };

            tracing::info!("[Librarian] Starting initial file scan...");
            for path in paths {
                if let Err(e) = Self::scan_directory_for_files(&state, std::path::Path::new(&path)) {
                    tracing::error!("[Librarian] Error scanning directory {}: {}", path, e);
                } else {
                    tracing::info!("[Librarian] Initial scan complete for: {}", path);
                }
            }
        }

        // Phase 5 Step 1: Initialize notify watcher
        let (tx, rx) = mpsc::channel();

        // Create a recommended watcher (platform-specific implementation)
        let mut watcher = match RecommendedWatcher::new(tx, notify::Config::default()) {
            Ok(w) => w,
            Err(e) => {
                tracing::error!("[Librarian] Failed to create watcher: {}", e);
                return;
            }
        };

        // Add all watch paths
        {
            let paths = match watch_paths.lock() {
                Ok(guard) => guard.clone(),
                Err(_) => {
                    tracing::error!("[Librarian] Failed to lock watch_paths");
                    Vec::new()
                }
            };

            for path in paths {
                if let Err(e) = watcher.watch(std::path::Path::new(&path), RecursiveMode::Recursive) {
                    tracing::error!("[Librarian] Failed to watch path {}: {}", path, e);
                } else {
                    tracing::info!("[Librarian] Watching path: {}", path);
                }
            }
        }

        // Phase 5 Step 2: Debouncing for file events
        // Wait for 500ms of quiet time before processing events
        let debounce_duration = Duration::from_millis(500);
        let mut event_queue: HashMap<std::path::PathBuf, Event> = HashMap::new();
        let mut last_activity = Instant::now();

        // Main event loop
        loop {
            // Receive file system events with timeout
            match rx.recv_timeout(Duration::from_millis(50)) {
                Ok(event) => {
                    // Collect events for debouncing
                    if let Ok(event) = event {
                        for path in &event.paths {
                            // Store the most recent event for this path
                            event_queue.insert(path.clone(), event.clone());
                        }
                        last_activity = Instant::now();
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    // Check if we've had a quiet period
                    if !event_queue.is_empty() && last_activity.elapsed() >= debounce_duration {
                        // Process all queued events
                        let events_to_process = std::mem::take(&mut event_queue);
                        for (_path, event) in events_to_process {
                            if let Err(e) = Self::handle_file_event(&Ok(event), &state) {
                                tracing::error!("[Librarian] Error handling file event: {}", e);
                            }
                        }
                        event_queue.clear();
                    }
                }
                Err(e) => {
                    tracing::error!("[Librarian] Watcher channel error: {}", e);
                    // Try to recreate watcher on error
                    thread::sleep(Duration::from_millis(1000));
                }
            }
        }
    }

    /// Recursively scan directory for existing files to index
    fn scan_directory_for_files(state: &SharedState, dir_path: &std::path::Path) -> Result<()> {
        tracing::info!("[Librarian] Scanning directory: {}", dir_path.display());

        if !dir_path.exists() {
            tracing::warn!("[Librarian] Directory does not exist: {}", dir_path.display());
            return Ok(());
        }

        if !dir_path.is_dir() {
            tracing::warn!("[Librarian] Path is not a directory: {}", dir_path.display());
            return Ok(());
        }

        // Recursively walk the directory, skipping hidden files and directories
        for entry in walkdir::WalkDir::new(dir_path) {
            match entry {
                Ok(entry) => {
                    let path = entry.path();

                    // Skip directory entries that start with '.' to prevent descending into them
                    if entry.file_type().is_dir() && entry.file_name().to_string_lossy().starts_with('.') {
                        tracing::debug!("[Librarian] Skipping hidden directory: {}", path.display());
                        continue;
                    }

                    // Skip hidden files and directories (e.g., .obsidian, .git, .DS_Store)
                    if entry.file_name().to_string_lossy().starts_with('.') || Self::is_ignored_path(path) {
                        tracing::debug!("[Librarian] Skipping hidden file: {}", path.display());
                        continue;
                    }

                    if path.is_file() {
                        let path_str = path.to_string_lossy().to_string();
                        tracing::debug!("[Librarian] Found file to index: {}", path_str);

                        // Add to indexing queue (same as event handling)
                        let files_to_index = {
                            let state_guard = state.read()
                                .map_err(|_| crate::error::MagicError::State("Poisoned lock".into()))?;
                            Arc::clone(&state_guard.files_to_index)
                        };

                        let mut queue = files_to_index.lock()
                            .map_err(|_| crate::error::MagicError::State("Poisoned lock".into()))?;
                        queue.push(path_str);
                    }
                }
                Err(e) => {
                    tracing::warn!("[Librarian] Error walking directory: {}", e);
                }
            }
        }

        tracing::info!("[Librarian] Directory scan complete: {}", dir_path.display());
        Ok(())
    }

    /// Handle a file system event
    fn handle_file_event(event: &std::result::Result<Event, notify::Error>, state: &SharedState) -> Result<()> {
        match event {
            Ok(event) => {
                tracing::debug!("[Librarian] File event: {:?} - {:?}", event.kind, event.paths);

                // Phase 5 Step 2: Debouncing (will be added later)
                // For now, process each event

                // Handle different event types
                match event.kind {
                    EventKind::Create(_) | EventKind::Modify(_) => {
                        for path in &event.paths {
                            // Skip hidden files and directories (e.g., .obsidian, .git, .DS_Store)
                            if Self::is_ignored_path(path) {
                                tracing::debug!("[Librarian] Ignoring hidden file event: {:?}", path);
                                continue;
                            }

                            if path.is_file() {
                                let path_str = path.to_string_lossy().to_string();
                                tracing::info!("[Librarian] Queuing file for indexing: {}", path_str);

                                // Phase 5 Step 6: Add to indexing queue
                                // Oracle will process this asynchronously
                                let files_to_index = {
                                    let state_guard = state.read()
                                        .map_err(|_| crate::error::MagicError::State("Poisoned lock".into()))?;
                                    Arc::clone(&state_guard.files_to_index)
                                };

                                let mut queue = files_to_index.lock()
                                    .map_err(|_| crate::error::MagicError::State("Poisoned lock".into()))?;
                                queue.push(path_str);
                            }
                        }
                    }
                    EventKind::Remove(_) => {
                        for path in &event.paths {
                            let path_str = path.to_string_lossy().to_string();
                            tracing::info!("[Librarian] Queuing file for deletion: {}", path_str);

                            // CRITICAL FIX: Librarian is "The Hands" (observer), not the executioner.
                            // We must NOT delete from registry here - let Oracle handle it atomically.
                            // This prevents the "Amnesiac Deletion" race condition where:
                            // 1. Librarian deletes from file_registry
                            // 2. Oracle tries to get file_id but gets None
                            // 3. Embedding in vec_index becomes orphaned forever

                            let files_to_index = {
                                let state_guard = state.read()
                                    .map_err(|_| crate::error::MagicError::State("Poisoned lock".into()))?;
                                Arc::clone(&state_guard.files_to_index)
                            };

                            let mut queue = files_to_index.lock()
                                .map_err(|_| crate::error::MagicError::State("Poisoned lock".into()))?;

                            // Push DELETE marker - Oracle will handle atomic cleanup:
                            // 1. Get file_id from file_registry
                            // 2. Delete embedding from vec_index
                            // 3. Delete file from file_registry
                            queue.push(format!("DELETE:{}", path_str));
                        }
                    }
                    _ => {
                        // Other event types (metadata changes, etc.)
                        tracing::debug!("[Librarian] Other event: {:?}", event.kind);
                    }
                }
            }
            Err(e) => {
                tracing::error!("[Librarian] Event error: {}", e);
            }
        }

        Ok(())
    }

    /// Stop the watcher thread
    pub fn stop(&mut self) -> Result<()> {
        if let Some(handle) = self.thread_handle.take() {
            handle.join()
                .map_err(|_| crate::error::MagicError::State("Failed to join watcher thread".into()))?;
            tracing::info!("[Librarian] Stopped");
        }
        Ok(())
    }
}

impl Drop for Librarian {
    fn drop(&mut self) {
        if let Some(handle) = &self.thread_handle {
            handle.thread().unpark();
            // Note: We can't join here without blocking
            // In a real implementation, we'd use a proper shutdown mechanism
        }
    }
}