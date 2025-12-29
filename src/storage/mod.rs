// FILE: src/storage/mod.rs
pub mod connection;
pub mod text_extraction;
pub mod repository;

// Common exports
pub use repository::Repository;
pub use connection::init_connection; // Kept for main.rs bootstrap
pub use text_extraction::extract_text_from_file;

// Data Types
#[derive(Debug, Clone)]
pub struct FileRecord {
    pub file_id: u64,
    pub abs_path: String,
    pub inode: u64,
    pub mtime: u64,
    pub size: u64,
    pub is_dir: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl std::fmt::Display for FileRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} (inode: {}, size: {})", self.abs_path, self.inode, self.size)
    }
}
