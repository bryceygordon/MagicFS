//! Librarian: The Background Watcher
//!
//! Watches physical directories, updates SQLite Index,
//! ensures VFS consistency.

use crate::state::SharedState;
use crate::error::Result;
use notify::{RecommendedWatcher, Watcher, Event, EventKind, RecursiveMode};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Helper struct to manage ignore rules
struct IgnoreManager {
    rules: Vec<String>,
}

impl IgnoreManager {
    fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Load rules from .magicfsignore in the given root directory
    fn load_from_dir(&mut self, root: &Path) {
        let ignore_file = root.join(".magicfsignore");
        
        tracing::info!("[Librarian] Looking for ignore file at: {}", ignore_file.display());

        if ignore_file.exists() {
            match fs::read_to_string(&ignore_file) {
                Ok(content) => {
                    tracing::info!("[Librarian] Found .magicfsignore. Parsing rules...");
                    for line in content.lines() {
                        let rule = line.trim();
                        // Basic rule parsing: ignore empty lines and comments
                        if !rule.is_empty() && !rule.starts_with('#') {
                            self.rules.push(rule.to_string());
                            tracing::info!("[Librarian] + Added ignore rule: '{}'", rule);
                        }
                    }
                },
                Err(e) => tracing::error!("[Librarian] Failed to read .magicfsignore: {}", e),
            }
        } else {
            tracing::info!("[Librarian] No .magicfsignore file found.");
        }
    }

    /// Check if a path should be ignored based on loaded rules
    fn is_ignored(&self, path: &Path) -> bool {
        // Always ignore the ignore file itself
        if path.file_name().map_or(false, |n| n == ".magicfsignore") {
            return true;
        }
        
        // Check if any component of the path matches a rule
        for component in path.components() {
            let comp_str = component.as_os_str().to_string_lossy();
            for rule in &self.rules {
                if comp_str == *rule {
                    tracing::info!("[Librarian] IGNORED '{}' because it matches rule '{}'", path.display(), rule);
                    return true;
                }
            }
        }
        
        false
    }
}

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
        
        // FIX: Clone the path here so we can still use it for logging below
        paths.push(path.clone());
        
        tracing::info!("[Librarian] Added watch path: {}", path);
        Ok(())
    }

    /// Main watcher loop (runs on dedicated thread)
    fn watcher_loop(watch_paths: Arc<Mutex<Vec<String>>>, state: SharedState) {
        tracing::info!("[Librarian] Watcher loop started");

        // Initialize Ignore Manager
        let mut ignore_manager = IgnoreManager::new();
        
        // Load ignore rules and perform initial scan
        {
            let paths = match watch_paths.lock() {
                Ok(guard) => guard.clone(),
                Err(_) => Vec::new(),
            };

            for path_str in &paths {
                let path = Path::new(path_str);
                
                // 1. Load rules for this watch root FIRST
                ignore_manager.load_from_dir(path);
                
                // 2. Scan using those rules
                if let Err(e) = Self::scan_directory_for_files(&state, path, &ignore_manager) {
                    tracing::error!("[Librarian] Error scanning directory {}: {}", path_str, e);
                } else {
                    tracing::info!("[Librarian] Initial scan complete for: {}", path_str);
                }
            }
        }

        // Initialize notify watcher
        let (tx, rx) = mpsc::channel();
        let mut watcher = match RecommendedWatcher::new(tx, notify::Config::default()) {
            Ok(w) => w,
            Err(e) => {
                tracing::error!("[Librarian] Failed to create watcher: {}", e);
                return;
            }
        };

        // Add watches
        {
            let paths = match watch_paths.lock() {
                Ok(guard) => guard.clone(),
                Err(_) => Vec::new(),
            };
            for path in paths {
                if let Err(e) = watcher.watch(Path::new(&path), RecursiveMode::Recursive) {
                    tracing::error!("[Librarian] Failed to watch path {}: {}", path, e);
                }
            }
        }

        // Debounce setup
        let debounce_duration = Duration::from_millis(500);
        let mut event_queue: HashMap<PathBuf, Event> = HashMap::new();
        let mut last_activity = Instant::now();

        // Main event loop
        loop {
            match rx.recv_timeout(Duration::from_millis(50)) {
                Ok(event) => {
                    if let Ok(event) = event {
                        for path in &event.paths {
                            event_queue.insert(path.clone(), event.clone());
                        }
                        last_activity = Instant::now();
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if !event_queue.is_empty() && last_activity.elapsed() >= debounce_duration {
                        let events_to_process = std::mem::take(&mut event_queue);
                        for (_path, event) in events_to_process {
                            // Pass ignore_manager to event handler
                            if let Err(e) = Self::handle_file_event(&Ok(event), &state, &ignore_manager) {
                                tracing::error!("[Librarian] Error handling file event: {}", e);
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("[Librarian] Watcher channel error: {}", e);
                    thread::sleep(Duration::from_millis(1000));
                }
            }
        }
    }

    /// Recursively scan directory with ignore support
    fn scan_directory_for_files(state: &SharedState, dir_path: &Path, ignore_manager: &IgnoreManager) -> Result<()> {
        if !dir_path.exists() || !dir_path.is_dir() {
            return Ok(());
        }

        // Use filter_entry to prevent descending into ignored directories
        let walker = walkdir::WalkDir::new(dir_path).into_iter();
        let filtered_walker = walker.filter_entry(|e| !ignore_manager.is_ignored(e.path()));

        for entry in filtered_walker {
            match entry {
                Ok(entry) => {
                    let path = entry.path();
                    
                    if path.is_file() {
                        let path_str = path.to_string_lossy().to_string();
                        // Add to indexing queue
                        let files_to_index = {
                            let state_guard = state.read().map_err(|_| crate::error::MagicError::State("Poisoned lock".into()))?;
                            Arc::clone(&state_guard.files_to_index)
                        };
                        let mut queue = files_to_index.lock().map_err(|_| crate::error::MagicError::State("Poisoned lock".into()))?;
                        queue.push(path_str);
                    }
                }
                Err(e) => tracing::warn!("[Librarian] Error walking directory: {}", e),
            }
        }
        Ok(())
    }

    /// Handle file system event with ignore support
    fn handle_file_event(event: &std::result::Result<Event, notify::Error>, state: &SharedState, ignore_manager: &IgnoreManager) -> Result<()> {
        match event {
            Ok(event) => {
                match event.kind {
                    EventKind::Create(_) | EventKind::Modify(_) => {
                        for path in &event.paths {
                            
                            // CHECK IGNORE RULES
                            if ignore_manager.is_ignored(path) {
                                tracing::debug!("[Librarian] Ignoring event for: {:?}", path);
                                continue;
                            }

                            if path.is_file() {
                                let path_str = path.to_string_lossy().to_string();
                                let files_to_index = {
                                    let state_guard = state.read().map_err(|_| crate::error::MagicError::State("Poisoned lock".into()))?;
                                    Arc::clone(&state_guard.files_to_index)
                                };
                                let mut queue = files_to_index.lock().map_err(|_| crate::error::MagicError::State("Poisoned lock".into()))?;
                                queue.push(path_str);
                            }
                        }
                    }
                    EventKind::Remove(_) => {
                        for path in &event.paths {
                            // Check ignore rules for delete events too
                            if ignore_manager.is_ignored(path) {
                                continue;
                            }
                            
                            let path_str = path.to_string_lossy().to_string();
                            let files_to_index = {
                                let state_guard = state.read().map_err(|_| crate::error::MagicError::State("Poisoned lock".into()))?;
                                Arc::clone(&state_guard.files_to_index)
                            };
                            let mut queue = files_to_index.lock().map_err(|_| crate::error::MagicError::State("Poisoned lock".into()))?;
                            queue.push(format!("DELETE:{}", path_str));
                        }
                    }
                    _ => {}
                }
            }
            Err(e) => tracing::error!("[Librarian] Event error: {}", e),
        }
        Ok(())
    }

    /// Stop the watcher thread
    pub fn stop(&mut self) -> Result<()> {
        if let Some(handle) = self.thread_handle.take() {
            handle.join().map_err(|_| crate::error::MagicError::State("Failed to join watcher thread".into()))?;
            tracing::info!("[Librarian] Stopped");
        }
        Ok(())
    }
}

impl Drop for Librarian {
    fn drop(&mut self) {
        if let Some(handle) = &self.thread_handle {
            handle.thread().unpark();
        }
    }
}
