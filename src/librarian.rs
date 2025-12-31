// FILE: src/librarian.rs
use crate::state::SharedState;
use crate::error::Result;
use notify::{RecommendedWatcher, Watcher, Event, EventKind, RecursiveMode};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

// === CONFIGURATION ===
const DEBOUNCE_WINDOW_MS: u128 = 2000; // 2 Seconds
const BATCH_SIZE_LIMIT: usize = 100;   // Starvation prevention

// === INTERNAL TYPES ===
struct DebounceState {
    last_sent: Instant,
    pending: bool,
}

struct IgnoreManager {
    rules: HashMap<PathBuf, HashSet<String>>,
}

impl IgnoreManager {
    fn new() -> Self { Self { rules: HashMap::new() } }
    fn load_rules_for_root(&mut self, root: &Path) {
        let ignore_file = root.join(".magicfsignore");
        let mut new_rules = HashSet::new();
        new_rules.insert(".magicfsignore".to_string());
        new_rules.insert(".magicfs".to_string());
        if let Ok(content) = fs::read_to_string(&ignore_file) {
            for line in content.lines() {
                let rule = line.trim();
                if !rule.is_empty() && !rule.starts_with('#') { new_rules.insert(rule.to_string()); }
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

        let _ = Self::purge_orphaned_records(&state, &ignore_manager, &paths_vec);
        for path_str in &paths_vec {
            let _ = Self::scan_directory_for_files(&state, Path::new(path_str), &ignore_manager, &paths_vec);
        }

        let (tx, rx) = mpsc::channel();
        let mut watcher = RecommendedWatcher::new(tx, notify::Config::default()).unwrap();
        for path in &paths_vec { let _ = watcher.watch(Path::new(path), RecursiveMode::Recursive); }

        let notify_debounce = Duration::from_millis(100);
        let mut debounce_map: HashMap<String, DebounceState> = HashMap::new();
        let mut event_queue: HashMap<PathBuf, Event> = HashMap::new();
        let mut last_activity = Instant::now();

        loop {
            // Check for new events
            let received_event = match rx.recv_timeout(Duration::from_millis(50)) {
                Ok(Ok(event)) => Some(event),
                _ => None,
            };

            // 1. Ingest Event
            if let Some(event) = received_event {
                if matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)) {
                    for path in &event.paths { event_queue.insert(path.clone(), event.clone()); }
                    last_activity = Instant::now();
                }
            }

            // 2. Decide: Process Queue?
            // Process if: (Queue not empty) AND (Timeout passed OR Queue too big)
            let should_process = !event_queue.is_empty() && 
                (last_activity.elapsed() >= notify_debounce || event_queue.len() >= BATCH_SIZE_LIMIT);

            if should_process {
                let events = std::mem::take(&mut event_queue);
                
                // Refresh Ignore Rules (in case .magicfsignore changed)
                for (path, _) in &events {
                    if path.file_name().map_or(false, |n| n == ".magicfsignore") {
                        for root_str in &paths_vec {
                            ignore_manager.load_rules_for_root(Path::new(root_str));
                        }
                    }
                }

                // Process Events
                for (path_buf, event) in events { 
                    let path_str = path_buf.to_string_lossy().to_string();
                    
                    // Thermal Debounce Logic
                    if matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_)) {
                        let should_emit = if let Some(debounce_state) = debounce_map.get_mut(&path_str) {
                            if debounce_state.last_sent.elapsed().as_millis() < DEBOUNCE_WINDOW_MS {
                                debounce_state.pending = true;
                                false 
                            } else {
                                debounce_state.last_sent = Instant::now();
                                debounce_state.pending = false;
                                true
                            }
                        } else {
                            debounce_map.insert(path_str.clone(), DebounceState { 
                                last_sent: Instant::now(), 
                                pending: false 
                            });
                            true
                        };

                        if should_emit {
                            let _ = Self::handle_file_event(&Ok(event), &state, &ignore_manager, &paths_vec);
                        } else {
                            tracing::debug!("[Librarian] Suppressing chatter for {}", path_str);
                        }
                    } else {
                        // Deletes always pass
                        if matches!(event.kind, EventKind::Remove(_)) {
                            debounce_map.remove(&path_str);
                        }
                        let _ = Self::handle_file_event(&Ok(event), &state, &ignore_manager, &paths_vec);
                    }
                }
            }

            // 3. Check Final Promises (Heartbeat)
            for (path_str, debounce_state) in debounce_map.iter_mut() {
                if debounce_state.pending && debounce_state.last_sent.elapsed().as_millis() >= DEBOUNCE_WINDOW_MS {
                    tracing::info!("[Librarian] Firing Final Promise for {}", path_str);
                    let path = PathBuf::from(path_str);
                    let synthetic_event = Event {
                        kind: EventKind::Modify(notify::event::ModifyKind::Any),
                        paths: vec![path],
                        attrs: Default::default(),
                    };
                    let _ = Self::handle_file_event(&Ok(synthetic_event), &state, &ignore_manager, &paths_vec);
                    
                    debounce_state.last_sent = Instant::now();
                    debounce_state.pending = false;
                }
            }
        }
    }

    // ... (rest of methods: purge_orphaned_records, scan_directory, handle_file_event remain same)
    fn purge_orphaned_records(state: &SharedState, ignore_manager: &IgnoreManager, watch_roots: &[String]) -> Result<()> {
        let state_guard = state.read().unwrap();
        let conn_lock = state_guard.db_connection.lock().unwrap();
        let conn = conn_lock.as_ref().unwrap();
        let repo = crate::storage::Repository::new(conn);
        let all_files = repo.get_all_files()?;
        for (id, path_str) in all_files {
            let path = Path::new(&path_str);
            if !path.exists() || ignore_manager.is_ignored(path, watch_roots) {
                let _ = repo.delete_file_by_id(id);
            }
        }
        Ok(())
    }

    fn scan_directory_for_files(state: &SharedState, dir_path: &Path, ignore_manager: &IgnoreManager, watch_roots: &[String]) -> Result<()> {
        if !dir_path.exists() { return Ok(()); }
        let state_guard = state.read().unwrap();
        let conn_lock = state_guard.db_connection.lock().unwrap();
        let conn = conn_lock.as_ref().unwrap();
        let repo = crate::storage::Repository::new(conn);
        let files_to_index_arc = Arc::clone(&state_guard.files_to_index);
        let mut queue_batch = Vec::new();
        for entry in walkdir::WalkDir::new(dir_path)
            .follow_links(false).into_iter()
            .filter_entry(|e| { if e.path_is_symlink() { return false; } !ignore_manager.is_ignored(e.path(), watch_roots) }) {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.is_file() {
                    let path_str = path.to_string_lossy().to_string();
                    let fs_mtime = path.metadata().ok().and_then(|m| m.modified().ok())
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
                    let should_index = match repo.get_file_metadata(&path_str) {
                        Ok(Some((db_mtime, _))) => fs_mtime > db_mtime,
                        _ => true,
                    };
                    if should_index { queue_batch.push(path_str); }
                }
            }
        }
        drop(conn_lock); drop(state_guard);
        if !queue_batch.is_empty() { files_to_index_arc.lock().unwrap().extend(queue_batch); }
        Ok(())
    }

    fn handle_file_event(event: &std::result::Result<Event, notify::Error>, state: &SharedState, ignore_manager: &IgnoreManager, watch_roots: &[String]) -> Result<()> {
        if let Ok(event) = event {
            let files_to_index = {
                let guard = state.read().unwrap();
                guard.files_to_index.clone()
            };
            let mut queue = files_to_index.lock().unwrap_or_else(|e| e.into_inner());
            for path in &event.paths {
                if ignore_manager.is_ignored(path, watch_roots) { continue; }
                if path.is_symlink() { continue; }
                let path_str = path.to_string_lossy().to_string();
                if queue.contains(&path_str) { continue; }
                match event.kind {
                    EventKind::Create(_) | EventKind::Modify(_) => { 
                        if path.is_file() { 
                            tracing::debug!("[Librarian] Queueing index: {}", path_str);
                            queue.push(path_str); 
                        } 
                    }
                    EventKind::Remove(_) => { 
                        let del_cmd = format!("DELETE:{}", path_str);
                        if !queue.contains(&del_cmd) {
                            tracing::debug!("[Librarian] Queueing delete: {}", path_str);
                            queue.push(del_cmd); 
                        }
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }
}
