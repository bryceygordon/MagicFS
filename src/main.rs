// src/main.rs

use magicfs::*;
use magicfs::{oracle::Oracle, librarian::Librarian, hollow_drive::HollowDrive};
use anyhow::Result;
use std::env;
use std::sync::Arc;
use fuser::{mount2, MountOption};
use std::path::PathBuf;
use tracing_subscriber::{fmt, EnvFilter};

#[tokio::main]
async fn main() -> Result<()> {
    // FIX: Explicitly load RUST_LOG env var
    fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    tracing::info!("=");
    tracing::info!("MagicFS Starting Up...");
    tracing::info!("Phase 7: The Universal Reader");
    tracing::info!("=");

    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <mountpoint> [watch_dir]", args[0]);
        return Ok(());
    }

    let mountpoint_str = args[1].clone();
    let mountpoint: PathBuf = mountpoint_str.clone().into();
    
    let watch_dir_str = args.get(2).map(|s| s.clone()).unwrap_or_else(|| {
        env::current_dir().unwrap().to_string_lossy().to_string()
    });
    let watch_dir = PathBuf::from(&watch_dir_str);

    tracing::info!("Mountpoint: {}", mountpoint.display());
    tracing::info!("Watch directory: {}", watch_dir.display());

    // ========== SAFETY CHECKS (Anti-Feedback Switch) ==========
    let abs_mount = std::fs::canonicalize(&mountpoint).unwrap_or(mountpoint.clone());
    let abs_watch = std::fs::canonicalize(&watch_dir).unwrap_or(watch_dir.clone());

    tracing::debug!("Safety Check: Mount={:?}, Watch={:?}", abs_mount, abs_watch);

    if abs_watch.starts_with(&abs_mount) {
        tracing::error!("FATAL: Feedback Loop Detected!");
        panic!("Feedback Loop Detected: Watch dir is inside Mount point.");
    }

    if abs_mount.starts_with(&abs_watch) {
         tracing::warn!("⚠️  WARNING: Mount point is inside Watch Directory.");
    }

    // ========== INITIALIZE GLOBAL STATE ==========
    let global_state = SharedState::new(magicfs::GlobalState::new().into());
    tracing::info!("✓ Global State initialized");

    // ========== INITIALIZE DATABASE ==========
    let db_path = PathBuf::from("/tmp").join(".magicfs").join("index.db");
    init_connection(&global_state, db_path.to_str().unwrap())?;
    tracing::info!("✓ Database initialized: {}", db_path.display());

    // ========== INITIALIZE ORACLE (Async Brain) ==========
    let mut oracle = Oracle::new(Arc::clone(&global_state))?;
    oracle.start()?; 
    tracing::info!("✓ Oracle (async brain + embedding actor) started");

    // ========== INITIALIZE LIBRARIAN (Background Watcher) ==========
    let mut librarian = Librarian::new(Arc::clone(&global_state));
    librarian.add_watch_path(watch_dir_str)?;
    librarian.start()?;
    tracing::info!("✓ Librarian (watcher) started");

    // ========== INITIALIZE HOLLOW DRIVE (FUSE Loop) ==========
    let hollow_drive = HollowDrive::new(global_state);
    tracing::info!("✓ Hollow Drive (FUSE) ready");

    tracing::info!("=");
    tracing::info!("All Organs Online - Mounting FUSE...");
    tracing::info!("=");

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
        }
    }

    tracing::info!("MagicFS shutting down...");
    Ok(())
}
