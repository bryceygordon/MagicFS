// FILE: src/core/inode_store.rs
use std::collections::{HashMap, BTreeMap};
use std::sync::RwLock; // [FIXED] Removed unused 'Arc'
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
}

pub struct InodeStore {
    // Maps Query String -> Inode ID
    queries: RwLock<HashMap<String, u64>>,
    
    // Maps Inode ID -> Inode Data
    inodes: RwLock<BTreeMap<u64, Inode>>,
    
    // Counter for new dynamic inodes
    next_inode: RwLock<u64>,

    // [RESTORED] Maps Inode ID -> Real File Path (for Mirror Mode)
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
        });

        Self {
            queries: RwLock::new(HashMap::new()),
            inodes: RwLock::new(inodes),
            next_inode: RwLock::new(2),
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

    // [RESTORED]
    pub fn get_query(&self, inode: u64) -> Option<String> {
        self.inodes.read().unwrap().get(&inode).map(|n| n.query.clone())
    }

    // [RESTORED]
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

    // [RESTORED] Deterministic hashing for file mapping
    pub fn hash_to_inode(&self, key: &str) -> u64 {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        // Ensure it doesn't collide with reserved inodes (0, 1) or small counters
        // We use the top bit or a large offset to separate these from sequential IDs if needed.
        // For simplicity in this mock, we just use the hash directly but ensure it's > 100000.
        hasher.finish().saturating_add(100000)
    }

    // [RESTORED] Mirror Path Management
    pub fn put_mirror_path(&self, inode: u64, path: String) {
        self.mirror_paths.write().unwrap().insert(inode, path);
    }

    // [RESTORED]
    pub fn get_mirror_path(&self, inode: u64) -> Option<String> {
        self.mirror_paths.read().unwrap().get(&inode).cloned()
    }

    /// Returns a snapshot of active queries (Query, InodeID)
    /// Used by Oracle to find work.
    pub fn active_queries(&self) -> Vec<(u64, String)> {
        let inodes = self.inodes.read().unwrap();
        inodes.values()
            .filter(|n| n.id > 1 && n.is_dir) // Skip root and files
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
        if inode <= 1 { return; } // Protect Root

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
