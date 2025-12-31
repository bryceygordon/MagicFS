// FILE: src/core/inode_store.rs
use crate::state::SearchResult;
use lru::LruCache;
use std::sync::Mutex;
use std::num::NonZeroUsize;

// SAFETY: Cap active queries to 1000.
// If a user opens 1001 search folders, the first one is forgotten.
// This prevents infinite memory growth from random queries.
const QUERY_CACHE_CAPACITY: usize = 1000;
const RESULTS_CACHE_CAPACITY: usize = 50;

/// InodeStore: The Authority on "What exists in the filesystem"
/// 
/// Refactored to use LRU Caching for Inode Mappings to prevent Memory Leaks.
/// It maps Query Strings <-> Inode Numbers consistently but with a capacity limit.
#[derive(Debug)]
pub struct InodeStore {
    /// Forward mapping: Query String -> Inode
    /// Protected by Mutex for thread safety
    query_cache: Mutex<LruCache<String, u64>>,
    
    /// Reverse mapping: Inode -> Query String
    /// Protected by Mutex for thread safety
    inode_cache: Mutex<LruCache<u64, String>>,

    /// Storage: Inode -> Search Results
    /// Managed via separate LRU to prevent memory bloat from result sets
    results: Mutex<LruCache<u64, Vec<SearchResult>>>,
}

impl Default for InodeStore {
    fn default() -> Self {
        Self::new()
    }
}

impl InodeStore {
    pub fn new() -> Self {
        Self {
            query_cache: Mutex::new(LruCache::new(NonZeroUsize::new(QUERY_CACHE_CAPACITY).unwrap())),
            inode_cache: Mutex::new(LruCache::new(NonZeroUsize::new(QUERY_CACHE_CAPACITY).unwrap())),
            results: Mutex::new(LruCache::new(NonZeroUsize::new(RESULTS_CACHE_CAPACITY).unwrap())),
        }
    }

    /// Get the inode for a query, creating it if it doesn't exist.
    /// This is the PRIMARY way to get a search inode.
    pub fn get_or_create_inode(&self, query: &str) -> u64 {
        // 1. Try Cache First
        {
            let mut q_cache = self.query_cache.lock().unwrap();
            if let Some(inode) = q_cache.get(query) {
                return *inode;
            }
        }

        // 2. Generate new inode (Deterministic hash)
        // We use a deterministic hash so that even if it falls out of cache,
        // re-creating it yields the same inode number (mostly).
        let inode = self.hash_to_inode(query);
        
        // 3. Insert into caches (evicting old ones if necessary)
        {
            let mut q_cache = self.query_cache.lock().unwrap();
            let mut i_cache = self.inode_cache.lock().unwrap();
            
            q_cache.put(query.to_string(), inode);
            i_cache.put(inode, query.to_string());
        }
        
        tracing::debug!("[InodeStore] Mapped '{}' -> Inode {}", query, inode);
        inode
    }

    /// Lookup a query by its inode (Reverse lookup)
    pub fn get_query(&self, inode: u64) -> Option<String> {
        let mut cache = self.inode_cache.lock().unwrap();
        cache.get(&inode).cloned()
    }

    /// Retrieve search results for a given inode
    pub fn get_results(&self, inode: u64) -> Option<Vec<SearchResult>> {
        let mut cache = self.results.lock().unwrap();
        cache.get(&inode).cloned()
    }

    /// Store results for a given inode
    pub fn put_results(&self, inode: u64, results: Vec<SearchResult>) {
        let mut cache = self.results.lock().unwrap();
        cache.put(inode, results);
    }

    /// Check if results exist (fast check for EAGAIN logic)
    pub fn has_results(&self, inode: u64) -> bool {
        let cache = self.results.lock().unwrap();
        cache.contains(&inode)
    }

    /// Clear all results (cache invalidation)
    pub fn clear_results(&self) {
        let mut cache = self.results.lock().unwrap();
        cache.clear();
    }

    /// Get all active queries (for readdir on /search)
    /// Note: This now only returns what is currently in the LRU cache.
    /// Old queries that have been evicted will effectively "disappear" from ls
    /// until searched again. This is intended behavior for memory safety.
    pub fn active_queries(&self) -> Vec<(u64, String)> {
        let cache = self.query_cache.lock().unwrap();
        cache.iter()
            .map(|(k, v)| (*v, k.clone()))
            .collect()
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
