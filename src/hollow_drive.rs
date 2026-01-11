// FILE: src/hollow_drive.rs
//! Hollow Drive: The Synchronous FUSE Loop (The Face)

use fuser::{Filesystem, FileType, ReplyEntry, ReplyAttr, ReplyDirectory, ReplyOpen, ReplyData, ReplyWrite, ReplyCreate, Request};
use crate::state::{SharedState, SearchWaiter};
use crate::core::bouncer::Bouncer;
use crate::core::inode_store::InodeStore;
use crate::storage::repository::Repository;
use crate::error::MagicError;
use std::fs::{File, OpenOptions};
use std::os::unix::fs::{FileExt, MetadataExt}; // Added MetadataExt for .ino()
use std::time::{SystemTime, Duration};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use libc;
use rusqlite::params;

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

    /// Helper to parse virtual filenames like "report (1).pdf" -> ("report.pdf", 1)
    fn parse_virtual_name(name: &str) -> Option<(String, usize)> {
        // 1. Find extension
        let path = Path::new(name);
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or(name);

        // 2. Check for " (N)" at the end of stem
        if stem.ends_with(')') {
            if let Some(open_idx) = stem.rfind(" (") {
                let number_part = &stem[open_idx + 2..stem.len() - 1];
                if let Ok(n) = number_part.parse::<usize>() {
                    let base_stem = &stem[..open_idx];
                    let base_name = if ext.is_empty() {
                        base_stem.to_string()
                    } else {
                        format!("{}.{}", base_stem, ext)
                    };
                    return Some((base_name, n));
                }
            }
        }
        None
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

    /// Helper to convert std::fs::Metadata to fuser::FileAttr
    /// Used in getattr for physical file passthrough
    fn metadata_to_attr(&self, meta: &std::fs::Metadata, inode: u64) -> fuser::FileAttr {
        fuser::FileAttr {
            ino: inode,
            size: meta.len(),
            blocks: (meta.len() + 511) / 512,
            atime: meta.accessed().unwrap_or(std::time::SystemTime::now()),
            mtime: meta.modified().unwrap_or(std::time::SystemTime::now()),
            ctime: meta.created().unwrap_or(std::time::SystemTime::now()),
            crtime: meta.created().unwrap_or(std::time::SystemTime::now()),
            kind: if meta.is_dir() { fuser::FileType::Directory } else { fuser::FileType::RegularFile },
            perm: if meta.is_dir() { 0o755 } else { 0o644 },
            nlink: if meta.is_dir() { 2 } else { 1 },
            uid: 0, // Will be overwritten by getattr masquerade
            gid: 0, // Will be overwritten by getattr masquerade
            rdev: 0,
            flags: 0,
            blksize: 512,
        }
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
            uid: 0, gid: 0, rdev: 0, blksize: 4096, flags: 0,
        };

        // 1. Root Directory
        if parent == INODE_ROOT {
            tracing::debug!("[lookup] Root lookup for name: '{}'", name_str);
            match name_str {
                "." | ".." => {
                    let mut attr = mk_attr(INODE_ROOT, fuser::FileType::Directory, 0o755);
                    let identity = { let state_guard = self.state.read().unwrap(); *state_guard.identity };
                    attr.uid = identity.uid; attr.gid = identity.gid;
                    reply.entry(&ttl, &attr, 0);
                },
                ".magic" => {
                    let mut attr = mk_attr(INODE_MAGIC, fuser::FileType::Directory, 0o755);
                    let identity = { let state_guard = self.state.read().unwrap(); *state_guard.identity };
                    attr.uid = identity.uid; attr.gid = identity.gid;
                    reply.entry(&ttl, &attr, 0);
                },
                "search" => {
                    let mut attr = mk_attr(INODE_SEARCH, fuser::FileType::Directory, 0o555);
                    let identity = { let state_guard = self.state.read().unwrap(); *state_guard.identity };
                    attr.uid = identity.uid; attr.gid = identity.gid;
                    reply.entry(&ttl, &attr, 0);
                },
                "mirror" => {
                    let mut attr = mk_attr(INODE_MIRROR, fuser::FileType::Directory, 0o755);
                    let identity = { let state_guard = self.state.read().unwrap(); *state_guard.identity };
                    attr.uid = identity.uid; attr.gid = identity.gid;
                    reply.entry(&ttl, &attr, 0);
                },
                "inbox" => {
                    tracing::debug!("[lookup] Returning INODE_INBOX (7) for 'inbox'");
                    let mut attr = mk_attr(INODE_INBOX, fuser::FileType::Directory, 0o755);
                    let identity = { let state_guard = self.state.read().unwrap(); *state_guard.identity };
                    attr.uid = identity.uid; attr.gid = identity.gid;
                    reply.entry(&ttl, &attr, 0);
                },
                "tags" => {
                    let mut attr = mk_attr(INODE_TAGS, fuser::FileType::Directory, 0o755);
                    let identity = { let state_guard = self.state.read().unwrap(); *state_guard.identity };
                    attr.uid = identity.uid; attr.gid = identity.gid;
                    reply.entry(&ttl, &attr, 0);
                },
                _ => reply.error(libc::ENOENT),
            }
            return;
        }

        // 2. .magic
        if parent == INODE_MAGIC {
             match name_str {
                "." | ".." => {
                    let mut attr = mk_attr(INODE_MAGIC, fuser::FileType::Directory, 0o755);
                    let identity = { let state_guard = self.state.read().unwrap(); *state_guard.identity };
                    attr.uid = identity.uid; attr.gid = identity.gid;
                    reply.entry(&ttl, &attr, 0);
                },
                "refresh" => {
                    let mut attr = mk_attr(INODE_REFRESH, fuser::FileType::RegularFile, 0o666);
                    attr.size = 0;
                    let identity = { let state_guard = self.state.read().unwrap(); *state_guard.identity };
                    attr.uid = identity.uid; attr.gid = identity.gid;
                    reply.entry(&ttl, &attr, 0);
                },
                _ => reply.error(libc::ENOENT),
            }
            return;
        }

        // 3. Search Root
        if parent == INODE_SEARCH {
            match name_str {
                "." | ".." => {
                    let mut attr = mk_attr(INODE_SEARCH, fuser::FileType::Directory, 0o555);
                    let identity = { let state_guard = self.state.read().unwrap(); *state_guard.identity };
                    attr.uid = identity.uid; attr.gid = identity.gid;
                    reply.entry(&ttl, &attr, 0);
                },
                _ => {
                    if Bouncer::is_noise(name_str) {
                        reply.error(libc::ENOENT);
                        return;
                    }
                    let query = name_str.to_string();
                    let state_guard = self.state.read().unwrap();
                    let inode = state_guard.inode_store.get_or_create_inode(&query);
                    drop(state_guard);
                    let mut attr = mk_attr(inode, fuser::FileType::Directory, 0o555);
                    let identity = { let state_guard = self.state.read().unwrap(); *state_guard.identity };
                    attr.uid = identity.uid; attr.gid = identity.gid;
                    reply.entry(&ttl, &attr, 0);
                }
            }
            return;
        }

        // 4. Tags Root
        if parent == INODE_TAGS {
            match name_str {
                "." | ".." => {
                    let mut attr = mk_attr(INODE_TAGS, fuser::FileType::Directory, 0o755);
                    let identity = { let state_guard = self.state.read().unwrap(); *state_guard.identity };
                    attr.uid = identity.uid; attr.gid = identity.gid;
                    reply.entry(&ttl, &attr, 0);
                },
                _ => {
                    // Check DB for tag existence
                    let state_guard = self.state.read().unwrap();
                    let mut conn_lock = state_guard.db_connection.lock().unwrap();

                    if let Some(conn) = conn_lock.as_mut() {
                        let repo = Repository::new(conn);
                        match repo.get_tag_id_by_name(name_str, None) {
                            Ok(Some(tag_id)) => {
                                let persistent_inode = InodeStore::db_id_to_inode(tag_id);
                                let mut attr = mk_attr(persistent_inode, fuser::FileType::Directory, 0o755);

                                // Apply Masquerade
                                let identity = {
                                    let state_guard = self.state.read().unwrap();
                                    *state_guard.identity
                                };
                                attr.uid = identity.uid;
                                attr.gid = identity.gid;

                                reply.entry(&ttl, &attr, 0);
                                return;
                            }
                            Ok(None) => {
                                reply.error(libc::ENOENT);
                                return;
                            }
                            Err(_) => {
                                reply.error(libc::EIO);
                                return;
                            }
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
                "." | ".." => {
                    let mut attr = mk_attr(INODE_INBOX, fuser::FileType::Directory, 0o755);
                    let identity = { let state_guard = self.state.read().unwrap(); *state_guard.identity };
                    attr.uid = identity.uid; attr.gid = identity.gid;
                    reply.entry(&ttl, &attr, 0);
                },
                _ => {
                    let state_guard = self.state.read().unwrap();

                    // PHASE 25: First, check if it's a child tag (still need DB for tags)
                    {
                        let mut conn_lock = state_guard.db_connection.lock().unwrap();
                        if let Some(conn) = conn_lock.as_mut() {
                            let repo = Repository::new(conn);
                            if let Ok(Some(tag_id)) = repo.get_tag_id_by_name(name_str, Some(1)) {
                                // Child tag found
                                let child_inode = InodeStore::db_id_to_inode(tag_id);
                                let mut attr = mk_attr(child_inode, fuser::FileType::Directory, 0o755);

                                // Apply Masquerade
                                let identity = {
                                    let state_guard = self.state.read().unwrap();
                                    *state_guard.identity
                                };
                                attr.uid = identity.uid;
                                attr.gid = identity.gid;

                                reply.entry(&ttl, &attr, 0);
                                return;
                            }
                        }
                    }

                    // PHASE 25: Physical file lookup using filesystem
                    // Get system inbox path (use as_ref() to avoid move)
                    let inbox_path_opt = {
                        let lock = state_guard.system_inbox_path.lock().unwrap();
                        lock.clone()
                    };

                    // Use a reference to avoid moving inbox_path_opt
                    if let Some(ref inbox_path) = inbox_path_opt {
                        let physical_path = std::path::Path::new(inbox_path).join(name_str);

                        // Check if file exists physically
                        if let Ok(metadata) = std::fs::metadata(&physical_path) {
                            // PHASE 25 FIX: Use Stable Hashed Inode
                            let inode = state_guard.inode_store.hash_to_inode(name_str);

                            // Register mapping for getattr
                            let abs_path = physical_path.to_string_lossy().to_string();
                            state_guard.inode_store.put_mirror_path(inode, abs_path.clone());

                            let kind = if metadata.is_dir() { fuser::FileType::Directory } else { fuser::FileType::RegularFile };
                            let perm = if metadata.is_dir() { 0o755 } else { 0o644 };
                            let mut attr = mk_attr(inode, kind, perm);
                            attr.size = metadata.len();
                            attr.blocks = (metadata.len() + 511)/512;
                            attr.mtime = metadata.modified().unwrap_or(std::time::SystemTime::now());

                            // Apply Masquerade
                            let identity = {
                                let state_guard = self.state.read().unwrap();
                                *state_guard.identity
                            };
                            attr.uid = identity.uid;
                            attr.gid = identity.gid;

                            reply.entry(&ttl, &attr, 0);
                            return;
                        }
                    }

                    // PHASE 25: Fallback to virtual name parsing for "(N)" duplicates
                    if let Some((base_name, index)) = Self::parse_virtual_name(name_str) {
                        // Check for physical files with collision pattern
                        if let Some(ref inbox_path) = inbox_path_opt {
                            let mut candidates = Vec::new();

                            // Scan for files starting with base_name
                            if let Ok(dir_entries) = std::fs::read_dir(inbox_path) {
                                for entry in dir_entries.flatten() {
                                    if let Ok(file_name) = entry.file_name().into_string() {
                                        if file_name == base_name || file_name.starts_with(&format!("{} (", base_name)) {
                                            let path = entry.path();
                                            candidates.push((file_name, path));
                                        }
                                    }
                                }
                            }

                            // Sort to ensure deterministic ordering
                            candidates.sort_by(|a, b| a.0.cmp(&b.0));

                            if let Some((_, path)) = candidates.get(index) {
                                if let Ok(metadata) = std::fs::metadata(path) {
                                    // PHASE 25 FIX: Use the requested name_str for inode calculation
                                    let inode = state_guard.inode_store.hash_to_inode(name_str);

                                    // Register mapping to the actual physical file
                                    let abs_path = path.to_string_lossy().to_string();
                                    state_guard.inode_store.put_mirror_path(inode, abs_path.clone());

                                    let kind = if metadata.is_dir() { fuser::FileType::Directory } else { fuser::FileType::RegularFile };
                                    let perm = if metadata.is_dir() { 0o755 } else { 0o644 };
                                    let mut attr = mk_attr(inode, kind, perm);
                                    attr.size = metadata.len();
                                    attr.blocks = (metadata.len() + 511)/512;
                                    attr.mtime = metadata.modified().unwrap_or(std::time::SystemTime::now());

                                    // Apply Masquerade
                                    let identity = {
                                        let state_guard = self.state.read().unwrap();
                                        *state_guard.identity
                                    };
                                    attr.uid = identity.uid;
                                    attr.gid = identity.gid;

                                    reply.entry(&ttl, &attr, 0);
                                    return;
                                }
                            }
                        }
                    }

                    reply.error(libc::ENOENT);
                }
             }
             return;
        }

        // 6. Mirror Root
        if parent == INODE_MIRROR {
             if name_str == "." || name_str == ".." {
                let mut attr = mk_attr(INODE_MIRROR, fuser::FileType::Directory, 0o755);
                let identity = { let state_guard = self.state.read().unwrap(); *state_guard.identity };
                attr.uid = identity.uid; attr.gid = identity.gid;
                reply.entry(&ttl, &attr, 0); return;
            }
            let state_guard = self.state.read().unwrap();
            let wp_guard = state_guard.watch_paths.lock().unwrap();
            
            for path_str in wp_guard.iter() {
                let path = Path::new(path_str);
                if let Some(filename) = path.file_name() {
                    if filename.to_str() == Some(name_str) {
                        let inode = state_guard.inode_store.hash_to_inode(path_str);
                        state_guard.inode_store.put_mirror_path(inode, path_str.clone());
                        let mut attr = mk_attr(inode, fuser::FileType::Directory, 0o755);
                        let identity = { let state_guard = self.state.read().unwrap(); *state_guard.identity };
                        attr.uid = identity.uid; attr.gid = identity.gid;
                        reply.entry(&ttl, &attr, 0);
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
                    // RESOLUTION STRATEGY:
                    // 1. Check if it's a child tag first
                    // 2. Try exact file match
                    // 3. If fail, try parsing "name (N).ext" and fetching Nth duplicate

                    let mut target_inode = None;
                    let mut target_meta = None;

                    // Attempt 0: Check if it's a child tag
                    let tag_check_sql = "SELECT tag_id FROM tags WHERE name = ?1 AND parent_tag_id = ?2";
                    if let Ok(mut stmt) = conn.prepare(tag_check_sql) {
                        if let Ok(tag_id_result) = stmt.query_row(params![name_str, tag_id], |row| row.get::<_, u64>(0)) {
                            // This is a child tag!
                            let child_inode = InodeStore::db_id_to_inode(tag_id_result);
                            let ttl = Duration::from_secs(1);
                            let attr = mk_attr(child_inode, fuser::FileType::Directory, 0o755);
                            reply.entry(&ttl, &attr, 0);
                            return;
                        }
                    }

                    // Attempt 1: Exact Match
                    tracing::info!("[HollowDrive] LOOKUP tag_id={}, name_str={}", tag_id, name_str);
                    let sql_exact = "
                        SELECT f.inode, f.size, f.mtime, f.is_dir
                        FROM file_tags ft
                        JOIN file_registry f ON ft.file_id = f.file_id
                        WHERE ft.tag_id = ?1 AND ft.display_name = ?2
                    ";
                    
                    if let Ok(mut stmt) = conn.prepare(sql_exact) {
                        let rows_res = stmt.query_map(params![tag_id, name_str], |row| {
                            Ok((
                                row.get::<_, u64>(0)?, 
                                row.get::<_, u64>(1)?, 
                                row.get::<_, u64>(2)?, 
                                row.get::<_, i32>(3)?
                            ))
                        });

                        if let Ok(mut rows) = rows_res {
                            if let Some(Ok(row)) = rows.next() {
                                target_inode = Some(row.0);
                                target_meta = Some((row.1, row.2, row.3));
                            }
                        }
                    }

                    // Attempt 2: Virtual Alias Resolution (if exact match failed)
                    if target_inode.is_none() {
                        if let Some((base_name, index)) = Self::parse_virtual_name(name_str) {
                             // Fetch ALL duplicates for base_name, sorted deterministically (by file_id)
                             let sql_alias = "
                                SELECT f.inode, f.size, f.mtime, f.is_dir 
                                FROM file_tags ft 
                                JOIN file_registry f ON ft.file_id = f.file_id 
                                WHERE ft.tag_id = ?1 AND ft.display_name = ?2
                                ORDER BY f.file_id ASC
                             ";
                             
                             if let Ok(mut stmt) = conn.prepare(sql_alias) {
                                 let rows_res = stmt.query_map(params![tag_id, base_name], |row| {
                                     Ok((
                                         row.get::<_, u64>(0)?, 
                                         row.get::<_, u64>(1)?, 
                                         row.get::<_, u64>(2)?, 
                                         row.get::<_, i32>(3)?
                                     ))
                                 });

                                 if let Ok(rows) = rows_res {
                                     // Skip 'index' items
                                     for (i, row) in rows.enumerate() {
                                         if i == index {
                                             if let Ok(r) = row {
                                                 target_inode = Some(r.0);
                                                 target_meta = Some((r.1, r.2, r.3));
                                             }
                                             break;
                                         }
                                     }
                                 }
                             }
                        }
                    }

                    if let Some(inode) = target_inode {
                        tracing::info!("[HollowDrive] LOOKUP SUCCESS: Found inode={} for tag_id={}, name={}", inode, tag_id, name_str);
                        if let Some((size, mtime, is_dir_int)) = target_meta {
                            let kind = if is_dir_int != 0 { fuser::FileType::Directory } else { fuser::FileType::RegularFile };
                            let perm = if is_dir_int != 0 { 0o755 } else { 0o644 };
                            let mut attr = mk_attr(inode, kind, perm);
                            attr.size = size;
                            attr.blocks = (size + 511)/512;
                            attr.mtime = std::time::UNIX_EPOCH + std::time::Duration::from_secs(mtime);

                            // Apply Masquerade for persistent tags
                            let identity = {
                                let state_guard = self.state.read().unwrap();
                                *state_guard.identity
                            };
                            attr.uid = identity.uid;
                            attr.gid = identity.gid;

                            reply.entry(&ttl, &attr, 0);
                            return;
                        } else {
                            tracing::error!("Target inode found but metadata missing for parent={}, name={}", parent, name_str);
                        }
                    } else {
                        tracing::info!("[HollowDrive] LOOKUP FAILED: No result for tag_id={}, name={}", tag_id, name_str);
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
                // PHASE 25 FIX: Use a stable inode for search results
                let file_inode = inode_store.hash_to_inode(&format!("{}-{}", parent, name_str));
                let mut attr = mk_attr(file_inode, fuser::FileType::RegularFile, 0o644);

                // Apply Masquerade
                let identity = {
                    let state_guard = self.state.read().unwrap();
                    *state_guard.identity
                };
                attr.uid = identity.uid;
                attr.gid = identity.gid;

                reply.entry(&ttl, &attr, 0);
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

                    // Apply Masquerade for mirror directories
                    let identity = {
                        let state_guard = self.state.read().unwrap();
                        *state_guard.identity
                    };
                    attr.uid = identity.uid;
                    attr.gid = identity.gid;

                    reply.entry(&ttl, &attr, 0);
                } else {
                    reply.error(libc::ENOENT);
                }
                return;
            }
        }

        reply.error(libc::ENOENT);
    }

    fn getattr(&mut self, _req: &Request, inode: u64, reply: ReplyAttr) {
        // 1. Retrieve Identity for Masquerade
        let identity = {
            let state_guard = self.state.read().unwrap();
            *state_guard.identity
        };

        let ts = self.state.read().unwrap().start_time;

        // 2. Resolve attributes based on Inode Type
        let mut attr = match inode {
            INODE_ROOT => fuser::FileAttr {
                ino: INODE_ROOT, size: 0, blocks: 0, atime: ts, mtime: ts, ctime: ts, crtime: ts,
                kind: FileType::Directory, perm: 0o755, nlink: 2, uid: 0, gid: 0, rdev: 0, flags: 0, blksize: 512,
            },
            INODE_TAGS => fuser::FileAttr {
                ino: INODE_TAGS, size: 0, blocks: 0, atime: ts, mtime: ts, ctime: ts, crtime: ts,
                kind: FileType::Directory, perm: 0o755, nlink: 2, uid: 0, gid: 0, rdev: 0, flags: 0, blksize: 512,
            },
            INODE_INBOX => fuser::FileAttr {
                ino: INODE_INBOX, size: 0, blocks: 0, atime: ts, mtime: ts, ctime: ts, crtime: ts,
                kind: FileType::Directory, perm: 0o755, nlink: 2, uid: 0, gid: 0, rdev: 0, flags: 0, blksize: 512,
            },
            INODE_MIRROR => fuser::FileAttr {
                ino: INODE_MIRROR, size: 0, blocks: 0, atime: ts, mtime: ts, ctime: ts, crtime: ts,
                kind: FileType::Directory, perm: 0o755, nlink: 2, uid: 0, gid: 0, rdev: 0, flags: 0, blksize: 512,
            },
            INODE_SEARCH => fuser::FileAttr {
                ino: INODE_SEARCH, size: 0, blocks: 0, atime: ts, mtime: ts, ctime: ts, crtime: ts,
                kind: FileType::Directory, perm: 0o555, nlink: 2, uid: 0, gid: 0, rdev: 0, flags: 0, blksize: 512,
            },
            INODE_MAGIC => fuser::FileAttr {
                ino: INODE_MAGIC, size: 0, blocks: 0, atime: ts, mtime: ts, ctime: ts, crtime: ts,
                kind: FileType::Directory, perm: 0o755, nlink: 2, uid: 0, gid: 0, rdev: 0, flags: 0, blksize: 512,
            },
            INODE_REFRESH => fuser::FileAttr {
                ino: INODE_REFRESH, size: 0, blocks: 0, atime: ts, mtime: ts, ctime: ts, crtime: ts,
                kind: FileType::RegularFile, perm: 0o666, nlink: 1, uid: 0, gid: 0, rdev: 0, flags: 0, blksize: 512,
            },
            _ => {
                if InodeStore::is_persistent(inode) {
                    // Persistent Tag (e.g. /tags/finance)
                    fuser::FileAttr {
                        ino: inode, size: 0, blocks: 0, atime: ts, mtime: ts, ctime: ts, crtime: ts,
                        kind: FileType::Directory, perm: 0o755, nlink: 2, uid: 0, gid: 0, rdev: 0, flags: 0, blksize: 512,
                    }
                } else {
                    // Dynamic Content (Virtual Search Results or Physical Files)
                    let state_guard = self.state.read().unwrap();

                    if let Some(node) = state_guard.inode_store.get_inode(inode) {
                        // Virtual Directory
                        let perm = if node.is_dir { 0o755 } else { 0o644 };
                        fuser::FileAttr {
                            ino: inode, size: 0, blocks: 0, atime: ts, mtime: ts, ctime: ts, crtime: ts,
                            kind: if node.is_dir { FileType::Directory } else { FileType::RegularFile },
                            perm, nlink: 1, uid: 0, gid: 0, rdev: 0, flags: 0, blksize: 512,
                        }
                    } else if let Some(real_path) = state_guard.inode_store.get_mirror_path(inode) {
                        // Physical File Passthrough
                        match std::fs::metadata(&real_path) {
                            Ok(meta) => self.metadata_to_attr(&meta, inode),
                            Err(_) => {
                                reply.error(libc::ENOENT);
                                return;
                            }
                        }
                    } else {
                        // Unknown Inode
                        reply.error(libc::ENOENT);
                        return;
                    }
                }
            }
        };

        // 3. THE MASQUERADE (Global Ownership Override)
        // Force the FUSE layer to report the User's UID/GID, not Root's.
        attr.uid = identity.uid;
        attr.gid = identity.gid;

        let ttl = Duration::from_secs(1);
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
        // Get identity for masquerade
        let identity = {
            let state_guard = self.state.read().unwrap();
            *state_guard.identity
        };

        if ino == INODE_REFRESH {
             let state_guard = self.state.read().unwrap();
             state_guard.refresh_signal.store(true, std::sync::atomic::Ordering::Relaxed);
             let mut attr = fuser::FileAttr {
                 ino: INODE_REFRESH, size: 0, blocks: 0, atime: SystemTime::now(),
                 mtime: SystemTime::now(), ctime: SystemTime::now(),
                 crtime: SystemTime::now(), kind: fuser::FileType::RegularFile,
                 perm: 0o666, nlink: 1, uid: identity.uid, gid: identity.gid, rdev: 0, blksize: 4096, flags: 0,
             };
             // Apply masquerade
             attr.uid = identity.uid;
             attr.gid = identity.gid;
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
                    perm: 0o644, nlink: 1, uid: identity.uid, gid: identity.gid, rdev: 0, blksize: 4096, flags: 0,
                };
                if let Ok(m) = meta.modified() { attr.mtime = m; }
                // Apply masquerade
                attr.uid = identity.uid;
                attr.gid = identity.gid;
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
            tracing::debug!("[readdir] Listing root directory");
            let mut root_entries = entries.clone();
            root_entries.extend_from_slice(&[
                (INODE_INBOX, FileType::Directory, "inbox".to_string()),
                (INODE_TAGS, FileType::Directory, "tags".to_string()),
                (INODE_SEARCH, FileType::Directory, "search".to_string()),
                (INODE_MIRROR, FileType::Directory, "mirror".to_string()),
                (INODE_MAGIC, FileType::Directory, ".magic".to_string()),
            ]);
            for (i, (ino, kind, name)) in root_entries.iter().enumerate().skip(offset as usize) {
                tracing::debug!("[readdir] Adding entry: inode={}, name={}", ino, name);
                if reply.add(*ino, (i+1) as i64, *kind, name) { break; }
            }
            reply.ok();
            return;
        }

        if ino == INODE_MAGIC {
            let mut items = entries.clone();
            items.extend_from_slice(&[
                (INODE_REFRESH, FileType::RegularFile, "refresh".to_string()),
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
                    if let Ok(rows) = stmt.query_map([], |row| {
                        Ok((row.get::<_, u64>(0)?, row.get::<_, String>(1)?))
                    }) {
                        for row in rows {
                            if let Ok((tag_id, name)) = row {
                                let persistent_inode = InodeStore::db_id_to_inode(tag_id);
                                items.push((persistent_inode, FileType::Directory, name));
                            }
                        }
                    } else {
                        tracing::error!("DB Error querying top-level tags");
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
             tracing::info!("[readdir] INODE_INBOX: Listing physical inbox directory (PHASE 25)");
             let mut items = entries.clone();

             // PHASE 25: Get system inbox path and use std::fs::read_dir
             let state_guard = self.state.read().unwrap();
             let inbox_path_opt = {
                 let lock = state_guard.system_inbox_path.lock().unwrap();
                 lock.clone()
             };

             if let Some(inbox_path) = inbox_path_opt {
                 tracing::info!("[readdir] INODE_INBOX: System inbox path = {}", inbox_path);
                 // Add child tags of Inbox (tag_id=1) - still need DB for tags
                 let mut conn_lock = state_guard.db_connection.lock().unwrap();
                 if let Some(conn) = conn_lock.as_mut() {
                     let child_tag_sql = "SELECT tag_id, name FROM tags WHERE parent_tag_id = 1";
                     if let Ok(mut stmt) = conn.prepare(child_tag_sql) {
                         if let Ok(rows) = stmt.query_map([], |row| {
                             Ok((row.get::<_, u64>(0)?, row.get::<_, String>(1)?))
                         }) {
                             for row in rows {
                                 if let Ok((child_tag_id, name)) = row {
                                     let child_inode = InodeStore::db_id_to_inode(child_tag_id);
                                     items.push((child_inode, FileType::Directory, name));
                                 }
                             }
                         }
                     }
                 }

                 // PHASE 25: Physical files - use std::fs::read_dir, NOT SQL
                 if let Ok(dir_entries) = std::fs::read_dir(&inbox_path) {
                     let mut name_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

                     for entry in dir_entries.flatten() {
                         if let Ok(file_name) = entry.file_name().into_string() {
                             // Skip dotfiles and hidden files
                             if file_name.starts_with('.') {
                                 continue;
                             }

                             // Count collisions for virtual naming (report (1).pdf)
                             let count = name_counts.entry(file_name.clone()).or_insert(0);
                             *count += 1;

                             let final_name = if *count > 1 {
                                 format!("{} ({})", file_name, count)
                             } else {
                                 file_name
                             };

                             // Determine file type and generate inode
                             let is_dir = entry.file_type().map(|f| f.is_dir()).unwrap_or(false);
                             let kind = if is_dir { FileType::Directory } else { FileType::RegularFile };

                             // PHASE 25 FIX: Use Stable Hashed Inode
                             let inode = state_guard.inode_store.hash_to_inode(&final_name);

                             // Register mapping
                             let abs_path = entry.path().to_string_lossy().to_string();
                             state_guard.inode_store.put_mirror_path(inode, abs_path);

                             items.push((inode, kind, final_name));
                         }
                     }
                 } else {
                     let err = std::fs::read_dir(&inbox_path).unwrap_err();
                     tracing::error!("[readdir] INODE_INBOX: Failed to read directory '{}': {}", inbox_path, err);
                 }
             } else {
                 tracing::error!("[readdir] INODE_INBOX: No system inbox path configured");
             }

             for (i, (ino, kind, name)) in items.iter().enumerate().skip(offset as usize) {
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
                 // First: Add child tags
                 let child_tag_sql = "SELECT tag_id, name FROM tags WHERE parent_tag_id = ?1";
                 if let Ok(mut stmt) = conn.prepare(child_tag_sql) {
                     if let Ok(rows) = stmt.query_map(params![tag_id], |row| {
                         Ok((row.get::<_, u64>(0)?, row.get::<_, String>(1)?))
                     }) {
                         for row in rows {
                             if let Ok((child_tag_id, name)) = row {
                                 let child_inode = InodeStore::db_id_to_inode(child_tag_id);
                                 items.push((child_inode, FileType::Directory, name));
                             }
                         }
                     } else {
                         tracing::error!("DB Error querying child tags");
                     }
                 }

                 // Second: Add files from this tag
                 let sql = "
                    SELECT f.inode, f.is_dir, ft.display_name
                    FROM file_tags ft
                    JOIN file_registry f ON ft.file_id = f.file_id
                    WHERE ft.tag_id = ?1
                    ORDER BY f.file_id ASC
                 ";
                 if let Ok(mut stmt) = conn.prepare(sql) {
                     if let Ok(rows) = stmt.query_map(params![tag_id], |row| {
                         Ok((
                             row.get::<_, u64>(0)?,
                             row.get::<_, i32>(1)?,
                             row.get::<_, String>(2)?
                         ))
                     }) {
                         // COLLISION RESOLUTION: Count Names
                         let mut name_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

                         for row in rows {
                             if let Ok((inode, is_dir, name)) = row {
                                 let count = name_counts.entry(name.clone()).or_insert(0);
                                 *count += 1;

                                 let final_name = if *count > 1 {
                                     format!("{} ({})", name, count)
                                 } else {
                                     name
                                 };

                                 let kind = if is_dir != 0 { FileType::Directory } else { FileType::RegularFile };
                                 items.push((inode, kind, final_name));
                             }
                         }
                     } else {
                         tracing::error!("DB Error querying file tags");
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

    /// Called when the last file handle for a file is closed
    /// This is essential for proper file handle cleanup
    fn release(
        &mut self,
        _req: &Request,
        ino: u64,
        fh: u64,
        _flags: i32,
        _lock_owner: Option<u64>,
        _flush: bool,
        reply: fuser::ReplyEmpty,
    ) {
        println!("[MOONBUG] release() called: ino={}, fh={}", ino, fh);
        tracing::info!("[RELEASE] ino={}, fh={}", ino, fh);
        // No cleanup needed for our implementation, but FUSE expects this to exist
        // Without this, operations like open().close() may fail
        reply.ok();
    }

    /// Called when file data should be flushed to disk
    /// This is called before release when the file was opened with O_SYNC or similar
    fn flush(
        &mut self,
        _req: &Request,
        ino: u64,
        fh: u64,
        _lock_owner: u64,
        reply: fuser::ReplyEmpty,
    ) {
        println!("[MOONBUG] flush() called: ino={}, fh={}", ino, fh);
        tracing::info!("[FLUSH] ino={}, fh={}", ino, fh);
        // Our write operations are already immediate (using write_at)
        // This is a no-op but prevents errors
        reply.ok();
    }

    fn rename(
        &mut self,
        _req: &Request,
        parent: u64,
        name: &std::ffi::OsStr,
        newparent: u64,
        newname: &std::ffi::OsStr,
        _flags: u32,
        reply: fuser::ReplyEmpty,
    ) {
        let name_str = match name.to_str() {
            Some(s) => s,
            None => { reply.error(libc::EINVAL); return; }
        };

        let newname_str = match newname.to_str() {
            Some(s) => s,
            None => { reply.error(libc::EINVAL); return; }
        };

        // DEBUG: Log rename operation
        tracing::info!("[HollowDrive] RENAME called: parent={:?}, name={}, newparent={:?}, newname={}",
                       parent, name_str, newparent, newname_str);
        tracing::info!("[HollowDrive] RENAME parent is_persistent={}, newparent is_persistent={}",
                       InodeStore::is_persistent(parent), InodeStore::is_persistent(newparent));

        // --- PHASE 39: INBOX TO TAG SUPPORT ---
        // Case A: Rename within inbox (atomic save: .part -> .txt)
        if parent == INODE_INBOX && newparent == INODE_INBOX {
             // 1. Get System Inbox Path from State
             let inbox_path = {
                 let state_guard = self.state.read().unwrap();
                 let lock = state_guard.system_inbox_path.lock().unwrap();
                 match lock.clone() {
                     Some(p) => p,
                     None => {
                         reply.error(libc::ENOENT);
                         return;
                     }
                 }
             };

             let old_path = std::path::Path::new(&inbox_path).join(name_str);
             let new_path = std::path::Path::new(&inbox_path).join(newname_str);

             // 2. Perform Physical Rename
             if let Err(e) = std::fs::rename(&old_path, &new_path) {
                 let err = match e.kind() {
                     std::io::ErrorKind::NotFound => libc::ENOENT,
                     std::io::ErrorKind::PermissionDenied => libc::EACCES,
                     std::io::ErrorKind::AlreadyExists => libc::EEXIST,
                     _ => libc::EIO,
                 };
                 tracing::error!("[HollowDrive] Inbox rename failed: {}", e);
                 reply.error(err);
                 return;
             }

             // Phase 40: Ensure ownership is maintained after rename
             {
                 let identity = {
                     let state_guard = self.state.read().unwrap();
                     *state_guard.identity
                 };

                 if let Err(e) = identity.enforce_ownership(&new_path) {
                     tracing::error!("[HollowDrive] Failed to enforce ownership after inbox rename: {}", e);
                     // Try to revert
                     let _ = std::fs::rename(&new_path, &old_path);
                     reply.error(libc::EIO);
                     return;
                 }
             }

             // 3. Success (Librarian will handle DB updates via inotify)
             tracing::info!("[HollowDrive] Atomic rename in Inbox: {} -> {}", name_str, newname_str);
             reply.ok();
             return;
        }

        // Case B: Move from Inbox to Tag (os.rename("/inbox/file.txt", "/tags/finance/file.txt"))
        if parent == INODE_INBOX && InodeStore::is_persistent(newparent) {
             tracing::info!("[HollowDrive] CASE B: Inbox-to-Tag move triggered");
             // 1. Get source file from system inbox
             let inbox_path = {
                 let state_guard = self.state.read().unwrap();
                 let lock = state_guard.system_inbox_path.lock().unwrap();
                 match lock.clone() {
                     Some(p) => p,
                     None => {
                         tracing::error!("[HollowDrive] CASE B: No system inbox path configured");
                         reply.error(libc::ENOENT);
                         return;
                     }
                 }
             };

             let source_path = std::path::Path::new(&inbox_path).join(name_str);
             tracing::info!("[HollowDrive] CASE B: Looking for source file: {}", source_path.display());

             // 2. Verify source exists
             if !source_path.exists() {
                 tracing::error!("[HollowDrive] CASE B: Source file does not exist: {}", source_path.display());
                 reply.error(libc::ENOENT);
                 return;
             }

             // 3. Get file metadata
             let metadata = match source_path.metadata() {
                 Ok(m) => m,
                 Err(e) => {
                     tracing::error!("[HollowDrive] Failed to get metadata: {}", e);
                     reply.error(libc::EIO);
                     return;
                 }
             };

             let physical_inode = metadata.ino();
             let mtime = metadata.modified().unwrap_or(SystemTime::now())
                 .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
             let size = metadata.len();

             // 4. Get physical path for the file registry
             // Use watch_dir[0]/_moved_from_inbox/<filename> as the physical location
             let physical_path = {
                 let state_guard = self.state.read().unwrap();
                 let wp = state_guard.watch_paths.lock().unwrap();
                 if wp.is_empty() {
                     reply.error(libc::ENOSPC);
                     return;
                 }
                 let base_dir = Path::new(&wp[0]);
                 let moved_dir = base_dir.join("_moved_from_inbox");
                 if !moved_dir.exists() {
                     if let Err(e) = std::fs::create_dir_all(&moved_dir) {
                         tracing::error!("[HollowDrive] Failed to create _moved_from_inbox: {}", e);
                         reply.error(libc::EIO);
                         return;
                     }
                 }
                 moved_dir.join(name_str)
             };

             // 5. Move physical file to the persistent location
             if let Err(e) = std::fs::rename(&source_path, &physical_path) {
                 let err = match e.kind() {
                     std::io::ErrorKind::NotFound => libc::ENOENT,
                     std::io::ErrorKind::PermissionDenied => libc::EACCES,
                     std::io::ErrorKind::AlreadyExists => libc::EEXIST,
                     _ => libc::EIO,
                 };
                 tracing::error!("[HollowDrive] Failed to move file from inbox: {}", e);
                 reply.error(err);
                 return;
             }

             // --- FIX: Update Inode Mapping immediately ---
             // The file has moved, so the Inode must point to the NEW physical location.
             // If we don't do this, getattr() will check the old path and return ENOENT.
             {
                 let state_guard = self.state.read().unwrap();
                 // 1. Calculate the stable inode (matches create/readdir logic)
                 let inode = state_guard.inode_store.hash_to_inode(name_str);
                 // 2. Point it to the new physical path
                 let new_path_str = physical_path.to_string_lossy().to_string();
                 state_guard.inode_store.put_mirror_path(inode, new_path_str.clone());
                 tracing::debug!("[rename] Updated mapping: inode={} -> path={}", inode, new_path_str);
             }
             // ---------------------------------------------

             // Phase 40: Enforce ownership after the move
             {
                 let identity = {
                     let state_guard = self.state.read().unwrap();
                     *state_guard.identity
                 };

                 if let Err(e) = identity.enforce_ownership(&physical_path) {
                     tracing::error!("[HollowDrive] Failed to enforce ownership after move: {}", e);
                     // Try to undo the move
                     let _ = std::fs::rename(&physical_path, &source_path);
                     reply.error(libc::EIO);
                     return;
                 }
             }

             // 7. Update DB (FIXED: UPDATE existing entry to preserve inode lookup integrity)
             let old_path_str = source_path.to_string_lossy().to_string();
             let new_path_str = physical_path.to_string_lossy().to_string();
             let dest_tag_id = InodeStore::inode_to_db_id(newparent);

             let state_guard = self.state.read().unwrap();
             let mut conn_lock = state_guard.db_connection.lock().unwrap();

             if let Some(conn) = conn_lock.as_mut() {
                 let tx = match conn.transaction() {
                     Ok(t) => t,
                     Err(_) => {
                         // Try to undo the move
                         let _ = std::fs::rename(&physical_path, &source_path);
                         reply.error(libc::EIO);
                         return;
                     }
                 };

                 // 1. Try to UPDATE existing record first (preserves file_id for inode lookup)
                 let updated = tx.execute(
                     "UPDATE file_registry SET abs_path = ?1, inode = ?2, mtime = ?3, size = ?4 WHERE abs_path = ?5",
                     params![new_path_str, physical_inode, mtime, size, old_path_str]
                 ).unwrap_or(0);

                 let file_id = if updated > 0 {
                     // Fetch the ID of the updated record
                     tx.query_row(
                         "SELECT file_id FROM file_registry WHERE abs_path = ?1",
                         params![new_path_str],
                         |r| r.get::<_, u64>(0)
                     ).unwrap_or(0)
                 } else {
                     // 2. If no record existed (edge case), INSERT new one
                     tx.query_row(
                         "INSERT INTO file_registry (abs_path, inode, mtime, size, is_dir)
                          VALUES (?1, ?2, ?3, ?4, 0)
                          ON CONFLICT(abs_path) DO UPDATE SET inode=excluded.inode, mtime=excluded.mtime
                          RETURNING file_id",
                         params![new_path_str, physical_inode, mtime, size],
                         |r| r.get::<_, u64>(0)
                     ).unwrap_or(0)
                 };

                 if file_id > 0 {
                     // 3. Link to destination tag (use INSERT OR REPLACE)
                     let _ = tx.execute(
                         "INSERT OR REPLACE INTO file_tags (file_id, tag_id, display_name) VALUES (?1, ?2, ?3)",
                         params![file_id, dest_tag_id, newname_str]
                     );

                     // 4. Cleanup old Inbox tag (Tag ID 1) if it exists
                     let _ = tx.execute(
                         "DELETE FROM file_tags WHERE file_id = ?1 AND tag_id = 1",
                         params![file_id]
                     );
                 }

                 if let Err(_) = tx.commit() {
                     let _ = std::fs::rename(&physical_path, &source_path);
                     reply.error(libc::EIO);
                     return;
                 }
             } else {
                 let _ = std::fs::rename(&physical_path, &source_path);
                 reply.error(libc::EIO);
                 return;
             }

             tracing::info!("[HollowDrive] Moved file from inbox to tag (Tag ID {}): {} -> {}",
                           dest_tag_id, name_str, newname_str);
             reply.ok();
             return;
        }

        // Case C: Move TO Inbox from Tag (os.rename("/tags/finance/file.txt", "/inbox/file.txt"))
        if InodeStore::is_persistent(parent) && newparent == INODE_INBOX {
             // This is the reverse operation - moving FROM tag back TO inbox
             // For now, we'll block this as it's not part of Phase 39 requirements
             // The tag view is persistent, inbox is "staging"
             reply.error(libc::EXDEV);
             return;
        }
        // --- END PHASE 39 LOGIC ---

        // Only support renaming within the Tag System (Existing Check)
        if !InodeStore::is_persistent(parent) || !InodeStore::is_persistent(newparent) {
            tracing::info!("[HollowDrive] REJECTED: Not persistent - parent={}, newparent={}", parent, newparent);
            reply.error(libc::EXDEV); // "Cross-device link"
            return;
        }

        let source_tag_id = InodeStore::inode_to_db_id(parent);
        let dest_tag_id = InodeStore::inode_to_db_id(newparent);

        tracing::info!("[HollowDrive] TAG-TO-TAG RENAME: {} (tag_id={}) -> {} (tag_id={})",
                       name_str, source_tag_id, newname_str, dest_tag_id);

        let state_guard = self.state.read().unwrap();
        let mut conn_lock = state_guard.db_connection.lock().unwrap();

        if let Some(conn) = conn_lock.as_mut() {
            let mut repo = Repository::new(conn);

            // Determine if we're renaming a file or a tag
            // First, check if it's a tag
            let tag_id_opt = match repo.get_tag_id_by_name(name_str, Some(source_tag_id)) {
                Ok(id) => id,
                Err(_) => {
                    reply.error(libc::EIO);
                    return;
                }
            };

            if let Some(tag_id) = tag_id_opt {
                // CASE: Renaming/moving a TAG

                if source_tag_id != dest_tag_id {
                    // Move tag to different parent
                    match repo.move_tag(tag_id, dest_tag_id, newname_str) {
                        Ok(_) => reply.ok(),
                        Err(MagicError::State(msg)) if msg == "Circular dependency detected" => {
                            reply.error(libc::ELOOP);
                        }
                        Err(MagicError::State(_)) => {
                            reply.error(libc::EEXIST);
                        }
                        Err(_) => reply.error(libc::EIO),
                    }
                } else {
                    // Just rename within same parent
                    match repo.rename_tag(tag_id, newname_str) {
                        Ok(_) => reply.ok(),
                        Err(MagicError::State(_)) => {
                            reply.error(libc::EEXIST);
                        }
                        Err(_) => reply.error(libc::EIO),
                    }
                }
                return;
            }

            // Not a tag, try to handle as file
            tracing::info!("[HollowDrive] Looking for file_id in tag_id={}", source_tag_id);
            let file_id_opt = match repo.get_file_id_in_tag(source_tag_id, name_str) {
                Ok(id) => {
                    tracing::info!("[HollowDrive] get_file_id_in_tag returned: {:?}", id);
                    id
                }
                Err(e) => {
                    tracing::error!("[HollowDrive] get_file_id_in_tag failed: {}", e);
                    reply.error(libc::EIO);
                    return;
                }
            };

            let file_id = match file_id_opt {
                Some(id) => {
                    tracing::info!("[HollowDrive] Found file_id: {}", id);
                    id
                }
                None => {
                    tracing::error!("[HollowDrive] File not found in source tag! tag_id={}, name={}", source_tag_id, name_str);
                    reply.error(libc::ENOENT);
                    return;
                }
            };

            // 3. Perform File Operation
            if source_tag_id == dest_tag_id {
                // CASE A: RENAME FILE (Same Folder)
                tracing::info!("[HollowDrive] Renaming file_id={} in tag_id={} to '{}'", file_id, source_tag_id, newname_str);
                match repo.rename_file_in_tag(file_id, source_tag_id, newname_str) {
                    Ok(_) => {
                        tracing::info!("[HollowDrive] File rename successful");
                        reply.ok()
                    },
                    Err(MagicError::State(_)) => {
                        tracing::error!("[HollowDrive] File rename conflict");
                        reply.error(libc::EEXIST);
                    }
                    Err(e) => {
                        tracing::error!("[HollowDrive] File rename failed: {}", e);
                        reply.error(libc::EIO);
                    }
                }
            } else {
                // CASE B: MOVE FILE (Retagging)
                tracing::info!("[HollowDrive] Moving file_id={} from tag_id={} to tag_id={}", file_id, source_tag_id, dest_tag_id);
                match repo.move_file_between_tags(file_id, source_tag_id, dest_tag_id, newname_str) {
                    Ok(_) => {
                        tracing::info!("[HollowDrive] File move successful");
                        reply.ok()
                    },
                    Err(MagicError::State(_)) => {
                        tracing::error!("[HollowDrive] File move conflict");
                        reply.error(libc::EEXIST);
                    }
                    Err(e) => {
                        tracing::error!("[HollowDrive] File move failed: {}", e);
                        reply.error(libc::EIO);
                    }
                }
            }
        } else {
            reply.error(libc::EIO);
        }
    }

    // --- NEW: CREATE HANDLER (IMPORT) ---
    fn create(
        &mut self,
        _req: &Request,
        parent: u64,
        name: &std::ffi::OsStr,
        _mode: u32,
        _umask: u32,
        flags: i32,
        reply: ReplyCreate,
    ) {
        tracing::debug!("[create] Called: parent_inode={}, name={:?}", parent, name);

        // 1. Check Context: Must be a Persistent Tag OR INODE_INBOX
        let is_inbox_parent = parent == INODE_INBOX;
        tracing::debug!("[create] is_inbox_parent={}, parent={}, INODE_INBOX={}", is_inbox_parent, parent, INODE_INBOX);

        if !InodeStore::is_persistent(parent) && !is_inbox_parent {
            tracing::debug!("[create] Rejected: not persistent and not inbox");
            reply.error(libc::EACCES);
            return;
        }

        let name_str = match name.to_str() {
            Some(s) => s,
            None => { reply.error(libc::EINVAL); return; }
        };

        // 2. Determine Landing Zone (Phase 17: System-managed inbox)
        let physical_path = if is_inbox_parent {
            tracing::debug!("[create] Routing to SYSTEM INBOX for name: {}", name_str);
            // For inbox, use system inbox directory
            let system_inbox_dir = {
                let state_guard = self.state.read().unwrap();
                let inbox_path_guard = state_guard.system_inbox_path.lock().unwrap();
                if let Some(path) = inbox_path_guard.as_ref() {
                    tracing::debug!("[create] System inbox path from state: {}", path);
                    PathBuf::from(path)
                } else {
                    tracing::error!("[create] CRITICAL: System inbox path not set in GlobalState!");
                    reply.error(libc::EIO);
                    return;
                }
            };

            // Ensure directory exists
            if !system_inbox_dir.exists() {
                tracing::debug!("[create] Creating system inbox dir: {:?}", system_inbox_dir);
                if let Err(e) = std::fs::create_dir_all(&system_inbox_dir) {
                    tracing::error!("[create] Failed to create system inbox dir: {}", e);
                    reply.error(libc::EIO); return;
                }
            }

            let final_path = system_inbox_dir.join(name_str);
            tracing::debug!("[create] Final system inbox path: {:?}", final_path);
            final_path
        } else {
            tracing::warn!("[create] Routing to LEGACY _imported (parent={}, INODE_INBOX={})", parent, INODE_INBOX);
            // For regular tags, use the old _imported logic (for now)
            // This maintains backward compatibility for tag creation
            let landing_root = {
                let state_guard = self.state.read().unwrap();
                let wp = state_guard.watch_paths.lock().unwrap();
                if wp.is_empty() {
                    reply.error(libc::ENOSPC); // No storage defined
                    return;
                }
                wp[0].clone()
            };

            let import_dir = Path::new(&landing_root).join("_imported");
            if !import_dir.exists() {
                if let Err(_) = std::fs::create_dir_all(&import_dir) {
                    reply.error(libc::EIO); return;
                }
            }

            let legacy_path = import_dir.join(name_str);
            tracing::debug!("[create] Final legacy path: {:?}", legacy_path);

            // Phase 40: Ensure import directory has proper ownership
            // Note: We don't enforce on import_dir itself (might be shared), just files
            // This prevents breaking other apps that might use _imported
            legacy_path
        };
        let physical_path_str = physical_path.to_string_lossy().to_string();

        let file = match OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&physical_path)
        {
            Ok(f) => {
                tracing::debug!("[create] File opened successfully: {}", physical_path_str);
                f
            },
            Err(e) => {
                tracing::error!("[create] Failed to open file {}: {}", physical_path_str, e);
                let err = match e.kind() {
                    std::io::ErrorKind::PermissionDenied => libc::EACCES,
                    _ => libc::EIO,
                };
                reply.error(err);
                return;
            }
        };

        // 4. Get Metadata (Inode)
        let meta = match file.metadata() {
            Ok(m) => m,
            Err(_) => { reply.error(libc::EIO); return; }
        };
        
        let _physical_inode = meta.ino(); // Kept for potential debug reference
        let mtime = meta.modified().unwrap_or(SystemTime::now())
            .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();

        // Phase 40: Enforce ownership immediately after file creation
        tracing::debug!("[create] About to get identity from state for path: {}", physical_path_str);
        let identity = {
            let state_guard = self.state.read().unwrap();
            *state_guard.identity
        };
        tracing::debug!("[create] Got identity - is_root={}, uid={}, gid={}", identity.is_root, identity.uid, identity.gid);

        // Check if file exists before calling enforce_ownership
        tracing::debug!("[create] File exists before enforce_ownership: {}", physical_path.exists());

        // Robin Hood: Enforce user ownership on the created file
        if let Err(e) = identity.enforce_ownership(&physical_path) {
            tracing::error!("[create] Failed to enforce ownership: {}", e);
            // Clean up the file we just created
            let _ = std::fs::remove_file(&physical_path);
            reply.error(libc::EIO);
            return;
        }
        tracing::debug!("[create] Ownership enforcement successful for: {}", physical_path_str);

        // PHASE 25/40 FIX: Standardize on Hashed Inodes for Inbox consistency.
        // We must use the same inode calculation here as we do in lookup/readdir/rename.
        let inode = {
            let state_guard = self.state.read().unwrap();
            state_guard.inode_store.hash_to_inode(name_str)
        };

        // Register the mapping so getattr can find the physical file
        {
            let state_guard = self.state.read().unwrap();
            state_guard.inode_store.put_mirror_path(inode, physical_path_str.clone());
        }

        tracing::debug!("[create] Inbox Registered: inode={} -> path={}", inode, physical_path_str);
        println!("[MOONBUG] DB operations section starting for '{}'", physical_path_str);

        // Reacquire state guard for DB operations
        let state_guard = self.state.read().unwrap();
        let mut conn_lock = state_guard.db_connection.lock().unwrap();

        if let Some(conn) = conn_lock.as_mut() {
            let tx = match conn.transaction() {
                Ok(t) => t,
                Err(e) => {
                    println!("[MOONBUG] Transaction start FAILED: {}", e);
                    tracing::error!("[create] Transaction start failed for '{}': {}", physical_path_str, e);
                    reply.error(libc::EIO); return;
                }
            };

            // Determine tag_id for database registration
            let tag_id = if parent == INODE_INBOX {
                1u64 // Inbox is always Tag 1
            } else {
                InodeStore::inode_to_db_id(parent)
            };

            // A. Register File
            println!("[MOONBUG] About to INSERT: path='{}', inode={}, mtime={}", physical_path_str, inode, mtime);
            let file_id_res = tx.query_row(
                "INSERT INTO file_registry (abs_path, inode, mtime, size, is_dir)
                 VALUES (?1, ?2, ?3, 0, 0)
                 ON CONFLICT(abs_path) DO UPDATE SET inode=excluded.inode, mtime=excluded.mtime
                 RETURNING file_id",
                params![physical_path_str, inode, mtime],
                |r| {
                    let id = r.get::<_, i64>(0)?;
                    println!("[MOONBUG] Got file_id from query_row: {} (raw i64)", id);
                    Ok(id as u64)
                }
            );

            let file_id = match file_id_res {
                Ok(id) => {
                    println!("[MOONBUG] INSERT succeeded with file_id: {}", id);
                    id
                },
                Err(e) => {
                    println!("[MOONBUG] INSERT file_registry FAILED: {}", e);
                    tracing::error!("[create] INSERT file_registry failed for '{}': {}", physical_path_str, e);
                    reply.error(libc::EIO); return;
                }
            };

            // B. Link to Tag
            if let Err(_) = tx.execute(
                "INSERT INTO file_tags (file_id, tag_id, display_name) VALUES (?1, ?2, ?3)",
                params![file_id, tag_id, name_str]
            ) {
                // Ignore conflict if it exists
            }

            if let Err(e) = tx.commit() {
                tracing::error!("[create] Transaction commit failed for '{}': {}", physical_path_str, e);
                reply.error(libc::EIO); return;
            }
        } else {
            println!("[MOONBUG] NO DATABASE CONNECTION");
            tracing::error!("[create] No database connection for '{}'", physical_path_str);
            reply.error(libc::EIO); return;
        }

        // 6. Reply - Return attributes using the HASHED inode, not the physical one
        // We need to convert SystemTime metadata to FileAttr
        let attr = fuser::FileAttr {
            ino: inode,
            size: 0,
            blocks: 0,
            atime: SystemTime::now(),
            mtime: SystemTime::now(),
            ctime: SystemTime::now(),
            crtime: SystemTime::now(),
            kind: fuser::FileType::RegularFile,
            perm: 0o644,
            nlink: 1,
            uid: identity.uid,  // Masquerade
            gid: identity.gid,  // Masquerade
            rdev: 0,
            blksize: 4096,
            flags: 0,
        };

        println!("[MOONBUG] About to call reply.created() for '{}' inode={}", physical_path_str, inode);
        tracing::debug!("[create] Calling reply.created() with inode={}, uid={}, gid={}",
                        inode, attr.uid, attr.gid);
        reply.created(&Duration::from_secs(1), &attr, 0, 0, flags as u32);
        println!("[MOONBUG] reply.created() returned for '{}'", physical_path_str);
        tracing::debug!("[create] reply.created() returned for '{}'", physical_path_str);
    }

    // --- MKDIR: Create new tag (hierarchical) ---
    fn mkdir(
        &mut self,
        _req: &Request,
        parent: u64,
        name: &std::ffi::OsStr,
        _mode: u32,
        _umask: u32,
        reply: ReplyEntry,
    ) {
        let name_str = match name.to_str() {
            Some(s) => s,
            None => { reply.error(libc::EINVAL); return; }
        };

        // Only allow mkdir in tags directory or persistent tag directories
        if parent != INODE_TAGS && !InodeStore::is_persistent(parent) {
            reply.error(libc::EPERM); // "Operation not permitted"
            return;
        }

        // Determine parent_tag_id
        let parent_tag_id = if parent == INODE_TAGS {
            None // Root level tag
        } else {
            Some(InodeStore::inode_to_db_id(parent))
        };

        let state_guard = self.state.read().unwrap();
        let mut conn_lock = state_guard.db_connection.lock().unwrap();

        if let Some(conn) = conn_lock.as_mut() {
            let repo = Repository::new(conn);
            match repo.create_tag(name_str, parent_tag_id) {
                Ok(tag_id) => {
                    let new_inode = InodeStore::db_id_to_inode(tag_id);
                    let ttl = Duration::from_secs(1);
                    // Get identity for masquerade
                    let identity = {
                        let state_guard = self.state.read().unwrap();
                        *state_guard.identity
                    };
                    let attr = fuser::FileAttr {
                        ino: new_inode,
                        size: 0,
                        blocks: 0,
                        atime: SystemTime::now(),
                        mtime: SystemTime::now(),
                        ctime: SystemTime::now(),
                        crtime: SystemTime::now(),
                        kind: fuser::FileType::Directory,
                        perm: 0o755, // Allow write for rmdir operations
                        nlink: 2,
                        uid: identity.uid,
                        gid: identity.gid,
                        rdev: 0,
                        blksize: 4096,
                        flags: 0,
                    };
                    reply.entry(&ttl, &attr, 0);
                }
                Err(MagicError::State(msg)) if msg == "Tag exists" => {
                    reply.error(libc::EEXIST);
                }
                Err(_) => {
                    reply.error(libc::EIO);
                }
            }
        } else {
            reply.error(libc::EIO);
        }
    }

    // --- RMDIR: Delete empty tag ---
    fn rmdir(
        &mut self,
        _req: &Request,
        parent: u64,
        name: &std::ffi::OsStr,
        reply: fuser::ReplyEmpty,
    ) {
        let name_str = match name.to_str() {
            Some(s) => s,
            None => { reply.error(libc::EINVAL); return; }
        };

        // Only allow rmdir in tags directory or persistent tag directories
        if parent != INODE_TAGS && !InodeStore::is_persistent(parent) {
            reply.error(libc::EPERM);
            return;
        }

        // Determine parent_tag_id for lookup
        let parent_tag_id = if parent == INODE_TAGS {
            None  // Root level tags
        } else {
            Some(InodeStore::inode_to_db_id(parent))
        };

        let state_guard = self.state.read().unwrap();
        let mut conn_lock = state_guard.db_connection.lock().unwrap();

        if let Some(conn) = conn_lock.as_mut() {
            let repo = Repository::new(conn);

            // Get the tag_id we want to delete
            let tag_id_opt = match repo.get_tag_id_by_name(name_str, parent_tag_id) {
                Ok(id) => id,
                Err(_) => {
                    reply.error(libc::EIO);
                    return;
                }
            };

            let tag_id = match tag_id_opt {
                Some(id) => id,
                None => {
                    reply.error(libc::ENOENT);
                    return;
                }
            };

            // Use repository methods to check emptiness
            let has_children = match repo.has_child_tags(tag_id) {
                Ok(val) => val,
                Err(_) => {
                    reply.error(libc::EIO);
                    return;
                }
            };

            if has_children {
                reply.error(libc::ENOTEMPTY);
                return;
            }

            let has_files = match repo.has_files(tag_id) {
                Ok(val) => val,
                Err(_) => {
                    reply.error(libc::EIO);
                    return;
                }
            };

            if has_files {
                reply.error(libc::ENOTEMPTY);
                return;
            }

            // Safe to delete
            match repo.delete_tag(tag_id) {
                Ok(_) => reply.ok(),
                Err(MagicError::State(msg)) if msg == "Directory not empty" => {
                    reply.error(libc::ENOTEMPTY);
                }
                Err(_) => reply.error(libc::EIO),
            }
        } else {
            reply.error(libc::EIO);
        }
    }

    fn unlink(&mut self, _req: &Request, parent: u64, name: &std::ffi::OsStr, reply: fuser::ReplyEmpty) {
        // 1. Check Context: Must be a Persistent Tag
        if !InodeStore::is_persistent(parent) {
            // /search and /mirror are Read-Only for deletion
            reply.error(libc::EACCES);
            return;
        }

        let name_str = match name.to_str() {
            Some(s) => s,
            None => { reply.error(libc::EINVAL); return; }
        };

        let tag_id = InodeStore::inode_to_db_id(parent);

        let state_guard = self.state.read().unwrap();
        let mut conn_lock = state_guard.db_connection.lock().unwrap();

        if let Some(conn) = conn_lock.as_mut() {
            // 2. Resolve File ID within this specific tag (with virtual alias support)
            // Strategy: Try exact match first, then virtual alias resolution
            let mut target_file_id = None;

            // Attempt 1: Exact Match
            {
                let repo = Repository::new(conn);
                if let Ok(Some(file_id)) = repo.get_file_id_in_tag(tag_id, name_str) {
                    target_file_id = Some(file_id);
                }
            }

            // Attempt 2: Virtual Alias Resolution (if exact match failed)
            if target_file_id.is_none() {
                if let Some((base_name, index)) = Self::parse_virtual_name(name_str) {
                    // Query for all files with base_name in this tag
                    let sql = "
                        SELECT ft.file_id, ft.display_name
                        FROM file_tags ft
                        WHERE ft.tag_id = ?1 AND ft.display_name = ?2
                        ORDER BY ft.file_id ASC
                    ";

                    if let Ok(mut stmt) = conn.prepare(sql) {
                        if let Ok(rows) = stmt.query_map(params![tag_id, base_name], |row| {
                            Ok((row.get::<_, u64>(0)?, row.get::<_, String>(1)?))
                        }) {
                            // Find the Nth occurrence
                            for (i, row) in rows.enumerate() {
                                if let Ok((file_id, _display_name)) = row {
                                    if i == index {
                                        target_file_id = Some(file_id);
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            let file_id = match target_file_id {
                Some(id) => id,
                None => {
                    reply.error(libc::ENOENT);
                    return;
                }
            };

            // 3. Execute Soft Delete
            let repo = Repository::new(conn);
            match repo.unlink_file(tag_id, file_id) {
                Ok(_) => reply.ok(),
                Err(MagicError::State(_)) => reply.error(libc::ENOENT),
                Err(e) => {
                    tracing::error!("[HollowDrive] Unlink failed: {}", e);
                    reply.error(libc::EIO);
                }
            }
        } else {
            reply.error(libc::EIO);
        }
    }
}
