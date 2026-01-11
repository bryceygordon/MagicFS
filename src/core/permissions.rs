// FILE: src/core/permissions.rs
//! Identity Management and Ownership Enforcement
//!
//! Implements the "Robin Hood Protocol" for seamless user/root mode switching.

use std::path::Path;
use anyhow::Result;
use std::ffi::CString;
use std::os::unix::ffi::OsStrExt; // Required for handling paths as bytes

/// Captures user identity for ownership management
#[derive(Debug, Clone, Copy)]
pub struct Identity {
    pub uid: u32,
    pub gid: u32,
    pub is_root: bool,
}

impl Identity {
    /// Capture current or sudo identity
    pub fn capture() -> Self {
        // ... (Keep existing implementation unchanged) ...
        let (uid, gid, is_root) = if let (Some(sudo_uid), Some(sudo_gid)) =
            (std::env::var("SUDO_UID").ok(), std::env::var("SUDO_GID").ok()) {
            
            let uid = sudo_uid.parse::<u32>().unwrap_or_else(|_| Self::get_current_uid());
            let gid = sudo_gid.parse::<u32>().unwrap_or_else(|_| Self::get_current_gid());
            let is_root = Self::get_current_uid() == 0;

            tracing::info!("Robin Hood Mode: Running as root (UID 0), serving as UID:{} GID:{}", uid, gid);
            (uid, gid, is_root)

        } else {
            let uid = Self::get_current_uid();
            let gid = Self::get_current_gid();
            let is_root = uid == 0;

            if !is_root {
                tracing::info!("Just Works Mode: Running as user UID:{} GID:{}", uid, gid);
            } else {
                tracing::warn!("Running as root without sudo environment variables");
            }

            (uid, gid, is_root)
        };

        Self { uid, gid, is_root }
    }

    fn get_current_uid() -> u32 { unsafe { libc::getuid() } }
    fn get_current_gid() -> u32 { unsafe { libc::getgid() } }

    /// Enforce ownership on a file path
    pub fn enforce_ownership(&self, path: &Path) -> Result<()> {
        tracing::debug!("[enforce_ownership] called for path: {}, is_root={}, uid={}, gid={}",
            path.display(), self.is_root, self.uid, self.gid);

        if !self.is_root {
            tracing::debug!("[enforce_ownership] User mode: skipping chown");
            return Ok(());
        }

        if !path.exists() {
            // It's possible the file was moved/deleted in a race, or we have a bad path
            tracing::error!("[enforce_ownership] File does not exist: {}", path.display());
            return Err(anyhow::anyhow!("File does not exist: {}", path.display()));
        }

        // SAFETY FIX: Convert Path to CString (Null Terminated)
        // usage of path.as_os_str().as_bytes() requires std::os::unix::ffi::OsStrExt
        let c_path = CString::new(path.as_os_str().as_bytes())
            .map_err(|e| anyhow::anyhow!("Path contains null byte: {}", e))?;

        // Robin Hood: Change ownership to the original user
        let result = unsafe {
            libc::chown(
                c_path.as_ptr(), // Now safe: points to \0 terminated buffer
                self.uid,
                self.gid
            )
        };

        if result == 0 {
            tracing::debug!("Enforced ownership on {}: UID:{} GID:{}", 
                path.display(), self.uid, self.gid);
            Ok(())
        } else {
            let err = std::io::Error::last_os_error();
            tracing::error!("Failed to enforce ownership on {}: {}", path.display(), err);
            Err(anyhow::anyhow!("chown failed: {}", err))
        }
    }

    /// Enforce ownership with recursive directory handling
    pub fn enforce_ownership_recursive(&self, path: &Path) -> Result<()> {
         // ... (Keep existing implementation unchanged) ...
        if !self.is_root {
            return Ok(());
        }

        self.enforce_ownership(path)?;

        if path.is_dir() {
            if let Ok(entries) = std::fs::read_dir(path) {
                for entry in entries {
                    if let Ok(entry) = entry {
                        self.enforce_ownership_recursive(&entry.path())?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Get mount options based on identity
    pub fn get_mount_options(&self) -> Vec<fuser::MountOption> {
        if self.is_root {
            vec![fuser::MountOption::AllowOther, fuser::MountOption::AutoUnmount]
        } else {
            vec![fuser::MountOption::AutoUnmount]
        }
    }
}
