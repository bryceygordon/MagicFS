// FILE: src/core/inode_store.rs
use std::collections::{HashMap, BTreeMap};
use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};
use crate::state::SearchResult;
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;

#[derive(Debug, Clone)]
pub struct Inode {
    pub id: u64,
    pub query: String,
    pub parent: u64,
    pub is_dir: bool,
    pub children: Vec<u64>,
    pub results: Option<Vec<SearchResult>>,
    pub created_at: u64,
    /// NEW: Has this inode been accessed (readdir/lookup child) yet?
    /// This prevents "Typewriter" phantom searches.
    pub initialized: bool,
}

pub struct InodeStore {
    // Maps Query String -> Inode ID
    queries: RwLock<HashMap<String, u64>>,
    
    // Maps Inode ID -> Inode Data
    inodes: RwLock<BTreeMap<u64, Inode>>,
    
    // Counter for new dynamic inodes
    next_inode: RwLock<u64>,

    // Maps Inode ID -> Real File Path (for Mirror Mode)
    mirror_paths: RwLock<HashMap<u64, String>>,
}

impl InodeStore {
    pub fn new() -> Self {
        let mut inodes = BTreeMap::new();
        
        // Create Root Inode (1)
        inodes.insert(1, Inode {
            id: 1,
            query: "".to_string(),
            parent: 1,
            is_dir: true,
            children: Vec::new(),
            results: None,
            created_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            initialized: true, // Root is always active
        });

        Self {
            queries: RwLock::new(HashMap::new()),
            inodes: RwLock::new(inodes),
            // Start at 100 to avoid collision with reserved FUSE inodes (1=Root, 2=.magic, 3=search, 4=refresh, 5=mirror)
            next_inode: RwLock::new(100),
            mirror_paths: RwLock::new(HashMap::new()),
        }
    }

    pub fn get_or_create_inode(&self, query: &str) -> u64 {
        // Fast path: Check existence
        {
            let map = self.queries.read().unwrap();
            if let Some(&id) = map.get(query) {
                return id;
            }
        }

        // Slow path: Create new
        let mut map = self.queries.write().unwrap();
        let mut inodes = self.inodes.write().unwrap();
        let mut next = self.next_inode.write().unwrap();

        // Double check after lock
        if let Some(&id) = map.get(query) {
            return id;
        }

        let id = *next;
        *next += 1;

        // Register Inode
        inodes.insert(id, Inode {
            id,
            query: query.to_string(),
            parent: 1, // All search queries are children of Root
            is_dir: true,
            children: Vec::new(),
            results: None,
            created_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            // NEW: Default to FALSE. We acknowledge existence, but don't search yet.
            initialized: false,
        });

        map.insert(query.to_string(), id);

        // Add to Root's children
        if let Some(root) = inodes.get_mut(&1) {
            root.children.push(id);
        }

        id
    }

    pub fn get_inode(&self, inode: u64) -> Option<Inode> {
        self.inodes.read().unwrap().get(&inode).cloned()
    }

    pub fn get_query(&self, inode: u64) -> Option<String> {
        self.inodes.read().unwrap().get(&inode).map(|n| n.query.clone())
    }

    pub fn get_results(&self, inode: u64) -> Option<Vec<SearchResult>> {
        self.inodes.read().unwrap().get(&inode).and_then(|n| n.results.clone())
    }

    pub fn put_results(&self, inode: u64, results: Vec<SearchResult>) {
        let mut inodes = self.inodes.write().unwrap();
        if let Some(node) = inodes.get_mut(&inode) {
            node.results = Some(results);
        }
    }

    pub fn has_results(&self, inode: u64) -> bool {
        let inodes = self.inodes.read().unwrap();
        if let Some(node) = inodes.get(&inode) {
            return node.results.is_some();
        }
        false
    }

    // NEW: The Trigger Mechanism
    // Called by FUSE when a directory is entered (readdir) or a child file is accessed.
    pub fn mark_active(&self, inode: u64) {
        // Protect reserved inodes
        if inode <= 5 { return; }

        let mut inodes = self.inodes.write().unwrap();
        if let Some(node) = inodes.get_mut(&inode) {
            if !node.initialized {
                node.initialized = true;
                tracing::debug!("[InodeStore] Triggered activation for Inode {} ('{}')", inode, node.query);
            }
        }
    }

    // Deterministic hashing for file mapping (used for Mirror Mode and Search Results)
    pub fn hash_to_inode(&self, key: &str) -> u64 {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        // Ensure it doesn't collide with reserved inodes or small counters
        hasher.finish().saturating_add(100000)
    }

    // Mirror Path Management
    pub fn put_mirror_path(&self, inode: u64, path: String) {
        self.mirror_paths.write().unwrap().insert(inode, path);
    }

    pub fn get_mirror_path(&self, inode: u64) -> Option<String> {
        self.mirror_paths.read().unwrap().get(&inode).cloned()
    }

    /// Returns a snapshot of active queries (InodeID, QueryString)
    /// Used by Oracle to find work.
    /// UPDATED: Now filters by `initialized` to support Lazy Search.
    pub fn active_queries(&self) -> Vec<(u64, String)> {
        let inodes = self.inodes.read().unwrap();
        inodes.values()
            .filter(|n| n.id > 5 && n.is_dir) 
            // CRITICAL: Only return queries that have been TRIGGERED
            .filter(|n| n.initialized)
            .map(|n| (n.id, n.query.clone()))
            .collect()
    }

    pub fn clear_results(&self) {
        let mut inodes = self.inodes.write().unwrap();
        for node in inodes.values_mut() {
            if node.id > 1 {
                node.results = None;
            }
        }
    }

    // --- Ghost Busting (Debouncing) ---
    pub fn prune_inode(&self, inode: u64) {
        // Protect reserved inodes (1-5)
        if inode <= 5 { return; } 

        let mut inodes = self.inodes.write().unwrap();
        let mut queries = self.queries.write().unwrap();

        if let Some(node) = inodes.remove(&inode) {
            // 1. Remove from Query Map
            queries.remove(&node.query);

            // 2. Remove from Parent's children list (Root)
            if let Some(parent) = inodes.get_mut(&node.parent) {
                if let Some(pos) = parent.children.iter().position(|&x| x == inode) {
                    parent.children.swap_remove(pos);
                }
            }
        }
    }
}
