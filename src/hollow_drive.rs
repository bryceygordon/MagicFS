//! Hollow Drive: The Synchronous FUSE Loop (The Face)
//!
//! This is the "Dumb Terminal" that accepts syscalls (lookup, readdir)
//! and returns data from Memory Cache.
//!
//! CRITICAL RULE: NEVER touches disk or runs embeddings.
//! Returns EAGAIN or placeholder if data is missing.
//! NEVER blocks the FUSE loop for >10ms.

use fuser::{Filesystem, ReplyEmpty, ReplyEntry, ReplyAttr, ReplyDirectory, ReplyStatfs, ReplyOpen, ReplyData, Request};
use std::sync::Arc;
use crate::state::SharedState;
use crate::error::{Result, MagicError};

/// The Hollow Drive filesystem implementation
pub struct HollowDrive {
    /// Shared state with Oracle and Librarian
    pub state: SharedState,
}

impl HollowDrive {
    /// Create a new Hollow Drive instance
    pub fn new(state: SharedState) -> Self {
        Self { state }
    }

    /// Parse virtual path and return dynamic inode
    fn parse_search_path(&self, path: &str) -> Option<String> {
        // Virtual layout: /search/[query_string]/
        if path.starts_with("/search/") && path.len() > "/search/".len() {
            let query = &path["/search/".len()..];
            // Remove trailing slash if present
            let query = query.trim_end_matches('/');
            if !query.is_empty() {
                return Some(query.to_string());
            }
        }
        None
    }

    /// Get or create dynamic inode for a query
    fn get_or_create_inode(&self, query: &str) -> Result<u64> {
        // Check if we already have this query mapped
        let existing_inode = {
            let state_guard = self.state.read().map_err(|_| MagicError::State("Poisoned lock".into()))?;
            state_guard.active_searches.get(query).map(|entry| *entry.value())
        };

        if let Some(inode) = existing_inode {
            return Ok(inode);
        }

        // Create new inode (simple hash for now, will be replaced with DB inode later)
        let new_inode = self.hash_to_inode(query);
        {
            let mut state_guard = self.state.write().map_err(|_| MagicError::State("Poisoned lock".into()))?;
            state_guard.active_searches.insert(query.to_string(), new_inode);
        }

        Ok(new_inode)
    }

    /// Simple hash function to generate inodes from query strings
    fn hash_to_inode(&self, s: &str) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        s.hash(&mut hasher);
        hasher.finish() as u64
    }
}

impl Filesystem for HollowDrive {
    fn init(&mut self, _req: &Request, _config: &mut fuser::KernelConfig) -> std::result::Result<(), i32> {
        tracing::info!("[HollowDrive] FUSE initialized");
        Ok(())
    }

    fn destroy(&mut self) {
        tracing::info!("[HollowDrive] FUSE destroyed");
    }

    fn lookup(&mut self, _req: &Request, parent: u64, name: &std::ffi::OsStr, reply: ReplyEntry) {
        let name_str = match name.to_str() {
            Some(s) => s,
            None => {
                reply.error(libc::EINVAL);
                return;
            }
        };

        tracing::debug!("[HollowDrive] lookup: parent={}, name={:?}", parent, name_str);

        // Helper to create file attributes
        use std::time::SystemTime;
        let ttl = std::time::Duration::from_secs(1);
        let mk_attr = |ino: u64, kind: fuser::FileType| fuser::FileAttr {
            ino,
            size: 4096,
            blocks: 8,
            atime: SystemTime::now(),
            mtime: SystemTime::now(),
            ctime: SystemTime::now(),
            crtime: SystemTime::now(),
            kind,
            perm: 0o755,
            nlink: 2,
            uid: 1000,
            gid: 1000,
            rdev: 0,
            blksize: 4096,
            flags: 0,
        };

        // Handle root directory (ino=1)
        if parent == 1 {
            if name_str == "." {
                let attr = mk_attr(1, fuser::FileType::Directory);
                reply.entry(&ttl, &attr, 0);
                return;
            } else if name_str == ".." {
                let attr = mk_attr(1, fuser::FileType::Directory);
                reply.entry(&ttl, &attr, 0);
                return;
            } else if name_str == ".magic" {
                let attr = mk_attr(2, fuser::FileType::Directory);
                reply.entry(&ttl, &attr, 0);
                return;
            } else if name_str == "search" {
                let attr = mk_attr(3, fuser::FileType::Directory);
                reply.entry(&ttl, &attr, 0);
                return;
            }
        }

        // Handle .magic directory (ino=2)
        if parent == 2 {
            if name_str == "." || name_str == ".." {
                let attr = mk_attr(2, fuser::FileType::Directory);
                reply.entry(&ttl, &attr, 0);
                return;
            }
            // TODO: Handle .magic files like config.db in future phases
            reply.error(libc::ENOENT);
            return;
        }

        // Handle search directory (ino=3)
        if parent == 3 {
            if name_str == "." || name_str == ".." {
                let attr = mk_attr(3, fuser::FileType::Directory);
                reply.entry(&ttl, &attr, 0);
                return;
            }

            // This is a search query directory
            // Check if we have this query mapped to an inode
            let query = name_str.to_string();
            if let Ok(search_inode) = self.get_or_create_inode(&query) {
                // Check if results are in cache
                let state_guard = self.state.read().map_err(|_| libc::EIO).unwrap();
                let has_results = state_guard.search_results.get(&search_inode).is_some();

                if has_results {
                    // Results are ready
                    drop(state_guard);
                    let attr = mk_attr(search_inode, fuser::FileType::Directory);
                    reply.entry(&ttl, &attr, 0);
                } else {
                    // Results not ready - trigger Oracle to process search
                    drop(state_guard);
                    // Spawn async task to handle the search
                    let query_for_oracle = query.clone();
                    let state_for_oracle = Arc::clone(&self.state);

                    tokio::spawn(async move {
                        // Hash the query to create a consistent inode for this search
                        use std::collections::hash_map::DefaultHasher;
                        use std::hash::{Hash, Hasher};
                        let mut hasher = DefaultHasher::new();
                        query_for_oracle.hash(&mut hasher);
                        let search_inode = hasher.finish() as u64 | 0x8000000000000000; // Mark as dynamic inode

                        // Add the search query to active_searches so Oracle can pick it up
                        let mut state_guard = state_for_oracle.write().unwrap();
                        state_guard.active_searches.insert(query_for_oracle.clone(), search_inode);
                    });

                    reply.error(libc::EAGAIN); // Signal that results aren't ready yet
                }
                return;
            }
        }

        // Handle dynamic search result directories (parent is a search inode)
        // Check if parent is a dynamic search inode
        {
            let state_guard = self.state.read().map_err(|_| libc::EIO).unwrap();
            let is_search_inode = parent > 3; // All search inodes are > 3

            if is_search_inode {
                // This is a lookup inside a search results directory
                // The name should be a score_filename.txt
                // For now, treat all lookups as valid file lookups
                drop(state_guard);

                // Generate inode for this search result file
                let file_inode = self.hash_to_inode(&format!("{}-{}", parent, name_str));
                let attr = mk_attr(file_inode, fuser::FileType::RegularFile);
                reply.entry(&ttl, &attr, 0);
                return;
            }
        }

        // Not found
        reply.error(libc::ENOENT);
    }

    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        use std::time::SystemTime;

        let ttl = std::time::Duration::from_secs(1);

        // Determine file type and attributes based on inode number
        let (kind, size, nlink) = match ino {
            // Root directory
            1 => (fuser::FileType::Directory, 4096, 3),
            // .magic directory
            2 => (fuser::FileType::Directory, 4096, 2),
            // search directory
            3 => (fuser::FileType::Directory, 4096, 2),
            // Search result directories (dynamic inodes)
            _ => {
                // Check if this is a search directory or a search result file
                let state_guard = self.state.read().map_err(|_| {
                    tracing::error!("[HollowDrive] Failed to acquire read lock for getattr");
                    libc::EIO
                }).unwrap();

                // If inode is in search_results, it's a search directory
                if state_guard.search_results.get(&ino).is_some() {
                    drop(state_guard);
                    (fuser::FileType::Directory, 4096, 2)
                } else {
                    drop(state_guard);
                    // Otherwise it's a search result file
                    (fuser::FileType::RegularFile, 1024, 1)
                }
            }
        };

        let attr = fuser::FileAttr {
            ino,
            size,
            blocks: (size / 512 + 1).max(1),
            atime: SystemTime::now(),
            mtime: SystemTime::now(),
            ctime: SystemTime::now(),
            crtime: SystemTime::now(),
            kind,
            perm: 0o644,
            nlink,
            uid: 1000,
            gid: 1000,
            rdev: 0,
            blksize: 4096,
            flags: 0,
        };

        reply.attr(&ttl, &attr);
    }

    fn readdir(&mut self, _req: &Request, ino: u64, _fh: u64, offset: i64, mut reply: ReplyDirectory) {
        use std::time::SystemTime;
        use fuser::FileType;

        tracing::debug!("[HollowDrive] readdir: ino={}, offset={}", ino, offset);

        let entries = vec![
            (1, FileType::Directory, ".".to_string()),
            (1, FileType::Directory, "..".to_string()),
        ];

        // Root directory
        if ino == 1 {
            let mut all_entries = entries.clone();
            all_entries.extend_from_slice(&[
                (2, FileType::Directory, ".magic".to_string()),
                (3, FileType::Directory, "search".to_string()),
            ]);

            for (i, (ino, file_type, name)) in all_entries.iter().enumerate().skip(offset as usize) {
                if reply.add(*ino, (i + 1) as i64, *file_type, name) {
                    break;
                }
            }
            reply.ok();
            return;
        }

        // .magic directory
        if ino == 2 {
            let mut all_entries = entries;
            // TODO: Add config.db and other .magic files here
            for (i, (ino, file_type, name)) in all_entries.iter().enumerate().skip(offset as usize) {
                if reply.add(*ino, (i + 1) as i64, *file_type, name) {
                    break;
                }
            }
            reply.ok();
            return;
        }

        // search directory
        if ino == 3 {
            let mut all_entries = entries;
            // List active searches
            let state_guard = self.state.read().map_err(|_| {
                tracing::error!("[HollowDrive] Failed to acquire read lock for readdir");
                libc::EIO
            }).unwrap();

            for entry in state_guard.active_searches.iter() {
                let query = entry.key();
                let search_inode = *entry.value();
                all_entries.push((search_inode, FileType::Directory, query.clone()));
            }
            drop(state_guard);

            for (i, (ino, file_type, name)) in all_entries.iter().enumerate().skip(offset as usize) {
                if reply.add(*ino, (i + 1) as i64, *file_type, name) {
                    break;
                }
            }
            reply.ok();
            return;
        }

        // Dynamic search result directories
        {
            let results_opt = {
                let state_guard = self.state.read().map_err(|_| {
                    tracing::error!("[HollowDrive] Failed to acquire read lock for search readdir");
                    libc::EIO
                }).unwrap();

                state_guard.search_results.get(&ino).map(|r| r.clone())
            };

            if let Some(results) = results_opt {
                let mut all_entries = entries;
                let search_results = &results;

                for (_i, result) in search_results.iter().enumerate() {
                    // File name format: 0.95_filename.txt
                    let score_str = format!("{:.2}", result.score);
                    let filename = format!("{}_{}", score_str, result.filename);
                    let file_inode = self.hash_to_inode(&format!("{}-{}", ino, &filename));

                    all_entries.push((file_inode, fuser::FileType::RegularFile, filename));
                }

                for (i, (ino, file_type, name)) in all_entries.iter().enumerate().skip(offset as usize) {
                    if reply.add(*ino, (i + 1) as i64, *file_type, name) {
                        break;
                    }
                }
                reply.ok();
                return;
            }
        }

        // If we get here, the directory is either empty or doesn't exist
        for (i, (ino, file_type, name)) in entries.iter().enumerate().skip(offset as usize) {
            if reply.add(*ino, (i + 1) as i64, *file_type, name) {
                break;
            }
        }
        reply.ok();
    }

    fn statfs(&mut self, _req: &Request, _ino: u64, reply: ReplyStatfs) {
        tracing::debug!("[HollowDrive] statfs");

        reply.statfs(
            4096,   // bsize: filesystem block size
            4096,   // frsize: fundamental block size
            0,      // blocks: total blocks
            0,      // bfree: free blocks
            0,      // bavail: available blocks
            0,      // files: total inodes
            0,      // ffree: free inodes
            255,    // namelen: max filename length
        );
    }

    fn open(&mut self, _req: &Request, ino: u64, _flags: i32, reply: ReplyOpen) {
        tracing::debug!("[HollowDrive] open: ino={}", ino);

        // For search result files, just check if they exist
        if ino > 3 {
            // Search result files are always readable
            reply.opened(0, 0);
            return;
        }

        reply.error(libc::ENOENT);
    }

    fn read(&mut self, _req: &Request, ino: u64, _fh: u64, offset: i64, size: u32, _flags: i32, _lock_owner: Option<u64>, reply: ReplyData) {
        tracing::debug!("[HollowDrive] read: ino={}, offset={}, size={}", ino, offset, size);

        // For search result files (ino > 3), return file content
        if ino > 3 {
            // Find which search result file this is
            // We need to search through all search results to find the matching inode

            let search_results_to_check: Vec<(u64, crate::state::SearchResult)> = {
                let state_guard = self.state.read().map_err(|_| {
                    tracing::error!("[HollowDrive] Failed to acquire read lock for read");
                    libc::EIO
                }).unwrap();

                state_guard.search_results.iter()
                    .flat_map(|entry| {
                        let inode_num = *entry.key();
                        entry.value().iter().map(move |result| (inode_num, result.clone())).collect::<Vec<_>>()
                    })
                    .collect()
            };

            // Iterate through all search results to find the matching file
            for (search_inode, result) in search_results_to_check {
                let score_str = format!("{:.2}", result.score);
                let filename = format!("{}_{}", score_str, result.filename);
                let expected_inode = self.hash_to_inode(&format!("{}-{}", search_inode, &filename));

                if expected_inode == ino {
                    // Found the file - return its content
                    // Format: "path/to/file.txt\nScore: 0.95"
                    let content = format!("{}\nScore: {:.2}", result.abs_path, result.score);
                    let content_bytes = content.as_bytes();

                    let read_size = size as usize;
                    let file_size = content_bytes.len();

                    if offset as usize >= file_size {
                        reply.data(&[]);
                        return;
                    }

                    let end = (offset as usize + read_size).min(file_size);
                    reply.data(&content_bytes[offset as usize..end]);
                    return;
                }
            }
        }

        reply.error(libc::ENOENT);
    }
}