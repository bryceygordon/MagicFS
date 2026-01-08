// FILE: src/storage/connection.rs
//! Database connection management
//!
//! Provides lazy initialization and connection management for the database.
//! Works with GlobalState's db_connection field.
//! Uses Repository for schema initialization.

use std::sync::Arc;
use rusqlite::Connection;
use crate::SharedState;
use crate::storage::Repository;
use std::os::unix::fs::PermissionsExt;
use std::fs;

/// Get the real user ID from SUDO_UID environment variable
/// Returns Some((uid, gid)) if running under sudo, None otherwise
fn get_real_user_id() -> Option<(u32, u32)> {
    use std::env;

    let sudo_uid = env::var("SUDO_UID").ok()?;
    let sudo_gid = env::var("SUDO_GID").ok()?;

    let uid: u32 = sudo_uid.parse().ok()?;
    let gid: u32 = sudo_gid.parse().ok()?;

    Some((uid, gid))
}

/// Fix permissions on database files to allow real user access
/// When SQLite runs in WAL mode, it creates -shm and -wal files owned by the process user (root)
/// This function changes ownership to the real user so external tools can read the database
fn fix_db_permissions(db_path: &str) -> crate::error::Result<()> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    // Check if we're running under sudo
    if let Some((uid, gid)) = get_real_user_id() {
        tracing::info!("[Permission Hardening] Running as sudo, fixing DB file ownership for real user (uid: {}, gid: {})", uid, gid);

        let db_path_obj = std::path::Path::new(db_path);
        let db_dir = db_path_obj.parent().unwrap_or_else(|| std::path::Path::new("."));
        let db_name = db_path_obj.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("index.db");

        // Fix permissions for main db and WAL files
        let files_to_fix = vec![
            db_name.to_string(),
            format!("{}-shm", db_name),
            format!("{}-wal", db_name),
        ];

        for file_name in files_to_fix {
            let file_path = db_dir.join(file_name);

            // Skip if file doesn't exist (e.g., first run before WAL files created)
            if !file_path.exists() {
                continue;
            }

            // Change ownership using libc::chown
            let path_cstr = match CString::new(file_path.as_os_str().as_bytes()) {
                Ok(cstr) => cstr,
                Err(_) => {
                    tracing::warn!("[Permission Hardening] Failed to convert path to C string: {}", file_path.display());
                    continue;
                }
            };

            let result = unsafe { libc::chown(path_cstr.as_ptr(), uid, gid) };

            if result != 0 {
                // Get the error code
                let errno = unsafe { *libc::__errno_location() };
                tracing::warn!("[Permission Hardening] chown() failed for {}: errno={}. Using chmod fallback.", file_path.display(), errno);

                // Set permissions to 0664 (rw-rw-r--)
                if let Ok(metadata) = fs::metadata(&file_path) {
                    let mut perms = metadata.permissions();
                    perms.set_mode(0o664);
                    if let Err(e2) = fs::set_permissions(&file_path, perms) {
                        tracing::error!("[Permission Hardening] Failed to chmod {}: {}", file_path.display(), e2);
                    } else {
                        tracing::debug!("[Permission Hardening] Set permissions 0664 on: {}", file_path.display());
                    }
                }
            } else {
                tracing::debug!("[Permission Hardening] Changed ownership of {} to {}:{}", file_path.display(), uid, gid);
            }
        }
    } else {
        tracing::debug!("[Permission Hardening] Not running under sudo, skipping permission fixes");
    }

    Ok(())
}

/// Register sqlite-vec extension with SQLite
fn register_sqlite_vec_extension() -> crate::error::Result<()> {
    unsafe {
        let result = rusqlite::ffi::sqlite3_auto_extension(Some(
            std::mem::transmute(sqlite_vec::sqlite3_vec_init as *const ())
        ));

        if result != rusqlite::ffi::SQLITE_OK {
            let err = rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(result),
                Some("Failed to register sqlite-vec extension".to_string())
            );
            return Err(crate::error::MagicError::Database(err));
        }
        tracing::info!("Successfully registered sqlite-vec extension");
    }
    Ok(())
}

/// Initialize the database connection in GlobalState
pub fn init_connection(state: &SharedState, db_path: &str) -> crate::error::Result<()> {
    register_sqlite_vec_extension()?;

    let db_dir = std::path::Path::new(db_path).parent()
        .ok_or_else(|| crate::error::MagicError::InvalidPath("Invalid database path".into()))?;

    std::fs::create_dir_all(db_dir)
        .map_err(crate::error::MagicError::Io)?;

    let mut conn = Connection::open(db_path)
        .map_err(crate::error::MagicError::Database)?;

    // Performance & Concurrency Pragmas
    // WAL mode allows readers (Searcher) not to block writers (Indexer)
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    
    // CRITICAL FIX: Set busy_timeout to 5000ms.
    conn.busy_timeout(std::time::Duration::from_millis(5000))?;

    // Use Repository to init schema
    // UPDATED: Pass &mut conn
    let repo = Repository::new(&mut conn);
    repo.initialize()?;

    // Store in global state
    let mut guard = state.write().map_err(|_| crate::error::MagicError::State("Poisoned lock".into()))?;
    guard.db_connection = Arc::new(std::sync::Mutex::new(Some(conn)));

    // Apply permission hardening after storing connection
    // This ensures the -shm and -wal files created by WAL mode are accessible to the real user
    if let Err(e) = fix_db_permissions(db_path) {
        tracing::warn!("[Permission Hardening] Failed to fix permissions: {}", e);
    }

    Ok(())
}

pub fn get_connection(state: &SharedState) -> crate::error::Result<Arc<std::sync::Mutex<Option<Connection>>>> {
    let guard = state.read().map_err(|_| crate::error::MagicError::State("Poisoned lock".into()))?;
    Ok(guard.db_connection.clone())
}
