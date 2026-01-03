// FILE: src/hollow_drive.rs
//! Hollow Drive: The Synchronous FUSE Loop (The Face)

use fuser::{Filesystem, ReplyEntry, ReplyAttr, ReplyDirectory, ReplyStatfs, ReplyOpen, ReplyData, ReplyWrite, Request};
use crate::state::{SharedState, SearchWaiter};
use std::fs::{File, OpenOptions};
use std::os::unix::fs::FileExt;
use std::time::{SystemTime, Duration};
use std::path::Path;
use std::sync::Arc;
use libc;

pub struct HollowDrive {
    pub state: SharedState,
}

impl HollowDrive {
    pub fn new(state: SharedState) -> Self {
        Self { state }
    }

    fn find_real_path(&self, target_inode: u64) -> Option<String> {
        if target_inode <= 5 { return None; } 

        let state_guard = self.state.read().ok()?;
        let inode_store = &state_guard.inode_store;

        // 1. Mirror Cache
        if let Some(path) = inode_store.get_mirror_path(target_inode) {
            return Some(path);
        }

        // 2. Search Results
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

    // NEW HELPER: Clean up shell artifacts
    fn sanitize_query(name: &str) -> String {
        let trimmed = name.trim();
        // Strip surrounding double quotes
        if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
            return trimmed[1..trimmed.len()-1].to_string();
        }
        // Strip surrounding single quotes
        if trimmed.starts_with('\'') && trimmed.ends_with('\'') && trimmed.len() >= 2 {
            return trimmed[1..trimmed.len()-1].to_string();
        }
        trimmed.to_string()
    }
}

impl Filesystem for HollowDrive {
    fn init(&mut self, _req: &Request, _config: &mut fuser::KernelConfig) -> std::result::Result<(), i32> {
        tracing::info!("[HollowDrive] FUSE initialized");
        Ok(())
    }

    fn lookup(&mut self, _req: &Request, parent: u64, name: &std::ffi::OsStr, reply: ReplyEntry) {
        let raw_name = match name.to_str() {
            Some(s) => s,
            None => { reply.error(libc::EINVAL); return; }
        };

        // APPLY SANITIZATION IMMEDIATELY
        let name_str_owned = Self::sanitize_query(raw_name);
        let name_str = name_str_owned.as_str();

        let stable_time = self.state.read().unwrap().start_time;
        let ttl = Duration::from_secs(1);
        
        let mk_attr = |ino: u64, kind: fuser::FileType, perm: u16| fuser::FileAttr {
            ino, size: 4096, blocks: 8, 
            atime: stable_time, mtime: stable_time, ctime: stable_time, crtime: stable_time, 
            kind, perm, nlink: 2,
            uid: 1000, gid: 1000, rdev: 0, blksize: 4096, flags: 0,
        };

        // 1. Root Directory
        if parent == 1 {
            match name_str {
                "." | ".." => reply.entry(&ttl, &mk_attr(1, fuser::FileType::Directory, 0o755), 0),
                ".magic" => reply.entry(&ttl, &mk_attr(2, fuser::FileType::Directory, 0o755), 0),
                "search" => reply.entry(&ttl, &mk_attr(3, fuser::FileType::Directory, 0o555), 0),
                "mirror" => reply.entry(&ttl, &mk_attr(5, fuser::FileType::Directory, 0o755), 0),
                _ => reply.error(libc::ENOENT),
            }
            return;
        }

        // 2. .magic
        if parent == 2 { 
            if name_str == "refresh" {
                let mut attr = mk_attr(4, fuser::FileType::RegularFile, 0o666);
                attr.size = 0; 
                reply.entry(&ttl, &attr, 0); return;
            }
             if name_str == "." || name_str == ".." {
                reply.entry(&ttl, &mk_attr(2, fuser::FileType::Directory, 0o755), 0); return;
            }
            reply.error(libc::ENOENT); return;
        }

        // 3. Search Root
        if parent == 3 {
            match name_str {
                "." | ".." => reply.entry(&ttl, &mk_attr(3, fuser::FileType::Directory, 0o555), 0),
                _ => {
                    // --- A. THE BOUNCER ---
                    if name_str.starts_with(".") || 
                       name_str.ends_with(".zip") || 
                       name_str.ends_with(".tar") ||
                       name_str.ends_with(".gz") ||
                       name_str.ends_with(".ini") ||
                       name_str.ends_with(".desktop") ||
                       name_str.starts_with("New Folder") {
                        tracing::debug!("[HollowDrive] Bouncer rejected: {}", name_str);
                        reply.error(libc::ENOENT);
                        return;
                    }

                    // --- B. THE EPHEMERAL PROMISE ---
                    let query = name_str.to_string();
                    let state_guard = self.state.read().unwrap();
                    let inode = state_guard.inode_store.get_or_create_inode(&query);
                    drop(state_guard);

                    // Return with STABLE time
                    reply.entry(&ttl, &mk_attr(inode, fuser::FileType::Directory, 0o555), 0);
                }
            }
            return;
        }

        // 4. Mirror Root (Inode 5)
        if parent == 5 {
             if name_str == "." || name_str == ".." {
                reply.entry(&ttl, &mk_attr(5, fuser::FileType::Directory, 0o755), 0); return;
            }
            let state_guard = self.state.read().unwrap();
            let wp_guard = state_guard.watch_paths.lock().unwrap();
            
            for path_str in wp_guard.iter() {
                let path = Path::new(path_str);
                if let Some(filename) = path.file_name() {
                    if filename.to_str() == Some(name_str) {
                        let inode = state_guard.inode_store.hash_to_inode(path_str);
                        state_guard.inode_store.put_mirror_path(inode, path_str.clone());
                        reply.entry(&ttl, &mk_attr(inode, fuser::FileType::Directory, 0o755), 0);
                        return;
                    }
                }
            }
            reply.error(libc::ENOENT); return;
        }

        // 5. Dynamic Content
        if parent > 5 {
            let state_guard = self.state.read().unwrap();
            let inode_store = &state_guard.inode_store;

            // Search Query -> Result File
            if inode_store.get_query(parent).is_some() {
                let file_inode = inode_store.hash_to_inode(&format!("{}-{}", parent, name_str));
                // Files are Read-Write (Passthrough)
                reply.entry(&ttl, &mk_attr(file_inode, fuser::FileType::RegularFile, 0o644), 0);
                return;
            }
            
            // Mirror Path -> Child
            if let Some(parent_path) = inode_store.get_mirror_path(parent) {
                let child_path = Path::new(&parent_path).join(name_str);
                let child_path_str = child_path.to_string_lossy().to_string();
                let child_inode = inode_store.hash_to_inode(&child_path_str);
                inode_store.put_mirror_path(child_inode, child_path_str);

                if let Ok(meta) = std::fs::metadata(&child_path) {
                    let kind = if meta.is_dir() { fuser::FileType::Directory } else { fuser::FileType::RegularFile };
                    let perm = if meta.is_dir() { 0o755 } else { 0o644 };
                    
                    // REAL files get REAL timestamps
                    let mut attr = mk_attr(child_inode, kind, perm);
                    attr.size = meta.len();
                    attr.blocks = (attr.size + 511)/512;
                    if let Ok(m) = meta.modified() { attr.mtime = m; }
                    if let Ok(a) = meta.accessed() { attr.atime = a; }
                    if let Ok(c) = meta.created() { attr.crtime = c; }
                    
                    reply.entry(&ttl, &attr, 0);
                } else {
                    reply.error(libc::ENOENT);
                }
                return;
            }
        }

        reply.error(libc::ENOENT);
    }

    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        let stable_time = self.state.read().unwrap().start_time;
        let ttl = Duration::from_secs(1);
        
        let mut attr = fuser::FileAttr {
            ino, size: 4096, blocks: 8, 
            atime: stable_time, mtime: stable_time, ctime: stable_time, crtime: stable_time, 
            kind: fuser::FileType::Directory,
            perm: 0o755, nlink: 2, uid: 1000, gid: 1000, rdev: 0, blksize: 4096, flags: 0,
        };

        match ino {
            1 => {}, // Root (Stable)
            2 => {}, // .magic (Stable)
            3 => { 
                attr.perm = 0o555; // Search Root (Stable, Read-Only)
            },
            4 => { attr.kind = fuser::FileType::RegularFile; attr.size = 0; attr.perm = 0o666; attr.nlink = 1; }, 
            5 => {}, // Mirror (Stable)
            _ => {
                let is_search_dir = self.state.read().unwrap().inode_store.get_query(ino).is_some();
                if is_search_dir {
                    // Search Query Dirs are STABLE and Read-Only
                    attr.perm = 0o555;
                } else {
                    // Everything else is a Real File (or Search Result File)
                    // These need REAL timestamps to detect changes
                    if let Some(real_path) = self.find_real_path(ino) {
                        if let Ok(meta) = std::fs::metadata(&real_path) {
                            attr.size = meta.len();
                            attr.blocks = (attr.size + 511) / 512;
                            attr.perm = if meta.is_dir() { 0o755 } else { 0o644 };
                            attr.nlink = if meta.is_dir() { 2 } else { 1 };
                            attr.kind = if meta.is_dir() { fuser::FileType::Directory } else { fuser::FileType::RegularFile };
                            if let Ok(m) = meta.modified() { attr.mtime = m; }
                            if let Ok(a) = meta.accessed() { attr.atime = a; }
                            if let Ok(c) = meta.created() { attr.crtime = c; }
                        }
                    } else {
                        // Fallback for search results not yet mapped?
                        attr.kind = fuser::FileType::RegularFile; attr.size = 1024; attr.perm = 0o644; attr.nlink = 1;
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
        _atime: Option<fuser::TimeOrNow>,
        mtime: Option<fuser::TimeOrNow>,
        _ctime: Option<SystemTime>,
        _fh: Option<u64>,
        _crtime: Option<SystemTime>,
        _chgtime: Option<SystemTime>,
        _bkuptime: Option<SystemTime>,
        _flags: Option<u32>,
        reply: ReplyAttr
    ) {
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

        if let Some(real_path) = self.find_real_path(ino) {
            let file = match OpenOptions::new().write(true).open(&real_path) {
                Ok(f) => f,
                Err(_) => { reply.error(libc::EACCES); return; }
            };
            if let Some(new_size) = size { let _ = file.set_len(new_size); }
            if let Some(mtime_val) = mtime {
                let new_mtime = match mtime_val {
                    fuser::TimeOrNow::SpecificTime(t) => t,
                    fuser::TimeOrNow::Now => SystemTime::now(),
                };
                let _ = file.set_modified(new_mtime);
            }
            if let Ok(meta) = std::fs::metadata(&real_path) {
                 let mut attr = fuser::FileAttr {
                    ino, size: meta.len(), blocks: (meta.len() + 511)/512, 
                    atime: SystemTime::now(), mtime: SystemTime::now(),
                    ctime: SystemTime::now(), crtime: SystemTime::now(), kind: fuser::FileType::RegularFile,
                    perm: 0o644, nlink: 1, uid: 1000, gid: 1000, rdev: 0, blksize: 4096, flags: 0,
                };
                if let Ok(m) = meta.modified() { attr.mtime = m; }
                reply.attr(&std::time::Duration::from_secs(1), &attr);
            } else { reply.error(libc::EIO); }
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
                (5, FileType::Directory, "mirror".to_string()),
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

        // 4. Mirror Root
        if ino == 5 { 
            let mut items = entries.clone();
            let paths_to_list: Vec<(u64, String)> = {
                let state_guard = self.state.read().unwrap();
                let wp_guard = state_guard.watch_paths.lock().unwrap();
                let mut list = Vec::new();
                for path_str in wp_guard.iter() {
                    let path = Path::new(path_str);
                    if let Some(name) = path.file_name() {
                        let name_str = name.to_string_lossy().to_string();
                        let inode = state_guard.inode_store.hash_to_inode(path_str);
                        state_guard.inode_store.put_mirror_path(inode, path_str.clone());
                        list.push((inode, name_str));
                    }
                }
                list
            };
            for (inode, name_str) in paths_to_list { items.push((inode, FileType::Directory, name_str)); }
            for (i, (ino, kind, name)) in items.iter().enumerate().skip(offset as usize) {
                if reply.add(*ino, (i+1) as i64, *kind, name) { break; }
            }
            reply.ok(); return;
        }

        // 5. Dynamic: Search Results OR Mirror Subdirs
        let state_guard = self.state.read().unwrap();
        
        // --- A. Search Results (SMART WAITER) ---
        if state_guard.inode_store.get_query(ino).is_some() {
            if !state_guard.inode_store.has_results(ino) {
                drop(state_guard); // Release lock before waiting

                let waiter = Arc::new(SearchWaiter::new());
                {
                    let sg = self.state.read().unwrap();
                    let mut waiters = sg.search_waiters.lock().unwrap();
                    waiters.insert(ino, waiter.clone());
                }

                let mut finished = waiter.finished.lock().unwrap();
                if !*finished {
                    let _result = waiter.cvar.wait_timeout(finished, Duration::from_millis(1000)).unwrap();
                }
                
                let state_guard = self.state.read().unwrap();
                if let Some(results) = state_guard.inode_store.get_results(ino) {
                     let mut items = entries.clone();
                     for result in results {
                        let score_str = format!("{:.2}", result.score);
                        let filename = format!("{}_{}", score_str, result.filename);
                        let file_inode = state_guard.inode_store.hash_to_inode(&format!("{}-{}", ino, &filename));
                        items.push((file_inode, FileType::RegularFile, filename));
                     }
                     drop(state_guard);
                     for (i, (ino, kind, name)) in items.iter().enumerate().skip(offset as usize) {
                        if reply.add(*ino, (i+1) as i64, *kind, name) { break; }
                     }
                     reply.ok(); return;
                } else {
                    drop(state_guard);
                    for (i, (ino, kind, name)) in entries.iter().enumerate().skip(offset as usize) {
                        if reply.add(*ino, (i+1) as i64, *kind, name) { break; }
                    }
                    reply.ok(); return;
                }
            } else {
                if let Some(results) = state_guard.inode_store.get_results(ino) {
                     let mut items = entries.clone();
                     for result in results {
                        let score_str = format!("{:.2}", result.score);
                        let filename = format!("{}_{}", score_str, result.filename);
                        let file_inode = state_guard.inode_store.hash_to_inode(&format!("{}-{}", ino, &filename));
                        items.push((file_inode, FileType::RegularFile, filename));
                     }
                     drop(state_guard);
                     for (i, (ino, kind, name)) in items.iter().enumerate().skip(offset as usize) {
                        if reply.add(*ino, (i+1) as i64, *kind, name) { break; }
                     }
                     reply.ok(); return;
                }
            }
        }

        // --- B. Mirror Subdirs ---
        if let Some(real_path) = state_guard.inode_store.get_mirror_path(ino) {
            let inode_store = &state_guard.inode_store;
            let mut items = entries.clone();
            
            let mut found_entries = Vec::new();
            if let Ok(dir_entries) = std::fs::read_dir(&real_path) {
                for entry in dir_entries.flatten() {
                    let child_path = entry.path();
                    let child_path_str = child_path.to_string_lossy().to_string();
                    if let Ok(name) = entry.file_name().into_string() {
                        let kind = if child_path.is_dir() { FileType::Directory } else { FileType::RegularFile };
                        found_entries.push((child_path_str, kind, name));
                    }
                }
            }
            for (path_str, kind, name) in found_entries {
                let child_inode = inode_store.hash_to_inode(&path_str);
                inode_store.put_mirror_path(child_inode, path_str);
                items.push((child_inode, kind, name));
            }
            drop(state_guard);
            for (i, (ino, kind, name)) in items.iter().enumerate().skip(offset as usize) {
                if reply.add(*ino, (i+1) as i64, *kind, name) { break; }
            }
            reply.ok(); return;
        }

        reply.ok();
    }
    
    fn open(&mut self, _req: &Request, ino: u64, flags: i32, reply: ReplyOpen) {
        if ino == 4 { reply.opened(0, 0); return; }
        if let Some(real_path) = self.find_real_path(ino) {
            reply.opened(0, flags as u32); return;
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
                        Err(_) => reply.error(libc::EIO),
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
                        Ok(bytes) => reply.written(bytes as u32),
                        Err(_) => reply.error(libc::EIO),
                    }
                },
                Err(_) => reply.error(libc::EACCES),
            }
            return;
         }
         reply.error(libc::ENOENT);
    }
}
