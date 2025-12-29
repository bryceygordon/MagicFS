// FILE: src/core/inode_store.rs
use dashmap::DashMap;
use crate::state::SearchResult;

/// InodeStore: The Authority on "What exists in the filesystem"
/// 
/// Responsibilities:
/// 1. Maps Query Strings <-> Inode Numbers consistently.
/// 2. Stores Search Results (the "Virtual Files").
/// 3. Ensures we never collide with static inodes (1, 2, 3).
#[derive(Debug)]
pub struct InodeStore {
    /// Forward mapping: Query String -> Inode
    query_to_inode: DashMap<String, u64>,
    
    /// Reverse mapping: Inode -> Query String (useful for debugging/readdir)
    inode_to_query: DashMap<u64, String>,

    /// Storage: Inode -> Search Results
    results: DashMap<u64, Vec<SearchResult>>,
}

impl Default for InodeStore {
    fn default() -> Self {
        Self::new()
    }
}

impl InodeStore {
    pub fn new() -> Self {
        Self {
            query_to_inode: DashMap::new(),
            inode_to_query: DashMap::new(),
            results: DashMap::new(),
        }
    }

    /// Get the inode for a query, creating it if it doesn't exist.
    /// This is the PRIMARY way to get a search inode.
    pub fn get_or_create_inode(&self, query: &str) -> u64 {
        if let Some(inode) = self.query_to_inode.get(query) {
            return *inode;
        }

        // Generate new inode
        let inode = self.hash_to_inode(query);
        
        self.query_to_inode.insert(query.to_string(), inode);
        self.inode_to_query.insert(inode, query.to_string());
        
        tracing::debug!("[InodeStore] Mapped '{}' -> Inode {}", query, inode);
        inode
    }

    /// Retrieve search results for a given inode
    pub fn get_results(&self, inode: u64) -> Option<Vec<SearchResult>> {
        self.results.get(&inode).map(|r| r.clone())
    }

    /// Store results for a given inode
    pub fn put_results(&self, inode: u64, results: Vec<SearchResult>) {
        self.results.insert(inode, results);
    }

    /// Check if results exist (fast check for EAGAIN logic)
    pub fn has_results(&self, inode: u64) -> bool {
        self.results.contains_key(&inode)
    }

    /// Clear all results (cache invalidation)
    pub fn clear_results(&self) {
        self.results.clear();
    }

    /// Get all active queries (for readdir on /search)
    pub fn active_queries(&self) -> Vec<(u64, String)> {
        self.query_to_inode.iter()
            .map(|entry| (*entry.value(), entry.key().clone()))
            .collect()
    }

    /// Lookup a query by its inode (Reverse lookup)
    pub fn get_query(&self, inode: u64) -> Option<String> {
        self.inode_to_query.get(&inode).map(|v| v.clone())
    }

    /// Deterministic hash function (High bit set to avoid collisions with 1,2,3)
    pub fn hash_to_inode(&self, s: &str) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        s.hash(&mut hasher);
        // Set high bit to ensure it doesn't collide with low integers
        hasher.finish() | 0x8000000000000000
    }
}
