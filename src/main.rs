//! MagicFS: Semantic Virtual Filesystem
//!
//! Single-process binary implementing three isolated "Organs":
//! - Hollow Drive (FUSE loop - synchronous, non-blocking)
//! - Oracle (async brain - handles vector search & embeddings)
//! - Librarian (background watcher - updates index)
//!
//! SYSTEM REALIZATION PROTOCOL v1.0
//! CRITICAL: Every line filtered through "Will this block the FUSE loop for >10ms?"

use magicfs::*;
use magicfs::{oracle::Oracle, librarian::Librarian, hollow_drive::HollowDrive};
use anyhow::Result;
use std::env;
use std::sync::Arc;
use fuser::{mount2, MountOption};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    tracing::info!("=");
    tracing::info!("MagicFS Starting Up...");
    tracing::info!("Phase 1: The Foundation");
    tracing::info!("=");

    // Parse command line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <mountpoint> [watch_dir]", args[0]);
        eprintln!("Example: {} /tmp/magicfs /home/user/documents", args[0]);
        return Ok(());
    }

    let mountpoint: PathBuf = args[1].clone().into();
    let watch_dir = args.get(2).map(|s| s.clone()).unwrap_or_else(|| {
        env::current_dir().unwrap().to_string_lossy().to_string()
    });

    tracing::info!("Mountpoint: {}", mountpoint.display());
    tracing::info!("Watch directory: {}", watch_dir);

    // ========== INITIALIZE GLOBAL STATE ==========
    let global_state = SharedState::new(magicfs::GlobalState::new().into());
    tracing::info!("✓ Global State initialized");

    // ========== INITIALIZE DATABASE ==========
    // IMPORTANT: Database must be OUTSIDE the FUSE mount to avoid chicken-and-egg problem
    // If database is inside mount point, FUSE hides the real filesystem and we can't create it
    let db_path = PathBuf::from("/tmp").join(".magicfs").join("index.db");
    init_connection(&global_state, db_path.to_str().unwrap())?;
    tracing::info!("✓ Database initialized: {}", db_path.display());

    // ========== INITIALIZE ORACLE (Async Brain) ==========
    let mut oracle = Oracle::new(Arc::clone(&global_state))?;
    oracle.start()?;
    oracle.init_embedding_model()?;
    tracing::info!("✓ Oracle (async brain) started");

    // ========== INITIALIZE LIBRARIAN (Background Watcher) ==========
    let mut librarian = Librarian::new(Arc::clone(&global_state));
    librarian.add_watch_path(watch_dir.clone())?;
    librarian.start()?;
    tracing::info!("✓ Librarian (watcher) started");
    tracing::info!("  Watching: {}", watch_dir);

    // ========== INITIALIZE HOLLOW DRIVE (FUSE Loop) ==========
    let mut hollow_drive = HollowDrive::new(global_state);
    tracing::info!("✓ Hollow Drive (FUSE) ready");

    tracing::info!("=");
    tracing::info!("All Organs Online - Mounting FUSE...");
    tracing::info!("=");

    // Mount the FUSE filesystem
    // Note: In a real implementation, we would handle signals for clean shutdown
    // and properly coordinate between the async runtime and FUSE loop

    let mount_options = vec![
        MountOption::AllowOther,
        MountOption::AutoUnmount,
    ];

    match mount2(
        hollow_drive,
        &mountpoint,
        &mount_options,
    ) {
        Ok(_) => {
            tracing::info!("FUSE mounted successfully");
        }
        Err(e) => {
            tracing::error!("FUSE mount failed: {}", e);
            // Note: We would properly shutdown Oracle and Librarian here
        }
    }

    tracing::info!("MagicFS shutting down...");
    Ok(())
}

// Helper function to test the system without mounting (for testing)
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_initialization() {
        let state = SharedState::new(magicfs::GlobalState::new());
        let oracle = magicfs::Oracle::new(Arc::clone(&state)).unwrap();

        assert!(oracle.runtime != Arc::new(tokio::runtime::Runtime::new().unwrap()));
    }
}