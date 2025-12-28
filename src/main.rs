// src/main.rs

use magicfs::*;
use magicfs::{oracle::Oracle, librarian::Librarian, hollow_drive::HollowDrive};
use anyhow::Result;
use std::env;
use std::sync::Arc;
use fuser::{mount2, MountOption};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    tracing::info!("=");
    tracing::info!("MagicFS Starting Up...");
    tracing::info!("Phase 5: The Watcher (Final Refinement)");
    tracing::info!("=");

    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <mountpoint> [watch_dir]", args[0]);
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
    let db_path = PathBuf::from("/tmp").join(".magicfs").join("index.db");
    init_connection(&global_state, db_path.to_str().unwrap())?;
    tracing::info!("✓ Database initialized: {}", db_path.display());

    // ========== INITIALIZE ORACLE (Async Brain) ==========
    let mut oracle = Oracle::new(Arc::clone(&global_state))?;
    // This now starts the dedicated Embedding Actor thread
    oracle.start()?; 
    tracing::info!("✓ Oracle (async brain + embedding actor) started");

    // ========== INITIALIZE LIBRARIAN (Background Watcher) ==========
    let mut librarian = Librarian::new(Arc::clone(&global_state));
    librarian.add_watch_path(watch_dir.clone())?;
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
