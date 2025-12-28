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
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

/// Manages ignore rules (like .gitignore)
struct IgnoreManager {
    rules: HashMap<PathBuf, HashSet<String>>,
}

impl IgnoreManager {
    fn new() -> Self {
        Self { rules: HashMap::new() }
    }

    fn load_rules_for_root(&mut self, root: &Path) {
        let ignore_file = root.join(".magicfsignore");
        let mut new_rules = HashSet::new();

        new_rules.insert(".magicfsignore".to_string());
        new_rules.insert(".magicfs".to_string());

        if ignore_file.exists() {
            tracing::info!("[Librarian] Loading ignore rules from: {}", ignore_file.display());
            match fs::read_to_string(&ignore_file) {
                Ok(content) => {
                    for line in content.lines() {
                        let rule = line.trim();
                        if !rule.is_empty() && !rule.starts_with('#') {
                            new_rules.insert(rule.to_string());
                        }
                    }
                },
                Err(e) => tracing::error!("[Librarian] Failed to read .magicfsignore: {}", e),
            }
        }
        self.rules.insert(root.to_path_buf(), new_rules);
    }

    fn is_ignored(&self, abs_path: &Path, watch_roots: &[String]) -> bool {
        let root = watch_roots.iter()
            .map(Path::new)
            .find(|root| abs_path.starts_with(root));

        let root = match root {
            Some(r) => r,
            None => return false,
        };

        if let Some(rules) = self.rules.get(root) {
            if let Ok(relative) = abs_path.strip_prefix(root) {
                for component in relative.components() {
                    let comp_str = component.as_os_str().to_string_lossy();
                    if rules.contains(comp_str.as_ref()) {
                        tracing::debug!("[Librarian] IGNORED '{}' (matched rule '{}')", abs_path.display(), comp_str);
                        return true;
                    }
                }
            }
        }
        false
    }
}

pub struct Librarian {
    pub state: SharedState,
    pub watch_paths: Arc<Mutex<Vec<String>>>,
    pub thread_handle: Option<thread::JoinHandle<()>>,
}

impl Librarian {
    pub fn new(state: SharedState) -> Self {
        Self {
            state,
            watch_paths: Arc::new(Mutex::new(Vec::new())),
            thread_handle: None,
        }
    }

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

    pub fn add_watch_path(&self, path: String) -> Result<()> {
        let mut paths = self.watch_paths.lock().map_err(|_| crate::error::MagicError::State("Poisoned lock".into()))?;
        
        // REVERTED: Do not canonicalize. Watch exactly what the user provided.
        paths.push(path.clone());
        tracing::info!("[Librarian] Added watch path: {}", path);
        Ok(())
    }

    fn watcher_loop(watch_paths: Arc<Mutex<Vec<String>>>, state: SharedState) {
        tracing::info!("[Librarian] Watcher loop started");

        let paths_vec = match watch_paths.lock() {
            Ok(guard) => guard.clone(),
            Err(_) => Vec::new(),
        };

        let mut ignore_manager = IgnoreManager::new();
        for path_str in &paths_vec {
            ignore_manager.load_rules_for_root(Path::new(path_str));
        }

        // Initial Scan
        for path_str in &paths_vec {
            let path = Path::new(path_str);
            tracing::info!("[Librarian] Scanning root: {}", path_str);
            if let Err(e) = Self::scan_directory_for_files(&state, path, &ignore_manager, &paths_vec) {
                tracing::error!("[Librarian] Error scanning directory {}: {}", path_str, e);
            }
        }

        let (tx, rx) = mpsc::channel();
        let mut watcher = match RecommendedWatcher::new(tx, notify::Config::default()) {
            Ok(w) => w,
            Err(e) => {
                tracing::error!("[Librarian] Failed to create watcher: {}", e);
                return;
            }
        };

        for path in &paths_vec {
            tracing::info!("[Librarian] Watching: {}", path);
            if let Err(e) = watcher.watch(Path::new(path), RecursiveMode::Recursive) {
                tracing::error!("[Librarian] Failed to watch path {}: {}", path, e);
            }
        }

        let debounce_duration = Duration::from_millis(500);
        let mut event_queue: HashMap<PathBuf, Event> = HashMap::new();
        let mut last_activity = Instant::now();

        loop {
            match rx.recv_timeout(Duration::from_millis(50)) {
                Ok(event_res) => {
                    if let Ok(event) = event_res {
                        // DEBUG LOG: See if notify is even firing
                        tracing::debug!("[Librarian] RAW EVENT: {:?}", event);
                        
                        // FIX: Only debounce events we actually care about.
                        let is_relevant = match event.kind {
                            EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => true,
                            _ => false,
                        };

                        if is_relevant {
                            for path in &event.paths {
                                event_queue.insert(path.clone(), event.clone());
                            }
                            last_activity = Instant::now();
                        }
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if !event_queue.is_empty() && last_activity.elapsed() >= debounce_duration {
                        tracing::debug!("[Librarian] Processing debounced batch of {} events", event_queue.len());
                        let events_to_process = std::mem::take(&mut event_queue);
                        
                        // PASS 1: Update ignore rules FIRST
                        for (path, _) in &events_to_process {
                            if path.file_name().map_or(false, |n| n == ".magicfsignore") {
                                tracing::info!("[Librarian] .magicfsignore modified. Reloading rules...");
                                if let Some(root) = paths_vec.iter().map(Path::new).find(|r| path.starts_with(r)) {
                                    ignore_manager.load_rules_for_root(root);
                                }
                            }
                        }

                        // PASS 2: Process all files (now using updated rules)
                        for (path, event) in events_to_process {
                            // Skip the ignore file itself in pass 2 (already handled)
                            if path.file_name().map_or(false, |n| n == ".magicfsignore") {
                                continue;
                            }
                            
                            if let Err(e) = Self::handle_file_event(&Ok(event), &state, &ignore_manager, &paths_vec) {
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

    fn scan_directory_for_files(state: &SharedState, dir_path: &Path, ignore_manager: &IgnoreManager, watch_roots: &[String]) -> Result<()> {
        if !dir_path.exists() {
            tracing::warn!("[Librarian] Scan path does not exist: {}", dir_path.display());
            return Ok(());
        }

        let walker = walkdir::WalkDir::new(dir_path).into_iter();
        
        let filtered_walker = walker.filter_entry(|e| {
            !ignore_manager.is_ignored(e.path(), watch_roots)
        });

        for entry in filtered_walker {
            match entry {
                Ok(entry) => {
                    let path = entry.path();
                    if path.is_file() {
                        if ignore_manager.is_ignored(path, watch_roots) {
                            continue;
                        }
                        let path_str = path.to_string_lossy().to_string();
                        tracing::debug!("[Librarian] Scan found file: {}", path_str);
                        
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

    fn handle_file_event(event: &std::result::Result<Event, notify::Error>, state: &SharedState, ignore_manager: &IgnoreManager, watch_roots: &[String]) -> Result<()> {
        match event {
            Ok(event) => {
                match event.kind {
                    EventKind::Create(_) | EventKind::Modify(_) => {
                        for path in &event.paths {
                            if ignore_manager.is_ignored(path, watch_roots) {
                                continue;
                            }
                            if path.is_file() {
                                let path_str = path.to_string_lossy().to_string();
                                tracing::info!("[Librarian] Queuing file for index: {}", path_str);
                                
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
                            if ignore_manager.is_ignored(path, watch_roots) {
                                continue;
                            }
                            let path_str = path.to_string_lossy().to_string();
                            tracing::info!("[Librarian] Queuing file for deletion: {}", path_str);
                            
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

    pub fn stop(&mut self) -> Result<()> {
        if let Some(handle) = self.thread_handle.take() {
            handle.join().map_err(|_| crate::error::MagicError::State("Failed to join watcher thread".into()))?;
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
