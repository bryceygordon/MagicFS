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
    // FIX: Set a default filter if RUST_LOG is not set
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("magicfs=debug,info")); // Force debug for our crate

    fmt().with_env_filter(filter).init();

    tracing::info!("=");
    tracing::info!("MagicFS Starting Up...");
    tracing::info!("Branch: experiment/nomic-embed-v1.5");
    tracing::info!("Model: Nomic v1.5 (768 dims)");
    tracing::info!("=");

    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <mountpoint> [watch_dirs]", args[0]);
        return Ok(());
    }

    let mountpoint_str = args[1].clone();
    let mountpoint: PathBuf = mountpoint_str.clone().into();
    
    let watch_dirs_input = args.get(2).map(|s| s.clone()).unwrap_or_else(|| {
        env::current_dir().unwrap().to_string_lossy().to_string()
    });

    let watch_dirs: Vec<PathBuf> = watch_dirs_input
        .split(',')
        .map(|s| PathBuf::from(s.trim()))
        .collect();

    tracing::info!("Mountpoint: {}", mountpoint.display());
    tracing::info!("Watch directories: {:?}", watch_dirs);

    // Safety Checks
    let abs_mount = std::fs::canonicalize(&mountpoint).unwrap_or(mountpoint.clone());
    for watch_dir in &watch_dirs {
        let abs_watch = std::fs::canonicalize(watch_dir).unwrap_or(watch_dir.clone());
        if abs_watch.starts_with(&abs_mount) {
            tracing::error!("FATAL: Feedback Loop Detected!");
            panic!("Feedback Loop Detected");
        }
    }

    // Initialize State
    let global_state = SharedState::new(magicfs::GlobalState::new().into());
    
    // Populate watch_paths in state
    {
        let state_guard = global_state.read().unwrap();
        let mut wp = state_guard.watch_paths.lock().unwrap();
        for p in &watch_dirs {
            wp.push(p.to_string_lossy().to_string());
        }
    }

    // --- ISOLATION UPDATE: Separate DB directory for Nomic v1.5 ---
    let db_dir = PathBuf::from("/tmp").join(".magicfs_nomic");
    let db_path = db_dir.join("index.db");
    
    // Ensure dir exists before init_connection (safety check)
    std::fs::create_dir_all(&db_dir).unwrap();
    
    init_connection(&global_state, db_path.to_str().unwrap())?;

    let mut oracle = Oracle::new(Arc::clone(&global_state))?;
    oracle.start()?; 

    let mut librarian = Librarian::new(Arc::clone(&global_state));
    for path in watch_dirs {
        librarian.add_watch_path(path.to_string_lossy().to_string())?;
    }
    librarian.start()?;

    let hollow_drive = HollowDrive::new(global_state);

    let mount_options = vec![MountOption::AllowOther, MountOption::AutoUnmount];
    match mount2(hollow_drive, &mountpoint, &mount_options) {
        Ok(_) => tracing::info!("FUSE mounted successfully"),
        Err(e) => tracing::error!("FUSE mount failed: {}", e),
    }

    Ok(())
}
