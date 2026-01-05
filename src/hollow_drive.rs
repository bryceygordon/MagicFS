// FILE: src/hollow_drive.rs
//! Hollow Drive: The Synchronous FUSE Loop (The Face)

use fuser::{Filesystem, ReplyEntry, ReplyAttr, ReplyDirectory, ReplyOpen, ReplyData, ReplyWrite, Request};
use crate::state::{SharedState, SearchWaiter};
use crate::core::bouncer::Bouncer;
use crate::core::inode_store::InodeStore;
use std::fs::{File, OpenOptions};
use std::os::unix::fs::FileExt;
use std::time::{SystemTime, Duration};
use std::path::Path;
use std::sync::Arc;
use libc;
use rusqlite::params; // Added for DB queries

// --- INODE CONSTANTS ---
const INODE_ROOT: u64 = 1;
const INODE_MAGIC: u64 = 2;
const INODE_SEARCH: u64 = 3;
const INODE_REFRESH: u64 = 4;
const INODE_MIRROR: u64 = 5;
const INODE_TAGS: u64 = 6;
const INODE_INBOX: u64 = 7;

pub struct HollowDrive {
    pub state: SharedState,
}

impl HollowDrive {
    pub fn new(state: SharedState) -> Self {
        Self { state }
    }

    fn find_real_path(&self, target_inode: u64) -> Option<String> {
        // Allow looking up files that might generate high hash inodes
        if target_inode <= 50 { return None; } 

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
        
        // 3. Persistent Files (Tag View)
        // If the inode matches a real file in our DB, return it.
        // We can check the DB directly.
        let mut conn_lock = state_guard.db_connection.lock().ok()?;
        if let Some(conn) = conn_lock.as_mut() {
             let mut stmt = conn.prepare("SELECT abs_path FROM file_registry WHERE inode = ?1").ok()?;
             let path: Result<String, _> = stmt.query_row([target_inode], |r| r.get(0));
             if let Ok(p) = path {
                 return Some(p);
             }
        }

        None
    }

    fn sanitize_query(name: &str) -> String {
        let trimmed = name.trim();
        if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
            return trimmed[1..trimmed.len()-1].to_string();
        }
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

        let name_str_owned = Self::sanitize_query(raw_name);
        let name_str = name_str_owned.as_str();

        let stable_time = self.state.read().unwrap().start_time;
        let ttl = Duration::from_secs(1);
        
        // Helper to create attributes
        let mk_attr = |ino: u64, kind: fuser::FileType, perm: u16| fuser::FileAttr {
            ino, size: 4096, blocks: 8, 
            atime: stable_time, mtime: stable_time, ctime: stable_time, crtime: stable_time, 
            kind, perm, nlink: 2,
            uid: 1000, gid: 1000, rdev: 0, blksize: 4096, flags: 0,
        };

        // 1. Root Directory
        if parent == INODE_ROOT {
            match name_str {
                "." | ".." => reply.entry(&ttl, &mk_attr(INODE_ROOT, fuser::FileType::Directory, 0o755), 0),
                ".magic" => reply.entry(&ttl, &mk_attr(INODE_MAGIC, fuser::FileType::Directory, 0o755), 0),
                "search" => reply.entry(&ttl, &mk_attr(INODE_SEARCH, fuser::FileType::Directory, 0o555), 0),
                "mirror" => reply.entry(&ttl, &mk_attr(INODE_MIRROR, fuser::FileType::Directory, 0o755), 0),
                _ => reply.error(libc::ENOENT),
            }
            return;
        }

        // 2. .magic
        if parent == INODE_MAGIC { 
             match name_str {
                "." | ".." => reply.entry(&ttl, &mk_attr(INODE_MAGIC, fuser::FileType::Directory, 0o755), 0),
                "refresh" => {
                    let mut attr = mk_attr(INODE_REFRESH, fuser::FileType::RegularFile, 0o666);
                    attr.size = 0; 
                    reply.entry(&ttl, &attr, 0); 
                },
                "tags" => reply.entry(&ttl, &mk_attr(INODE_TAGS, fuser::FileType::Directory, 0o555), 0),
                "inbox" => reply.entry(&ttl, &mk_attr(INODE_INBOX, fuser::FileType::Directory, 0o555), 0),
                _ => reply.error(libc::ENOENT),
            }
            return;
        }

        // 3. Search Root
        if parent == INODE_SEARCH {
            match name_str {
                "." | ".." => reply.entry(&ttl, &mk_attr(INODE_SEARCH, fuser::FileType::Directory, 0o555), 0),
                _ => {
                    if Bouncer::is_noise(name_str) {
                        reply.error(libc::ENOENT);
                        return;
                    }
                    let query = name_str.to_string();
                    let state_guard = self.state.read().unwrap();
                    let inode = state_guard.inode_store.get_or_create_inode(&query);
                    drop(state_guard);
                    reply.entry(&ttl, &mk_attr(inode, fuser::FileType::Directory, 0o555), 0);
                }
            }
            return;
        }

        // 4. Tags Root
        if parent == INODE_TAGS {
            match name_str {
                "." | ".." => reply.entry(&ttl, &mk_attr(INODE_TAGS, fuser::FileType::Directory, 0o555), 0),
                _ => {
                    // Check DB for tag existence
                    let state_guard = self.state.read().unwrap();
                    let mut conn_lock = state_guard.db_connection.lock().unwrap();
                    
                    if let Some(conn) = conn_lock.as_mut() {
                        let mut stmt = conn.prepare("SELECT tag_id FROM tags WHERE name = ?1").unwrap();
                        let tag_id_res: std::result::Result<u64, _> = stmt.query_row([name_str], |r| r.get(0));

                        if let Ok(tag_id) = tag_id_res {
                            let persistent_inode = InodeStore::db_id_to_inode(tag_id);
                            reply.entry(&ttl, &mk_attr(persistent_inode, fuser::FileType::Directory, 0o555), 0);
                            return;
                        }
                    }
                    reply.error(libc::ENOENT);
                }
            }
            return;
        }
        
        // 5. Inbox Root
        if parent == INODE_INBOX {
             match name_str {
                "." | ".." => reply.entry(&ttl, &mk_attr(INODE_INBOX, fuser::FileType::Directory, 0o555), 0),
                _ => reply.error(libc::ENOENT),
             }
             return;
        }

        // 6. Mirror Root
        if parent == INODE_MIRROR {
             if name_str == "." || name_str == ".." {
                reply.entry(&ttl, &mk_attr(INODE_MIRROR, fuser::FileType::Directory, 0o755), 0); return;
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

        // 7. Dynamic Content (Search Results, Mirror Children, OR PERSISTENT TAGS)
        if parent >= 100 {
            // A. IS IT A TAG? (Persistent Inode)
            if InodeStore::is_persistent(parent) {
                let tag_id = InodeStore::inode_to_db_id(parent);
                
                let state_guard = self.state.read().unwrap();
                let mut conn_lock = state_guard.db_connection.lock().unwrap();
                
                if let Some(conn) = conn_lock.as_mut() {
                    // Look for file inside this tag
                    let sql = "
                        SELECT f.inode, f.size, f.mtime, f.is_dir 
                        FROM file_tags ft 
                        JOIN file_registry f ON ft.file_id = f.file_id 
                        WHERE ft.tag_id = ?1 AND ft.display_name = ?2
                    ";
                    
                    if let Ok(mut stmt) = conn.prepare(sql) {
                        let row_res = stmt.query_row(params![tag_id, name_str], |row| {
                            Ok((
                                row.get::<_, u64>(0)?, // inode
                                row.get::<_, u64>(1)?, // size
                                row.get::<_, u64>(2)?, // mtime
                                row.get::<_, i32>(3)?  // is_dir
                            ))
                        });

                        if let Ok((inode, size, mtime, is_dir_int)) = row_res {
                            let kind = if is_dir_int != 0 { fuser::FileType::Directory } else { fuser::FileType::RegularFile };
                            let perm = if is_dir_int != 0 { 0o755 } else { 0o644 };
                            let mut attr = mk_attr(inode, kind, perm);
                            attr.size = size;
                            attr.blocks = (size + 511)/512;
                            attr.mtime = std::time::UNIX_EPOCH + std::time::Duration::from_secs(mtime);
                            
                            reply.entry(&ttl, &attr, 0);
                            return;
                        }
                    }
                }
                reply.error(libc::ENOENT);
                return;
            }

            // B. STANDARD DYNAMIC CONTENT
            let state_guard = self.state.read().unwrap();
            let inode_store = &state_guard.inode_store;

            // Is it a Search Query Dir?
            if inode_store.get_query(parent).is_some() {
                drop(state_guard);
                // Trigger Lazy Search
                self.state.read().unwrap().inode_store.mark_active(parent);
                
                let state_guard = self.state.read().unwrap();
                let inode_store = &state_guard.inode_store;
                let file_inode = inode_store.hash_to_inode(&format!("{}-{}", parent, name_str));
                reply.entry(&ttl, &mk_attr(file_inode, fuser::FileType::RegularFile, 0o644), 0);
                return;
            }
            
            // Is it a Mirror Dir?
            if let Some(parent_path) = inode_store.get_mirror_path(parent) {
                let child_path = Path::new(&parent_path).join(name_str);
                let child_path_str = child_path.to_string_lossy().to_string();
                let child_inode = inode_store.hash_to_inode(&child_path_str);
                inode_store.put_mirror_path(child_inode, child_path_str);

                if let Ok(meta) = std::fs::metadata(&child_path) {
                    let kind = if meta.is_dir() { fuser::FileType::Directory } else { fuser::FileType::RegularFile };
                    let perm = if meta.is_dir() { 0o755 } else { 0o644 };
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
            INODE_ROOT => {}, 
            INODE_MAGIC => {}, 
            INODE_SEARCH => { attr.perm = 0o555; },
            INODE_REFRESH => { attr.kind = fuser::FileType::RegularFile; attr.size = 0; attr.perm = 0o666; attr.nlink = 1; }, 
            INODE_MIRROR => {}, 
            INODE_TAGS => { attr.perm = 0o555; },
            INODE_INBOX => { attr.perm = 0o555; },
            _ => {
                if InodeStore::is_persistent(ino) {
                    // Tag View
                     attr.perm = 0o555;
                } else {
                    let is_search_dir = self.state.read().unwrap().inode_store.get_query(ino).is_some();
                    if is_search_dir {
                        attr.perm = 0o555;
                    } else {
                        // Passthrough or Mirror
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
                            } else {
                                attr.kind = fuser::FileType::RegularFile; attr.size = 1024; attr.perm = 0o644; attr.nlink = 1;
                            }
                        }
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
        if ino == INODE_REFRESH {
             let state_guard = self.state.read().unwrap();
             state_guard.refresh_signal.store(true, std::sync::atomic::Ordering::Relaxed);
             let attr = fuser::FileAttr {
                 ino: INODE_REFRESH, size: 0, blocks: 0, atime: SystemTime::now(),
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

        if ino == INODE_ROOT {
            let mut root_entries = entries.clone();
            root_entries.extend_from_slice(&[
                (INODE_MAGIC, FileType::Directory, ".magic".to_string()),
                (INODE_SEARCH, FileType::Directory, "search".to_string()),
                (INODE_MIRROR, FileType::Directory, "mirror".to_string()),
            ]);
            for (i, (ino, kind, name)) in root_entries.iter().enumerate().skip(offset as usize) {
                if reply.add(*ino, (i+1) as i64, *kind, name) { break; }
            }
            reply.ok();
            return;
        }

        if ino == INODE_MAGIC {
            let mut items = entries.clone();
            items.extend_from_slice(&[
                (INODE_REFRESH, FileType::RegularFile, "refresh".to_string()),
                (INODE_TAGS, FileType::Directory, "tags".to_string()),
                (INODE_INBOX, FileType::Directory, "inbox".to_string()),
            ]);
            for (i, (ino, kind, name)) in items.iter().enumerate().skip(offset as usize) {
                if reply.add(*ino, (i+1) as i64, *kind, name) { break; }
            }
            reply.ok();
            return;
        }

        if ino == INODE_TAGS {
            let mut items = entries.clone();
            // DB Query for top-level tags
            let state_guard = self.state.read().unwrap();
            let mut conn_lock = state_guard.db_connection.lock().unwrap();
            
            if let Some(conn) = conn_lock.as_mut() {
                // Ignore error if table not ready, just empty list
                if let Ok(mut stmt) = conn.prepare("SELECT tag_id, name FROM tags WHERE parent_tag_id IS NULL") {
                     let rows = stmt.query_map([], |row| {
                        Ok((row.get::<_, u64>(0)?, row.get::<_, String>(1)?))
                    }).unwrap();
                    
                    for row in rows {
                        if let Ok((tag_id, name)) = row {
                            let persistent_inode = InodeStore::db_id_to_inode(tag_id);
                            items.push((persistent_inode, FileType::Directory, name));
                        }
                    }
                }
            }
            for (i, (ino, kind, name)) in items.iter().enumerate().skip(offset as usize) {
                if reply.add(*ino, (i+1) as i64, *kind, name) { break; }
            }
            reply.ok();
            return;
        }

        if ino == INODE_INBOX {
             // For now, Inbox is just empty
             for (i, (ino, kind, name)) in entries.iter().enumerate().skip(offset as usize) {
                if reply.add(*ino, (i+1) as i64, *kind, name) { break; }
            }
            reply.ok();
            return;
        }

        if ino == INODE_SEARCH {
             let mut items = entries.clone();
             let state_guard = self.state.read().unwrap();
             for (search_inode, query) in state_guard.inode_store.active_queries() {
                 items.push((search_inode, FileType::Directory, query.clone()));
             }
             drop(state_guard);
             for (i, (ino, kind, name)) in items.iter().enumerate().skip(offset as usize) {
                if reply.add(*ino, (i+1) as i64, *kind, name) { break; }
            }
             reply.ok();
             return;
        }

        if ino == INODE_MIRROR { 
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
            reply.ok();
            return;
        }

        // Search Results & Mirror Subdirs & PERSISTENT TAGS
        if InodeStore::is_persistent(ino) {
             let tag_id = InodeStore::inode_to_db_id(ino);
             let mut items = entries.clone();

             let state_guard = self.state.read().unwrap();
             let mut conn_lock = state_guard.db_connection.lock().unwrap();
             
             if let Some(conn) = conn_lock.as_mut() {
                 let sql = "
                    SELECT f.inode, f.is_dir, ft.display_name 
                    FROM file_tags ft 
                    JOIN file_registry f ON ft.file_id = f.file_id 
                    WHERE ft.tag_id = ?1
                 ";
                 if let Ok(mut stmt) = conn.prepare(sql) {
                     let rows = stmt.query_map(params![tag_id], |row| {
                         Ok((
                             row.get::<_, u64>(0)?, 
                             row.get::<_, i32>(1)?, 
                             row.get::<_, String>(2)?
                         ))
                     }).unwrap();
                     
                     for row in rows {
                         if let Ok((inode, is_dir_int, name)) = row {
                             let kind = if is_dir_int != 0 { FileType::Directory } else { FileType::RegularFile };
                             items.push((inode, kind, name));
                         }
                     }
                 }
             }
             
             for (i, (ino, kind, name)) in items.iter().enumerate().skip(offset as usize) {
                if reply.add(*ino, (i+1) as i64, *kind, name) { break; }
            }
            reply.ok();
            return;
        }

        // Dynamic Content (Search)
        if !InodeStore::is_persistent(ino) {
            let is_query_dir = {
                let state_guard = self.state.read().unwrap();
                state_guard.inode_store.get_query(ino).is_some()
            };

            if is_query_dir {
                // TRIGGER LAZY SEARCH
                self.state.read().unwrap().inode_store.mark_active(ino);
                
                let state_guard = self.state.read().unwrap();
                if !state_guard.inode_store.has_results(ino) {
                    drop(state_guard);
                    let waiter = Arc::new(SearchWaiter::new());
                    {
                        let sg = self.state.read().unwrap();
                        let mut waiters = sg.search_waiters.lock().unwrap();
                        waiters.insert(ino, waiter.clone());
                    }
                    let finished = waiter.finished.lock().unwrap();
                    if !*finished {
                        let _result = waiter.cvar.wait_timeout(finished, Duration::from_millis(1000)).unwrap();
                    }
                }
                
                let state_guard = self.state.read().unwrap();
                let mut items = entries.clone();
                if let Some(results) = state_guard.inode_store.get_results(ino) {
                    for result in results {
                        let score_str = format!("{:.2}", result.score);
                        let filename = format!("{}_{}", score_str, result.filename);
                        let file_inode = state_guard.inode_store.hash_to_inode(&format!("{}-{}", ino, &filename));
                        items.push((file_inode, FileType::RegularFile, filename));
                    }
                }
                
                drop(state_guard);
                for (i, (ino, kind, name)) in items.iter().enumerate().skip(offset as usize) {
                    if reply.add(*ino, (i+1) as i64, *kind, name) { break; }
                }
                reply.ok(); 
                return;
            }

            // Mirror Subdirs
            let state_guard = self.state.read().unwrap();
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
                reply.ok();
                return;
            }
        }

        reply.ok();
    }
    
    fn open(&mut self, _req: &Request, ino: u64, flags: i32, reply: ReplyOpen) {
        if ino == INODE_REFRESH { reply.opened(0, 0); return; }
        if self.find_real_path(ino).is_some() {
            reply.opened(0, flags as u32); return;
        }
        reply.error(libc::ENOENT);
    }

    fn read(&mut self, _req: &Request, ino: u64, _fh: u64, offset: i64, size: u32, _flags: i32, _lock_owner: Option<u64>, reply: ReplyData) {
        if ino == INODE_REFRESH { reply.data(&[]); return; }
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
         if ino == INODE_REFRESH { reply.error(libc::EACCES); return; }
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
