// FILE: src/librarian.rs
use crate::state::{SharedState, SystemState};
use crate::error::Result;
use notify::{RecommendedWatcher, Watcher, Event, EventKind, RecursiveMode};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

struct IgnoreManager {
    rules: HashMap<PathBuf, HashSet<String>>,
}

impl IgnoreManager {
    fn new() -> Self { Self { rules: HashMap::new() } }
    
    fn load_rules_for_root(&mut self, root: &Path) {
        let ignore_file = root.join(".magicfsignore");
        let mut new_rules = HashSet::new();
        
        // DEFAULT IGNORES
        new_rules.insert(".magicfsignore".to_string());
        new_rules.insert(".magicfs".to_string());
        new_rules.insert(".magic".to_string()); 
        new_rules.insert(".git".to_string());   
        
        if let Ok(content) = fs::read_to_string(&ignore_file) {
            for line in content.lines() {
                let rule = line.trim();
                if !rule.is_empty() && !rule.starts_with('#') { 
                    new_rules.insert(rule.to_string()); 
                }
            }
        }
        self.rules.insert(root.to_path_buf(), new_rules);
    }

    fn is_ignored(&self, abs_path: &Path, watch_roots: &[String]) -> bool {
        for root_str in watch_roots {
            let root = Path::new(root_str);
            if let Ok(relative) = abs_path.strip_prefix(root) {
                if let Some(rules) = self.rules.get(root) {
                    for component in relative.components() {
                        let comp_str = component.as_os_str().to_string_lossy();
                        if rules.contains(comp_str.as_ref()) { return true; }
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
        Self { state, watch_paths: Arc::new(Mutex::new(Vec::new())), thread_handle: None }
    }

    pub fn start(&mut self) -> Result<()> {
        let watch_paths = Arc::clone(&self.watch_paths);
        let state = Arc::clone(&self.state);
        self.thread_handle = Some(thread::spawn(move || { Self::watcher_loop(watch_paths, state); }));
        Ok(())
    }

    pub fn add_watch_path(&self, path: String) -> Result<()> {
        self.watch_paths.lock().unwrap().push(path);
        Ok(())
    }

    fn watcher_loop(watch_paths: Arc<Mutex<Vec<String>>>, state: SharedState) {
        let paths_vec = watch_paths.lock().unwrap().clone();
        let mut ignore_manager = IgnoreManager::new();
        for path_str in &paths_vec { ignore_manager.load_rules_for_root(Path::new(path_str)); }

        // ==== WAR MODE PHASE 1: INITIAL BULK INDEXING ====
        tracing::info!("[Librarian] 🚀 Starting initial bulk scan");

        // 1. Engage War Mode
        {
            let state_guard = state.read().unwrap();
            let mut conn_lock = state_guard.db_connection.lock().unwrap();
            if let Some(conn) = conn_lock.as_mut() {
                let mut repo = crate::storage::Repository::new(conn);
                if let Err(e) = repo.set_performance_mode(true) {
                    tracing::error!("[Librarian] Failed to enter War Mode: {}", e);
                }
            }
        }

        // 2. Update System State
        state.read().unwrap().system_state.store(SystemState::Indexing.as_u8(), Ordering::Relaxed);

        // 3. Purge orphaned records
        let _ = Self::purge_orphaned_records(&state, &ignore_manager, &paths_vec);

        // 3.5 Run Scavenger to move orphans to trash
        let _ = Self::run_scavenger(&state);

        // 4. Perform bulk scan for all roots
        for path_str in &paths_vec {
            let _ = Self::scan_directory_for_files(&state, Path::new(path_str), &ignore_manager, &paths_vec);
        }

        // 5. Wait for Indexing Queue to Drain
        tracing::info!("[Librarian] 🔄 Waiting for initial indexing queue to drain...");
        let mut empty_ticks = 0;
        loop {
            thread::sleep(Duration::from_millis(100));

            let queue_len = {
                let state_guard = state.read().unwrap();
                let files_to_index = state_guard.files_to_index.lock().unwrap();
                files_to_index.len()
            };

            // Check Oracle activity (simplified - we'll check if Oracle thread is still running)
            // In a production system, we'd have proper job tracking
            let oracle_active = {
                let state_guard = state.read().unwrap();
                // Check if embedding_tx exists (actor is alive)
                let tx_guard = state_guard.embedding_tx.read().unwrap();
                tx_guard.is_some()
            };

            if queue_len == 0 {
                empty_ticks += 1;
                // Wait for 2 consecutive empty ticks to ensure stability
                if empty_ticks >= 2 && oracle_active {
                    break;
                }
            } else {
                empty_ticks = 0;
                if queue_len % 100 == 0 {
                    tracing::info!("[Librarian] Still draining queue... {} items remaining", queue_len);
                }
            }
        }

        // 6. Disengage War Mode - Enter Peace Mode
        tracing::info!("[Librarian] 🛡️ Initial indexing complete. Switching to Peace Mode.");
        {
            let state_guard = state.read().unwrap();
            let mut conn_lock = state_guard.db_connection.lock().unwrap();
            if let Some(conn) = conn_lock.as_mut() {
                let mut repo = crate::storage::Repository::new(conn);
                if let Err(e) = repo.set_performance_mode(false) {
                    tracing::error!("[Librarian] Failed to exit War Mode: {}", e);
                }
            }
        }

        // Update System State
        state.read().unwrap().system_state.store(SystemState::Monitoring.as_u8(), Ordering::Relaxed);
        tracing::info!("[Librarian] 🛡️ Peace Mode active. Starting file watcher...");

        // ==== PHASE 2: STEADY-STATE MONITORING ====
        let (tx, rx) = mpsc::channel();
        let mut watcher = RecommendedWatcher::new(tx, notify::Config::default()).unwrap();
        for path in &paths_vec { let _ = watcher.watch(Path::new(path), RecursiveMode::Recursive); }

        // Hardening: Increased debounce to 500ms
        let debounce = Duration::from_millis(500);
        let mut event_queue: HashMap<PathBuf, Event> = HashMap::new();
        let mut last_activity = Instant::now();

        // Incinerator timing: Run every 60 seconds
        let incinerator_interval = Duration::from_secs(60);
        let mut last_incinerator_run = Instant::now();

        loop {
            match rx.recv_timeout(Duration::from_millis(50)) {
                Ok(Ok(event)) => {
                    if matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)) {
                        for path in &event.paths { event_queue.insert(path.clone(), event.clone()); }
                        last_activity = Instant::now();
                    }
                }
                _ => {
                    // Check if it's time to run the incinerator
                    if last_incinerator_run.elapsed() >= incinerator_interval {
                        tracing::debug!("[Librarian] Running periodic Incinerator check...");
                        let _ = Self::run_incinerator(&state);
                        last_incinerator_run = Instant::now();
                    }

                    if !event_queue.is_empty() && last_activity.elapsed() >= debounce {
                        let events = std::mem::take(&mut event_queue);

                        // 1. Reload Ignore Rules
                        for (path, _) in &events {
                            if path.file_name().map_or(false, |n| n == ".magicfsignore") {
                                for root_str in &paths_vec {
                                    ignore_manager.load_rules_for_root(Path::new(root_str));
                                }
                            }
                        }

                        // 2. Process Events
                        for (_path, event) in events {
                            let _ = Self::handle_file_event(&Ok(event), &state, &ignore_manager, &paths_vec);
                        }
                    }
                }
            }
        }
    }

    fn run_scavenger(state: &SharedState) -> Result<()> {
        let state_guard = state.read().unwrap();
        let mut conn_lock = state_guard.db_connection.lock().unwrap();

        if let Some(conn) = conn_lock.as_mut() {
            // A. Ensure 'trash' tag exists (first, before borrowing mutably for repo)
            let trash_id = {
                let repo = crate::storage::Repository::new(conn);
                match repo.get_tag_id_by_name("trash", None)? {
                    Some(id) => id,
                    None => repo.create_tag("trash", None)?,
                }
            };

            // B. Find Orphans
            let orphans = {
                let repo = crate::storage::Repository::new(conn);
                repo.get_orphans(100)? // Process in batches of 100
            };

            if !orphans.is_empty() {
                tracing::info!("[Scavenger] Found {} orphaned files. Moving to @trash.", orphans.len());

                for file_id in orphans {
                    // We need the filename to link it
                    let filename = conn.query_row(
                        "SELECT abs_path FROM file_registry WHERE file_id = ?1",
                        [file_id],
                        |r| r.get::<_, String>(0)
                    ).unwrap_or_else(|_| "unknown_orphan".to_string());

                    let name = std::path::Path::new(&filename)
                        .file_name().and_then(|n| n.to_str()).unwrap_or("unknown");

                    // Link file to trash tag
                    let repo = crate::storage::Repository::new(conn);
                    let _ = repo.link_file(file_id, trash_id, name);
                }
            }
        }
        Ok(())
    }

    fn run_incinerator(state: &SharedState) -> Result<()> {
        const TRASH_RETENTION_DAYS: i64 = 30;
        const SECONDS_PER_DAY: i64 = 86400;

        let state_guard = state.read().unwrap();
        let mut conn_lock = state_guard.db_connection.lock().unwrap();

        if let Some(conn) = conn_lock.as_mut() {
            // A. Ensure 'trash' tag exists
            let trash_id = {
                let repo = crate::storage::Repository::new(conn);
                match repo.get_tag_id_by_name("trash", None)? {
                    Some(id) => id,
                    None => return Ok(()), // No trash tag means nothing to incinerate
                }
            };

            // B. Get files older than retention period
            let old_files = {
                let repo = crate::storage::Repository::new(conn);
                repo.get_old_trash_files(trash_id, TRASH_RETENTION_DAYS * SECONDS_PER_DAY)?
            };

            if !old_files.is_empty() {
                tracing::info!("[Incinerator] Found {} files older than {} days. Hard deleting.", old_files.len(), TRASH_RETENTION_DAYS);

                for (file_id, display_name, added_at) in old_files {
                    // Log what we're about to incinerate
                    let current_time = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs() as i64;
                    let age_days = (current_time - added_at) / SECONDS_PER_DAY;
                    tracing::info!("[Incinerator] 🔥 Burning file_id={}, name={}, age={} days", file_id, display_name, age_days);

                    // Step 1: Get physical file path before deletion
                    let abs_path = match conn.query_row(
                        "SELECT abs_path FROM file_registry WHERE file_id = ?1",
                        [file_id],
                        |r| r.get::<_, String>(0)
                    ) {
                        Ok(path) => path,
                        Err(e) => {
                            tracing::error!("[Incinerator] Failed to get path for file_id={}: {}", file_id, e);
                            continue; // Skip this file, try next one
                        }
                    };

                    // Step 2: Check if file actually exists before attempting deletion
                    // This prevents race conditions where a file might have been restored
                    if !std::path::Path::new(&abs_path).exists() {
                        tracing::debug!("[Incinerator] File no longer exists, skipping: {}", abs_path);
                        // Clean up the database entry anyway since the physical file is gone
                        let repo = crate::storage::Repository::new(conn);
                        if let Err(e) = repo.delete_file_by_id(file_id) {
                            tracing::error!("[Incinerator] Failed to delete file_id={} from registry: {}", file_id, e);
                        }
                        continue;
                    }

                    // Step 3: Delete physical file from disk
                    match std::fs::remove_file(&abs_path) {
                        Ok(()) => {
                            tracing::debug!("[Incinerator] Deleted physical file: {}", abs_path);
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                            tracing::warn!("[Incinerator] Physical file already gone: {}", abs_path);
                        }
                        Err(e) => {
                            tracing::error!("[Incinerator] Failed to delete physical file {}: {}", abs_path, e);
                            // Continue with database cleanup anyway - the file is in trash, we should clean up the record
                        }
                    }

                    // Step 4: Clean up database entries (registry + tags via cascade)
                    let repo = crate::storage::Repository::new(conn);
                    if let Err(e) = repo.delete_file_by_id(file_id) {
                        tracing::error!("[Incinerator] Failed to delete file_id={} from registry: {}", file_id, e);
                    } else {
                        tracing::debug!("[Incinerator] Successfully incinerated file_id={}", file_id);
                    }
                }

                tracing::info!("[Incinerator] Incineration complete.");
            }
        }
        Ok(())
    }

    fn purge_orphaned_records(state: &SharedState, ignore_manager: &IgnoreManager, watch_roots: &[String]) -> Result<()> {
        let state_guard = state.read().unwrap();
        // MUTABLE LOCK
        let mut conn_lock = state_guard.db_connection.lock().unwrap();
        let conn = conn_lock.as_mut().unwrap();
        let repo = crate::storage::Repository::new(conn);

        let mut deletion_queue: Vec<u64> = Vec::new();
        const BATCH_SIZE: usize = 1000;

        tracing::info!("[Librarian] Starting orphan scan (Streaming Mode)...");

        repo.scan_all_files(|id, path_str| {
            let path = Path::new(&path_str);
            let should_delete = !path.exists() || ignore_manager.is_ignored(path, watch_roots);
            
            if should_delete {
                deletion_queue.push(id);
            }

            if deletion_queue.len() >= BATCH_SIZE {
                for orphan_id in &deletion_queue {
                    let _ = repo.delete_file_by_id(*orphan_id);
                }
                deletion_queue.clear();
            }
            Ok(())
        })?;

        for orphan_id in &deletion_queue {
            let _ = repo.delete_file_by_id(*orphan_id);
        }

        tracing::info!("[Librarian] Orphan scan complete.");
        Ok(())
    }

    fn scan_directory_for_files(state: &SharedState, dir_path: &Path, ignore_manager: &IgnoreManager, watch_roots: &[String]) -> Result<()> {
        if !dir_path.exists() { return Ok(()); }
        
        let files_to_index_arc = {
            let state_guard = state.read().unwrap();
            state_guard.files_to_index.clone()
        };

        let mut queue_batch = Vec::new();
        
        for entry in walkdir::WalkDir::new(dir_path)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| {
                if e.path_is_symlink() { return false; }
                !ignore_manager.is_ignored(e.path(), watch_roots)
            }) 
        {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.is_file() {
                    let path_str = path.to_string_lossy().to_string();
                    let fs_mtime = path.metadata().ok().and_then(|m| m.modified().ok())
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
                    let fs_size = path.metadata().ok().map(|m| m.len()).unwrap_or(0);
                    
                    // PERFORMANCE FIX: Lock DB only for the specific check
                    let should_index = {
                        let state_guard = state.read().unwrap();
                        // MUTABLE LOCK
                        let mut conn_lock = state_guard.db_connection.lock().unwrap();
                        let conn = conn_lock.as_mut().unwrap();
                        let repo = crate::storage::Repository::new(conn);
                        
                        match repo.get_file_metadata(&path_str) {
                            Ok(Some((db_mtime, db_size))) => (fs_mtime > db_mtime) || (fs_size != db_size),
                            _ => true,
                        }
                    };
                    
                    if should_index { queue_batch.push(path_str); }
                }
            }
        }
        
        if !queue_batch.is_empty() {
            files_to_index_arc.lock().unwrap().extend(queue_batch);
        }
        Ok(())
    }

    fn handle_file_event(event: &std::result::Result<Event, notify::Error>, state: &SharedState, ignore_manager: &IgnoreManager, watch_roots: &[String]) -> Result<()> {
        if let Ok(event) = event {
            
            // MANUAL REFRESH TRIGGER
            for path in &event.paths {
                if let Some(file_name) = path.file_name() {
                    if file_name == "refresh" {
                        if let Some(parent) = path.parent() {
                             if parent.file_name().map_or(false, |n| n == ".magic") {
                                tracing::info!("[Librarian] 🔄 Manual Refresh Triggered via {:?}", path);
                                for root in watch_roots {
                                    tracing::info!("[Librarian] Rescanning root: {}", root);
                                    let _ = Self::scan_directory_for_files(state, Path::new(root), ignore_manager, watch_roots);
                                }
                                let _ = Self::run_scavenger(&state);
                                return Ok(());
                            }
                        }
                    }
                }
            }

            let files_to_index = {
                let guard = state.read().unwrap();
                guard.files_to_index.clone()
            };
            
            let mut queue = files_to_index.lock().unwrap_or_else(|e| e.into_inner());

            for path in &event.paths {
                if ignore_manager.is_ignored(path, watch_roots) { continue; }
                if path.is_symlink() { continue; }

                let path_str = path.to_string_lossy().to_string();
                match event.kind {
                    EventKind::Create(_) | EventKind::Modify(_) => { 
                        if path.is_file() { 
                            tracing::debug!("[Librarian] Queueing index: {}", path_str);
                            queue.push(path_str); 
                        } 
                    }
                    EventKind::Remove(_) => { 
                        tracing::debug!("[Librarian] Queueing delete: {}", path_str);
                        queue.push(format!("DELETE:{}", path_str)); 
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }
}
