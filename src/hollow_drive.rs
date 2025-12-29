// FILE: src/hollow_drive.rs
//! Hollow Drive: The Synchronous FUSE Loop (The Face)
//!
//! This is the "Dumb Terminal" that accepts syscalls (lookup, readdir)
//! and returns data from Memory Cache.
//!
//! CRITICAL RULE: NEVER touches disk or runs embeddings.
//! Returns EAGAIN or placeholder if data is missing.
//! NEVER blocks the FUSE loop for >10ms.

use fuser::{Filesystem, ReplyEntry, ReplyAttr, ReplyDirectory, ReplyStatfs, ReplyOpen, ReplyData, Request};
use crate::state::SharedState;

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
            let query = name_str.to_string();
            
            // USE INODE STORE
            let state_guard = self.state.read().unwrap();
            let inode_store = &state_guard.inode_store;

            let search_inode = inode_store.get_or_create_inode(&query);
            let has_results = inode_store.has_results(search_inode);

            if !has_results {
                tracing::debug!("[HollowDrive] Lookup '{}' (Inode {}) -> EAGAIN", query, search_inode);
            }
            
            drop(state_guard);

            if has_results {
                let attr = mk_attr(search_inode, fuser::FileType::Directory);
                reply.entry(&ttl, &attr, 0);
            } else {
                reply.error(libc::EAGAIN); 
            }
            return;
        }

        // Handle dynamic search result directories (parent is a search inode)
        // Check if parent is a dynamic search inode
        if parent > 3 {
            let state_guard = self.state.read().unwrap();
            let inode_store = &state_guard.inode_store;

            if inode_store.get_query(parent).is_some() {
                // This is a lookup inside a search results directory
                // The name should be a score_filename.txt
                
                // Generate inode for this search result file
                let file_inode = inode_store.hash_to_inode(&format!("{}-{}", parent, name_str));
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
                let state_guard = self.state.read().map_err(|_| {
                    tracing::error!("[HollowDrive] Failed to acquire read lock for getattr");
                    libc::EIO
                }).unwrap();

                // USE INODE STORE
                if state_guard.inode_store.get_query(ino).is_some() {
                    (fuser::FileType::Directory, 4096, 2)
                } else {
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
        use fuser::FileType;

        tracing::debug!("[HollowDrive] readdir: ino={}, offset={}", ino, offset);

        let entries = vec![
            (1, FileType::Directory, ".".to_string()),
            (1, FileType::Directory, "..".to_string()),
        ];

        // Root directory
        if ino == 1 {
            let all_entries = entries.clone();
            let mut root_entries = all_entries;
            root_entries.extend_from_slice(&[
                (2, FileType::Directory, ".magic".to_string()),
                (3, FileType::Directory, "search".to_string()),
            ]);

            for (i, (ino, file_type, name)) in root_entries.iter().enumerate().skip(offset as usize) {
                if reply.add(*ino, (i + 1) as i64, *file_type, name) {
                    break;
                }
            }
            reply.ok();
            return;
        }

        // .magic directory
        if ino == 2 {
            let all_entries = entries;
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
            let all_entries = entries;
            let mut search_entries = all_entries;
            
            // List active searches from INODE STORE
            let state_guard = self.state.read().map_err(|_| {
                tracing::error!("[HollowDrive] Failed to acquire read lock for readdir");
                libc::EIO
            }).unwrap();

            let active_queries = state_guard.inode_store.active_queries();
            
            for (search_inode, query) in active_queries {
                search_entries.push((search_inode, FileType::Directory, query));
            }
            drop(state_guard);

            for (i, (ino, file_type, name)) in search_entries.iter().enumerate().skip(offset as usize) {
                if reply.add(*ino, (i + 1) as i64, *file_type, name) {
                    break;
                }
            }
            reply.ok();
            return;
        }

        // Dynamic search result directories
        {
            let state_guard = self.state.read().map_err(|_| {
                tracing::error!("[HollowDrive] Failed to acquire read lock for search readdir");
                libc::EIO
            }).unwrap();
            let inode_store = &state_guard.inode_store;

            if let Some(results) = inode_store.get_results(ino) {
                tracing::debug!("[HollowDrive] readdir for Inode {} found {} results", ino, results.len());
                
                let mut all_entries = entries;
                let search_results = &results;

                for (_i, result) in search_results.iter().enumerate() {
                    // File name format: 0.95_filename.txt
                    let score_str = format!("{:.2}", result.score);
                    let filename = format!("{}_{}", score_str, result.filename);
                    let file_inode = inode_store.hash_to_inode(&format!("{}-{}", ino, &filename));

                    all_entries.push((file_inode, fuser::FileType::RegularFile, filename));
                }

                for (i, (ino, file_type, name)) in all_entries.iter().enumerate().skip(offset as usize) {
                    if reply.add(*ino, (i + 1) as i64, *file_type, name) {
                        break;
                    }
                }
                reply.ok();
                return;
            } else {
                 tracing::debug!("[HollowDrive] readdir for Inode {} found NO results in inode_store", ino);
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
            let state_guard = self.state.read().map_err(|_| {
                tracing::error!("[HollowDrive] Failed to acquire read lock for read");
                libc::EIO
            }).unwrap();
            let inode_store = &state_guard.inode_store;

            // Find which search result file this is
            // We iterate active searches to find the owner
            let queries = inode_store.active_queries();

            for (search_inode, _) in queries {
                if let Some(results) = inode_store.get_results(search_inode) {
                    for result in results {
                        let score_str = format!("{:.2}", result.score);
                        let filename = format!("{}_{}", score_str, result.filename);
                        let expected_inode = inode_store.hash_to_inode(&format!("{}-{}", search_inode, &filename));

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
            }
        }

        reply.error(libc::ENOENT);
    }
}
