// FILE: src/core/bouncer.rs
//! The Bouncer: Distinguishes between Human Intent and System Noise.
//! 
//! System Noise is defined as:
//! 1. Hidden files (start with .)
//! 2. Backup files (end with ~)
//! 3. Known OS metadata files (Thumbs.db, desktop.ini)
//! 4. Binary/Archive containers that shouldn't be "navigated" via search.

const IGNORED_EXACT: &[&str] = &[
    "thumbs.db",
    "ehthumbs.db", 
    "desktop.ini",
    "icon?",
    "folder.jpg",
    "autorun.inf",
    "$recycle.bin",
    "system volume information",
];

const IGNORED_EXTENSIONS: &[&str] = &[
    // Archives - We don't want to trigger searches for these
    "zip", "tar", "gz", "rar", "7z", "iso", "dmg",
    // Binaries/System
    "exe", "dll", "so", "dylib", "sys", "cab", "msi",
    // Swap/Temp
    "swp", "tmp", "bak", "ds_store", "partial", "crdownload"
];

pub struct Bouncer;

impl Bouncer {
    /// Decides if a query is "System Noise" or "Human Intent".
    pub fn is_noise(name: &str) -> bool {
        let name_lower = name.to_lowercase();

        // 1. Hidden / Backup Files
        if name.starts_with('.') || name.ends_with('~') {
            return true;
        }

        // 2. Exact Match (OS Metadata)
        if IGNORED_EXACT.contains(&name_lower.as_str()) {
            return true;
        }

        // 3. Extension Check
        // We scan from the *last* dot.
        if let Some(idx) = name_lower.rfind('.') {
            // Ensure there is something after the dot
            if idx + 1 < name_lower.len() {
                let ext = &name_lower[idx+1..];
                if IGNORED_EXTENSIONS.contains(&ext) {
                    return true;
                }
            }
        }
        
        // 4. "New Folder" artifacts from GUIs trying to create dirs
        if name_lower.starts_with("new folder") {
            return true;
        }

        false
    }
}
