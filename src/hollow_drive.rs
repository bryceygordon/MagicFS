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
use std::fs::File;
use std::os::unix::fs::FileExt;

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
            if name_str == "." || name_str == ".." {
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
            if name_str == "refresh" {
                let mut attr = mk_attr(4, fuser::FileType::RegularFile);
                attr.size = 0;
                attr.perm = 0o666; 
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
            
            let state_guard = self.state.read().unwrap();
            let inode_store = &state_guard.inode_store;

            let search_inode = inode_store.get_or_create_inode(&query);
            let has_results = inode_store.has_results(search_inode);

            drop(state_guard);

            if has_results {
                let attr = mk_attr(search_inode, fuser::FileType::Directory);
                reply.entry(&ttl, &attr, 0);
            } else {
                // Return EAGAIN to tell the caller to try again (Polling)
                reply.error(libc::EAGAIN); 
            }
            return;
        }

        // Handle dynamic search result directories (parent is a search inode)
        if parent > 4 {
            let state_guard = self.state.read().unwrap();
            let inode_store = &state_guard.inode_store;

            if inode_store.get_query(parent).is_some() {
                // This is a lookup inside a search results directory
                // Generate inode for this search result file
                let file_inode = inode_store.hash_to_inode(&format!("{}-{}", parent, name_str));
                
                // It's a file, so we mark it as RegularFile
                let attr = mk_attr(file_inode, fuser::FileType::RegularFile);
                reply.entry(&ttl, &attr, 0);
                return;
            }
        }

        reply.error(libc::ENOENT);
    }

    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        use std::time::SystemTime;

        let ttl = std::time::Duration::from_secs(1);

        // Determine file type and attributes based on inode number
        let (kind, size, nlink, perm) = match ino {
            // Root directory
            1 => (fuser::FileType::Directory, 4096, 3, 0o755),
            // .magic directory
            2 => (fuser::FileType::Directory, 4096, 2, 0o755),
            // search directory
            3 => (fuser::FileType::Directory, 4096, 2, 0o755),
            // Refresh file
            4 => (fuser::FileType::RegularFile, 0, 1, 0o666),
            // Search result directories OR Files
            _ => {
                let state_guard = self.state.read().map_err(|_| {
                    tracing::error!("[HollowDrive] Failed to acquire read lock for getattr");
                    libc::EIO
                }).unwrap();

                // If the inode maps to a Query String, it's a Directory
                if state_guard.inode_store.get_query(ino).is_some() {
                    (fuser::FileType::Directory, 4096, 2, 0o755)
                } else {
                    // Otherwise, it's a File inside a search result
                    // We give it a dummy size of 1KB. 
                    // Real size is determined at read() time via Passthrough.
                    (fuser::FileType::RegularFile, 1024, 1, 0o644)
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
            perm,
            nlink,
            uid: 1000,
            gid: 1000,
            rdev: 0,
            blksize: 4096,
            flags: 0,
        };

        reply.attr(&ttl, &attr);
    }

    fn setattr(
        &mut self,
        _req: &Request,
        ino: u64,
        _mode: Option<u32>,
        _uid: Option<u32>,
        _gid: Option<u32>,
        _size: Option<u64>,
        _atime: Option<fuser::TimeOrNow>,
        _mtime: Option<fuser::TimeOrNow>,
        _ctime: Option<std::time::SystemTime>,
        _fh: Option<u64>,
        _crtime: Option<std::time::SystemTime>,
        _chgtime: Option<std::time::SystemTime>,
        _bkuptime: Option<std::time::SystemTime>,
        _flags: Option<u32>,
        reply: ReplyAttr
    ) {
        if ino == 4 {
            let state_guard = self.state.read().unwrap();
            state_guard.refresh_signal.store(true, std::sync::atomic::Ordering::Relaxed);
            
            let ttl = std::time::Duration::from_secs(1);
            let attr = fuser::FileAttr {
                ino: 4, size: 0, blocks: 0, atime: std::time::SystemTime::now(),
                mtime: std::time::SystemTime::now(), ctime: std::time::SystemTime::now(),
                crtime: std::time::SystemTime::now(), kind: fuser::FileType::RegularFile,
                perm: 0o666, nlink: 1, uid: 1000, gid: 1000, rdev: 0, blksize: 4096, flags: 0,
            };
            reply.attr(&ttl, &attr);
            return;
        }
        reply.error(libc::EACCES);
    }

    fn readdir(&mut self, _req: &Request, ino: u64, _fh: u64, offset: i64, mut reply: ReplyDirectory) {
        use fuser::FileType;
        let entries = vec![
            (1, FileType::Directory, ".".to_string()),
            (1, FileType::Directory, "..".to_string()),
        ];

        if ino == 1 { // Root
            let mut root_entries = entries.clone();
            root_entries.extend_from_slice(&[
                (2, FileType::Directory, ".magic".to_string()),
                (3, FileType::Directory, "search".to_string()),
            ]);
            for (i, (ino, file_type, name)) in root_entries.iter().enumerate().skip(offset as usize) {
                if reply.add(*ino, (i + 1) as i64, *file_type, name) { break; }
            }
            reply.ok(); return;
        }

        if ino == 2 { // .magic
            let mut all_entries = entries;
            all_entries.push((4, FileType::RegularFile, "refresh".to_string()));
            for (i, (ino, file_type, name)) in all_entries.iter().enumerate().skip(offset as usize) {
                if reply.add(*ino, (i + 1) as i64, *file_type, name) { break; }
            }
            reply.ok(); return;
        }

        if ino == 3 { // search
            let mut search_entries = entries;
            let state_guard = self.state.read().unwrap();
            for (search_inode, query) in state_guard.inode_store.active_queries() {
                search_entries.push((search_inode, FileType::Directory, query));
            }
            drop(state_guard);

            for (i, (ino, file_type, name)) in search_entries.iter().enumerate().skip(offset as usize) {
                if reply.add(*ino, (i + 1) as i64, *file_type, name) { break; }
            }
            reply.ok(); return;
        }

        // Dynamic search results
        {
            let state_guard = self.state.read().unwrap();
            let inode_store = &state_guard.inode_store;

            if let Some(results) = inode_store.get_results(ino) {
                let mut all_entries = entries;
                for result in results {
                    let score_str = format!("{:.2}", result.score);
                    let filename = format!("{}_{}", score_str, result.filename);
                    let file_inode = inode_store.hash_to_inode(&format!("{}-{}", ino, &filename));
                    all_entries.push((file_inode, fuser::FileType::RegularFile, filename));
                }

                for (i, (ino, file_type, name)) in all_entries.iter().enumerate().skip(offset as usize) {
                    if reply.add(*ino, (i + 1) as i64, *file_type, name) { break; }
                }
                reply.ok(); return;
            }
        }
        
        // Default empty directory if not found
        for (i, (ino, file_type, name)) in entries.iter().enumerate().skip(offset as usize) {
            if reply.add(*ino, (i + 1) as i64, *file_type, name) { break; }
        }
        reply.ok();
    }

    fn statfs(&mut self, _req: &Request, _ino: u64, reply: ReplyStatfs) {
        reply.statfs(4096, 4096, 0, 0, 0, 0, 0, 255);
    }

    fn open(&mut self, _req: &Request, ino: u64, _flags: i32, reply: ReplyOpen) {
        if ino > 4 { reply.opened(0, 0); return; } // Search results
        if ino == 4 { reply.opened(0, 0); return; } // Refresh button
        reply.error(libc::ENOENT);
    }

    fn read(&mut self, _req: &Request, ino: u64, _fh: u64, offset: i64, size: u32, _flags: i32, _lock_owner: Option<u64>, reply: ReplyData) {
        tracing::debug!("[HollowDrive] read: ino={}, offset={}, size={}", ino, offset, size);

        // PASSTHROUGH LOGIC for Search Results
        if ino > 4 {
            let state_guard = self.state.read().unwrap();
            let inode_store = &state_guard.inode_store;
            let active_queries = inode_store.active_queries();

            for (search_inode, _) in active_queries {
                if let Some(results) = inode_store.get_results(search_inode) {
                    for result in results {
                        // Reconstruct Inode
                        let score_str = format!("{:.2}", result.score);
                        let filename = format!("{}_{}", score_str, result.filename);
                        let expected_inode = inode_store.hash_to_inode(&format!("{}-{}", search_inode, &filename));

                        if expected_inode == ino {
                            let abs_path = result.abs_path.clone();
                            drop(state_guard); // DROP LOCK BEFORE IO

                            match File::open(&abs_path) {
                                Ok(file) => {
                                    let mut buffer = vec![0u8; size as usize];
                                    match file.read_at(&mut buffer, offset as u64) {
                                        Ok(bytes_read) => {
                                            reply.data(&buffer[..bytes_read]);
                                        },
                                        Err(e) => {
                                            tracing::error!("[HollowDrive] Read failed: {}", e);
                                            reply.error(libc::EIO);
                                        }
                                    }
                                },
                                Err(e) => {
                                    tracing::warn!("[HollowDrive] Open failed for {}: {}", abs_path, e);
                                    reply.error(libc::ENOENT);
                                }
                            }
                            return;
                        }
                    }
                }
            }
        }

        // Refresh button (empty)
        if ino == 4 { reply.data(&[]); return; }

        reply.error(libc::ENOENT);
    }
}
