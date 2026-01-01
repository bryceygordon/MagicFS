// FILE: src/hollow_drive.rs
//! Hollow Drive: The Synchronous FUSE Loop (The Face)
//!
//! This is the "Dumb Terminal" that accepts syscalls (lookup, readdir)
//! and returns data from Memory Cache.
//!
//! CRITICAL RULE: NEVER touches disk or runs embeddings.
//! Returns EAGAIN or placeholder if data is missing.
//! NEVER blocks the FUSE loop for >10ms.

use fuser::{Filesystem, ReplyEntry, ReplyAttr, ReplyDirectory, ReplyStatfs, ReplyOpen, ReplyData, ReplyWrite, Request};
use crate::state::SharedState;
use std::fs::{File, OpenOptions};
use std::os::unix::fs::FileExt; // Required for atomic read_at/write_at
use std::time::SystemTime;
use libc;

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

    /// Helper: Reverse lookup from Inode -> Real Absolute Path
    fn find_real_path(&self, target_inode: u64) -> Option<String> {
        if target_inode <= 4 { return None; }

        let state_guard = self.state.read().ok()?;
        let inode_store = &state_guard.inode_store;

        for (search_inode, _) in inode_store.active_queries() {
            if let Some(results) = inode_store.get_results(search_inode) {
                for result in results {
                    let score_str = format!("{:.2}", result.score);
                    let filename = format!("{}_{}", score_str, result.filename);
                    let expected_inode = inode_store.hash_to_inode(&format!("{}-{}", search_inode, &filename));

                    if expected_inode == target_inode {
                        return Some(result.abs_path.clone());
                    }
                }
            }
        }
        None
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

        let ttl = std::time::Duration::from_secs(1);
        
        let mk_attr = |ino: u64, kind: fuser::FileType| fuser::FileAttr {
            ino, size: 4096, blocks: 8, atime: SystemTime::now(), mtime: SystemTime::now(),
            ctime: SystemTime::now(), crtime: SystemTime::now(), kind, perm: 0o755, nlink: 2,
            uid: 1000, gid: 1000, rdev: 0, blksize: 4096, flags: 0,
        };

        // 1. Root Directory
        if parent == 1 {
            if name_str == "." || name_str == ".." {
                reply.entry(&ttl, &mk_attr(1, fuser::FileType::Directory), 0); return;
            }
            if name_str == ".magic" {
                reply.entry(&ttl, &mk_attr(2, fuser::FileType::Directory), 0); return;
            }
            if name_str == "search" {
                reply.entry(&ttl, &mk_attr(3, fuser::FileType::Directory), 0); return;
            }
        }

        // 2. .magic Directory
        if parent == 2 {
            if name_str == "." || name_str == ".." {
                reply.entry(&ttl, &mk_attr(2, fuser::FileType::Directory), 0); return;
            }
            if name_str == "refresh" {
                let mut attr = mk_attr(4, fuser::FileType::RegularFile);
                attr.size = 0; attr.perm = 0o666;
                reply.entry(&ttl, &attr, 0); return;
            }
        }

        // 3. Search Directory (The Query Router)
        if parent == 3 {
            if name_str == "." || name_str == ".." {
                reply.entry(&ttl, &mk_attr(3, fuser::FileType::Directory), 0); return;
            }
            
            let query = name_str.to_string();
            let state_guard = self.state.read().unwrap();
            let inode = state_guard.inode_store.get_or_create_inode(&query);
            let ready = state_guard.inode_store.has_results(inode);
            drop(state_guard);

            if ready {
                reply.entry(&ttl, &mk_attr(inode, fuser::FileType::Directory), 0);
            } else {
                reply.error(libc::EAGAIN);
            }
            return;
        }

        // 4. Dynamic Search Results
        if parent > 4 {
            let state_guard = self.state.read().unwrap();
            if state_guard.inode_store.get_query(parent).is_some() {
                let file_inode = state_guard.inode_store.hash_to_inode(&format!("{}-{}", parent, name_str));
                reply.entry(&ttl, &mk_attr(file_inode, fuser::FileType::RegularFile), 0);
                return;
            }
        }

        reply.error(libc::ENOENT);
    }

    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        let ttl = std::time::Duration::from_secs(1);
        
        let mut attr = fuser::FileAttr {
            ino, size: 4096, blocks: 8, atime: SystemTime::now(), mtime: SystemTime::now(),
            ctime: SystemTime::now(), crtime: SystemTime::now(), kind: fuser::FileType::Directory,
            perm: 0o755, nlink: 2, uid: 1000, gid: 1000, rdev: 0, blksize: 4096, flags: 0,
        };

        match ino {
            1 => { /* Root */ },
            2 => { /* .magic */ },
            3 => { /* search */ },
            4 => { 
                attr.kind = fuser::FileType::RegularFile; 
                attr.size = 0; 
                attr.perm = 0o666;
                attr.nlink = 1;
            },
            _ => {
                let is_search_dir = {
                    let state_guard = self.state.read().unwrap();
                    state_guard.inode_store.get_query(ino).is_some()
                };

                if !is_search_dir {
                    // Passthrough Metadata
                    attr.kind = fuser::FileType::RegularFile;
                    attr.nlink = 1;
                    attr.perm = 0o644;
                    
                    if let Some(real_path) = self.find_real_path(ino) {
                        if let Ok(meta) = std::fs::metadata(&real_path) {
                            attr.size = meta.len();
                            attr.blocks = (attr.size + 511) / 512;
                            if let Ok(m) = meta.modified() { attr.mtime = m; }
                            if let Ok(a) = meta.accessed() { attr.atime = a; }
                            if let Ok(c) = meta.created() { attr.crtime = c; }
                        } else {
                             tracing::warn!("[HollowDrive] getattr: Underlying file missing: {}", real_path);
                        }
                    } else {
                        attr.size = 1024;
                    }
                }
            }
        }
        reply.attr(&ttl, &attr);
    }

    fn setattr(
        &mut self,
        _req: &Request,
        ino: u64,
        _mode: Option<u32>,
        _uid: Option<u32>,
        _gid: Option<u32>,
        size: Option<u64>,
        _atime: Option<fuser::TimeOrNow>, // FIX: Renamed to _atime
        mtime: Option<fuser::TimeOrNow>,
        _ctime: Option<SystemTime>,
        _fh: Option<u64>,
        _crtime: Option<SystemTime>,
        _chgtime: Option<SystemTime>,
        _bkuptime: Option<SystemTime>,
        _flags: Option<u32>,
        reply: ReplyAttr
    ) {
        // Handle Refresh Button (Size 0)
        if ino == 4 {
            let state_guard = self.state.read().unwrap();
            state_guard.refresh_signal.store(true, std::sync::atomic::Ordering::Relaxed);
            
            let attr = fuser::FileAttr {
                ino: 4, size: 0, blocks: 0, atime: SystemTime::now(),
                mtime: SystemTime::now(), ctime: SystemTime::now(),
                crtime: SystemTime::now(), kind: fuser::FileType::RegularFile,
                perm: 0o666, nlink: 1, uid: 1000, gid: 1000, rdev: 0, blksize: 4096, flags: 0,
            };
            reply.attr(&std::time::Duration::from_secs(1), &attr);
            return;
        }

        // Passthrough Setattr
        if let Some(real_path) = self.find_real_path(ino) {
            let file = match OpenOptions::new().write(true).open(&real_path) {
                Ok(f) => f,
                Err(e) => {
                    tracing::error!("[HollowDrive] setattr open failed for {}: {}", real_path, e);
                    reply.error(libc::EACCES);
                    return;
                }
            };

            // Handle Truncate
            if let Some(new_size) = size {
                if let Err(e) = file.set_len(new_size) {
                    tracing::error!("[HollowDrive] setattr truncate failed: {}", e);
                    reply.error(libc::EIO);
                    return;
                }
            }

            // Handle Times (mtime)
            // We ignore atime requests because standard Rust doesn't support setting it easily,
            // and mtime is what matters for editor save detection.
            if let Some(mtime_val) = mtime {
                let new_mtime = match mtime_val {
                    fuser::TimeOrNow::SpecificTime(t) => t,
                    fuser::TimeOrNow::Now => SystemTime::now(),
                };
                let _ = file.set_modified(new_mtime);
            }
            
            // Construct result attr
            if let Ok(meta) = std::fs::metadata(&real_path) {
                let mut attr = fuser::FileAttr {
                    ino, size: meta.len(), blocks: (meta.len() + 511)/512, 
                    atime: SystemTime::now(), mtime: SystemTime::now(),
                    ctime: SystemTime::now(), crtime: SystemTime::now(), kind: fuser::FileType::RegularFile,
                    perm: 0o644, nlink: 1, uid: 1000, gid: 1000, rdev: 0, blksize: 4096, flags: 0,
                };
                if let Ok(m) = meta.modified() { attr.mtime = m; }
                if let Ok(a) = meta.accessed() { attr.atime = a; }
                reply.attr(&std::time::Duration::from_secs(1), &attr);
            } else {
                reply.error(libc::EIO);
            }
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

        // 1. Root
        if ino == 1 {
            let mut root_entries = entries.clone();
            root_entries.extend_from_slice(&[
                (2, FileType::Directory, ".magic".to_string()),
                (3, FileType::Directory, "search".to_string()),
            ]);
            for (i, (ino, kind, name)) in root_entries.iter().enumerate().skip(offset as usize) {
                if reply.add(*ino, (i+1) as i64, *kind, name) { break; }
            }
            reply.ok(); return;
        }

        // 2. .magic
        if ino == 2 {
            let mut items = entries.clone();
            items.push((4, FileType::RegularFile, "refresh".to_string()));
            for (i, (ino, kind, name)) in items.iter().enumerate().skip(offset as usize) {
                if reply.add(*ino, (i+1) as i64, *kind, name) { break; }
            }
            reply.ok(); return;
        }

        // 3. Search Root
        if ino == 3 {
            let mut items = entries.clone();
            let state_guard = self.state.read().unwrap();
            
            for (search_inode, query) in state_guard.inode_store.active_queries() {
                items.push((search_inode, FileType::Directory, query.clone()));
            }
            drop(state_guard);
            
            for (i, (ino, kind, name)) in items.iter().enumerate().skip(offset as usize) {
                if reply.add(*ino, (i+1) as i64, *kind, name) { break; }
            }
            reply.ok(); return;
        }

        // 4. Search Results
        {
            let state_guard = self.state.read().unwrap();
            if let Some(results) = state_guard.inode_store.get_results(ino) {
                 let mut items = entries.clone();
                 
                 for result in results {
                    let score_str = format!("{:.2}", result.score);
                    let filename = format!("{}_{}", score_str, result.filename);
                    let file_inode = state_guard.inode_store.hash_to_inode(&format!("{}-{}", ino, &filename));
                    items.push((file_inode, FileType::RegularFile, filename));
                 }
                 
                 for (i, (ino, kind, name)) in items.iter().enumerate().skip(offset as usize) {
                    if reply.add(*ino, (i+1) as i64, *kind, name) { break; }
                 }
                 reply.ok(); return;
            }
        }
        
        // Default empty directory
        for (i, (ino, kind, name)) in entries.iter().enumerate().skip(offset as usize) {
            if reply.add(*ino, (i+1) as i64, *kind, name) { break; }
        }
        reply.ok();
    }

    fn statfs(&mut self, _req: &Request, _ino: u64, reply: ReplyStatfs) {
        reply.statfs(4096, 4096, 0, 0, 0, 0, 0, 255);
    }

    fn open(&mut self, _req: &Request, ino: u64, flags: i32, reply: ReplyOpen) {
        if ino == 4 { reply.opened(0, 0); return; }
        
        if let Some(real_path) = self.find_real_path(ino) {
            tracing::debug!("[HollowDrive] Opened virtual file -> {}", real_path);
            reply.opened(0, flags as u32);
            return;
        }
        reply.error(libc::ENOENT);
    }

    fn read(&mut self, _req: &Request, ino: u64, _fh: u64, offset: i64, size: u32, _flags: i32, _lock_owner: Option<u64>, reply: ReplyData) {
        if ino == 4 { reply.data(&[]); return; }

        if let Some(real_path) = self.find_real_path(ino) {
            match File::open(&real_path) {
                Ok(file) => {
                    let mut buffer = vec![0u8; size as usize];
                    match file.read_at(&mut buffer, offset as u64) {
                        Ok(bytes) => reply.data(&buffer[..bytes]),
                        Err(e) => {
                            tracing::error!("[HollowDrive] read error: {}", e);
                            reply.error(libc::EIO);
                        }
                    }
                },
                Err(e) => {
                    let err = match e.kind() {
                        std::io::ErrorKind::NotFound => libc::ENOENT,
                        std::io::ErrorKind::PermissionDenied => libc::EACCES,
                        _ => libc::EIO,
                    };
                    reply.error(err);
                }
            }
            return;
        }
        reply.error(libc::ENOENT);
    }

    fn write(&mut self, _req: &Request, ino: u64, _fh: u64, offset: i64, data: &[u8], _write_flags: u32, _flags: i32, _lock_owner: Option<u64>, reply: ReplyWrite) {
         if ino == 4 { reply.error(libc::EACCES); return; }

         if let Some(real_path) = self.find_real_path(ino) {
            match OpenOptions::new().write(true).open(&real_path) {
                Ok(file) => {
                    match file.write_at(data, offset as u64) {
                        Ok(bytes) => {
                            tracing::debug!("[HollowDrive] wrote {} bytes to {}", bytes, real_path);
                            reply.written(bytes as u32);
                        },
                        Err(e) => {
                            tracing::error!("[HollowDrive] write error: {}", e);
                            reply.error(libc::EIO);
                        }
                    }
                },
                Err(e) => {
                    tracing::warn!("[HollowDrive] write open failed: {}", e);
                    reply.error(libc::EACCES);
                }
            }
            return;
         }
         reply.error(libc::ENOENT);
    }
}
