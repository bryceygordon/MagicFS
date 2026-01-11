// FILE: src/core/inode_store.rs
use std::collections::{HashMap, BTreeMap};
use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};
use crate::state::SearchResult;
// Removed DefaultHasher import to avoid temptation

// --- INODE ZONING SPECIFICATION ---
// Bit 63 (Highest Bit) indicates PERSISTENCE.
const PERSISTENT_FLAG: u64 = 1 << 63; 

#[derive(Debug, Clone)]
pub struct Inode {
    pub id: u64,
    pub query: String,
    pub parent: u64,
    pub is_dir: bool,
    pub children: Vec<u64>,
    pub results: Option<Vec<SearchResult>>,
    pub created_at: u64,
    pub initialized: bool,
}

pub struct InodeStore {
    queries: RwLock<HashMap<String, u64>>,
    inodes: RwLock<BTreeMap<u64, Inode>>,
    next_inode: RwLock<u64>,
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
            initialized: true,
        });

        Self {
            queries: RwLock::new(HashMap::new()),
            inodes: RwLock::new(inodes),
            next_inode: RwLock::new(100),
            mirror_paths: RwLock::new(HashMap::new()),
        }
    }

    pub fn is_persistent(inode: u64) -> bool {
        (inode & PERSISTENT_FLAG) != 0
    }

    pub fn db_id_to_inode(db_id: u64) -> u64 {
        db_id | PERSISTENT_FLAG
    }

    pub fn inode_to_db_id(inode: u64) -> u64 {
        inode & !PERSISTENT_FLAG
    }

    pub fn get_or_create_inode(&self, query: &str) -> u64 {
        {
            let map = self.queries.read().unwrap();
            if let Some(&id) = map.get(query) {
                return id;
            }
        }

        let mut map = self.queries.write().unwrap();
        let mut inodes = self.inodes.write().unwrap();
        let mut next = self.next_inode.write().unwrap();

        if let Some(&id) = map.get(query) {
            return id;
        }

        let id = *next;
        *next += 1;

        inodes.insert(id, Inode {
            id,
            query: query.to_string(),
            parent: 1, 
            is_dir: true,
            children: Vec::new(),
            results: None,
            created_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            initialized: false,
        });

        map.insert(query.to_string(), id);

        if let Some(root) = inodes.get_mut(&1) {
            root.children.push(id);
        }

        id
    }

    pub fn get_inode(&self, inode: u64) -> Option<Inode> {
        if Self::is_persistent(inode) {
            return None; 
        }
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

    pub fn mark_active(&self, inode: u64) {
        if inode <= 5 { return; }
        let mut inodes = self.inodes.write().unwrap();
        if let Some(node) = inodes.get_mut(&inode) {
            if !node.initialized {
                node.initialized = true;
                tracing::debug!("[InodeStore] Triggered activation for Inode {} ('{}')", inode, node.query);
            }
        }
    }

    /// STABLE FNV-1a HASHING
    /// Replaces DefaultHasher to guarantee that hash_to_inode("A") 
    /// returns the same ID across readdir() and open() calls.
    pub fn hash_to_inode(&self, key: &str) -> u64 {
        const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
        const FNV_PRIME: u64 = 0x100000001b3;
        const SQLITE_MAX_INT: u64 = 0x7FFFFFFFFFFFFFFF; // 2^63-1

        let mut hash = FNV_OFFSET_BASIS;
        for byte in key.bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(FNV_PRIME);
        }

        // Ensure it doesn't collide with reserved inodes or small counters
        // AND stays within SQLite INTEGER range (signed 64-bit)
        let hash = hash.saturating_add(100000);
        hash & SQLITE_MAX_INT
    }

    pub fn put_mirror_path(&self, inode: u64, path: String) {
        self.mirror_paths.write().unwrap().insert(inode, path);
    }

    pub fn get_mirror_path(&self, inode: u64) -> Option<String> {
        self.mirror_paths.read().unwrap().get(&inode).cloned()
    }

    pub fn active_queries(&self) -> Vec<(u64, String)> {
        let inodes = self.inodes.read().unwrap();
        inodes.values()
            .filter(|n| n.id > 5 && n.is_dir) 
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

    pub fn prune_inode(&self, inode: u64) {
        if inode <= 5 { return; } 

        let mut inodes = self.inodes.write().unwrap();
        let mut queries = self.queries.write().unwrap();

        if let Some(node) = inodes.remove(&inode) {
            queries.remove(&node.query);
            if let Some(parent) = inodes.get_mut(&node.parent) {
                if let Some(pos) = parent.children.iter().position(|&x| x == inode) {
                    parent.children.swap_remove(pos);
                }
            }
        }
    }
}
