// FILE: src/core/inode_store.rs
use crate::state::SearchResult;
use lru::LruCache;
use std::sync::Mutex;
use std::num::NonZeroUsize;

// SAFETY: Cap active queries to 1000.
const QUERY_CACHE_CAPACITY: usize = 1000;
const RESULTS_CACHE_CAPACITY: usize = 50;
// NEW: Cap mirror paths to 5000 to allow deep browsing without OOM
const MIRROR_CACHE_CAPACITY: usize = 5000;

/// InodeStore: The Authority on "What exists in the filesystem"
#[derive(Debug)]
pub struct InodeStore {
    /// Forward mapping: Query String -> Inode
    query_cache: Mutex<LruCache<String, u64>>,
    
    /// Reverse mapping: Inode -> Query String
    inode_cache: Mutex<LruCache<u64, String>>,

    /// Storage: Inode -> Search Results
    results: Mutex<LruCache<u64, Vec<SearchResult>>>,

    /// NEW: Mapping Inode -> Real Absolute Path (for Mirror Mode)
    mirror_map: Mutex<LruCache<u64, String>>,
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
            mirror_map: Mutex::new(LruCache::new(NonZeroUsize::new(MIRROR_CACHE_CAPACITY).unwrap())),
        }
    }

    // ... [Keep existing query methods: get_or_create_inode, get_query, get_results, put_results, has_results, clear_results, active_queries] ...
    
    pub fn get_or_create_inode(&self, query: &str) -> u64 {
        {
            let mut q_cache = self.query_cache.lock().unwrap();
            if let Some(inode) = q_cache.get(query) { return *inode; }
        }
        let inode = self.hash_to_inode(query);
        {
            let mut q_cache = self.query_cache.lock().unwrap();
            let mut i_cache = self.inode_cache.lock().unwrap();
            q_cache.put(query.to_string(), inode);
            i_cache.put(inode, query.to_string());
        }
        inode
    }

    pub fn get_query(&self, inode: u64) -> Option<String> {
        let mut cache = self.inode_cache.lock().unwrap();
        cache.get(&inode).cloned()
    }

    pub fn get_results(&self, inode: u64) -> Option<Vec<SearchResult>> {
        let mut cache = self.results.lock().unwrap();
        cache.get(&inode).cloned()
    }

    pub fn put_results(&self, inode: u64, results: Vec<SearchResult>) {
        let mut cache = self.results.lock().unwrap();
        cache.put(inode, results);
    }

    pub fn has_results(&self, inode: u64) -> bool {
        let cache = self.results.lock().unwrap();
        cache.contains(&inode)
    }

    pub fn clear_results(&self) {
        let mut cache = self.results.lock().unwrap();
        cache.clear();
    }

    pub fn active_queries(&self) -> Vec<(u64, String)> {
        let cache = self.query_cache.lock().unwrap();
        cache.iter().map(|(k, v)| (*v, k.clone())).collect()
    }

    // --- NEW: Mirror Mode Methods ---

    pub fn put_mirror_path(&self, inode: u64, path: String) {
        let mut cache = self.mirror_map.lock().unwrap();
        cache.put(inode, path);
    }

    pub fn get_mirror_path(&self, inode: u64) -> Option<String> {
        let mut cache = self.mirror_map.lock().unwrap();
        cache.get(&inode).cloned()
    }

    // --- End New Methods ---

    pub fn hash_to_inode(&self, s: &str) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        s.hash(&mut hasher);
        hasher.finish() | 0x8000000000000000
    }
}
