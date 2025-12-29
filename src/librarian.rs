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

        // Core Foundation: Purge then Scan
        let _ = Self::purge_orphaned_records(&state, &ignore_manager, &paths_vec);

        for path_str in &paths_vec {
            let _ = Self::scan_directory_for_files(&state, Path::new(path_str), &ignore_manager, &paths_vec);
        }

        let (tx, rx) = mpsc::channel();
        let mut watcher = RecommendedWatcher::new(tx, notify::Config::default()).unwrap();
        for path in &paths_vec { let _ = watcher.watch(Path::new(path), RecursiveMode::Recursive); }

        let debounce = Duration::from_millis(200);
        let mut event_queue: HashMap<PathBuf, Event> = HashMap::new();
        let mut last_activity = Instant::now();

        loop {
            match rx.recv_timeout(Duration::from_millis(50)) {
                Ok(Ok(event)) => {
                    if matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)) {
                        for path in &event.paths { event_queue.insert(path.clone(), event.clone()); }
                        last_activity = Instant::now();
                    }
                }
                _ => {
                    if !event_queue.is_empty() && last_activity.elapsed() >= debounce {
                        let events = std::mem::take(&mut event_queue);
                        
                        // PRIORITIZE IGNORE RULES
                        for (path, _) in &events {
                            if path.file_name().map_or(false, |n| n == ".magicfsignore") {
                                for root_str in &paths_vec {
                                    ignore_manager.load_rules_for_root(Path::new(root_str));
                                }
                            }
                        }

                        for (_path, event) in events { 
                            let _ = Self::handle_file_event(&Ok(event), &state, &ignore_manager, &paths_vec);
                        }
                    }
                }
            }
        }
    }

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
        for entry in walkdir::WalkDir::new(dir_path).into_iter().filter_entry(|e| !ignore_manager.is_ignored(e.path(), watch_roots)) {
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
        if !queue_batch.is_empty() {
            files_to_index_arc.lock().unwrap().extend(queue_batch);
        }
        Ok(())
    }

    fn handle_file_event(event: &std::result::Result<Event, notify::Error>, state: &SharedState, ignore_manager: &IgnoreManager, watch_roots: &[String]) -> Result<()> {
        if let Ok(event) = event {
            let files_to_index = {
                let guard = state.read().unwrap();
                guard.files_to_index.clone()
            };
            let mut queue = files_to_index.lock().unwrap();
            for path in &event.paths {
                if ignore_manager.is_ignored(path, watch_roots) { continue; }
                let path_str = path.to_string_lossy().to_string();
                match event.kind {
                    EventKind::Create(_) | EventKind::Modify(_) => { if path.is_file() { queue.push(path_str); } }
                    EventKind::Remove(_) => { queue.push(format!("DELETE:{}", path_str)); }
                    _ => {}
                }
            }
        }
        Ok(())
    }
}
