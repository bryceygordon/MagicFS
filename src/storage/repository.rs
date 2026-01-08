// FILE: src/storage/repository.rs
use rusqlite::{Connection, params};
use crate::error::{Result, MagicError};
use crate::state::SearchResult;
use bytemuck;
use std::path::Path;

pub struct Repository<'a> {
    conn: &'a mut Connection,
}

impl<'a> Repository<'a> {
    pub fn new(conn: &'a mut Connection) -> Self {
        Self { conn }
    }

    pub fn initialize(&self) -> Result<()> {
        // 1. Core File Registry (The Warehouse)
        self.conn.execute_batch(r#"
            CREATE TABLE IF NOT EXISTS file_registry (
                file_id INTEGER PRIMARY KEY AUTOINCREMENT,
                abs_path TEXT NOT NULL UNIQUE,
                inode INTEGER NOT NULL,
                mtime INTEGER NOT NULL,
                size INTEGER NOT NULL DEFAULT 0,
                is_dir INTEGER NOT NULL DEFAULT 0,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );
        "#).map_err(MagicError::Database)?;

        // 2. Vector Index (Nomic Embed v1.5 / Snowflake Arctic)
        let has_vec_index: i32 = self.conn.query_row(
            "SELECT count(*) FROM sqlite_master WHERE name='vec_index'", 
            [], 
            |r| r.get(0)
        ).unwrap_or(0);

        if has_vec_index == 0 {
             match self.conn.execute_batch(r#"
                CREATE VIRTUAL TABLE IF NOT EXISTS vec_index USING vec0(
                    file_id INTEGER,
                    embedding float[768] distance_metric=cosine
                )
            "#) {
                Ok(_) => tracing::info!("[Repository] Created vec_index table (768 dim)"),
                Err(e) => tracing::warn!("[Repository] Failed to create vec_index: {}", e),
            }
        }

        // 3. Taxonomy (Tags/Folders)
        // Represents the folder structure under /magic/tags
        self.conn.execute_batch(r#"
            CREATE TABLE IF NOT EXISTS tags (
                tag_id INTEGER PRIMARY KEY AUTOINCREMENT,
                parent_tag_id INTEGER,
                name TEXT NOT NULL,
                color TEXT,
                icon TEXT,
                
                UNIQUE(parent_tag_id, name),
                FOREIGN KEY(parent_tag_id) REFERENCES tags(tag_id) ON DELETE CASCADE
            );
        "#).map_err(MagicError::Database)?;

        // 4. The Graph (File <-> Tag Edges)
        // This allows the "Multiverse": One file, multiple locations/names.
        self.conn.execute_batch(r#"
            CREATE TABLE IF NOT EXISTS file_tags (
                file_id INTEGER NOT NULL,
                tag_id INTEGER NOT NULL,
                
                -- The Multiverse: A file can have a different name in this specific tag view
                display_name TEXT, 
                
                added_at INTEGER DEFAULT (unixepoch()),
                
                PRIMARY KEY (file_id, tag_id),
                FOREIGN KEY (file_id) REFERENCES file_registry(file_id) ON DELETE CASCADE,
                FOREIGN KEY (tag_id) REFERENCES tags(tag_id) ON DELETE CASCADE
            );
        "#).map_err(MagicError::Database)?;

        // 5. Default Tags (Inbox, Trash)
        // We reserve low IDs for system tags if needed, or handle via name.
        // For now, ensure standard structure exists? No, do lazily in logic.

        Ok(())
    }

    pub fn get_file_metadata(&self, abs_path: &str) -> Result<Option<(u64, u64)>> {
        let mut stmt = self.conn.prepare("SELECT mtime, size FROM file_registry WHERE abs_path = ?1")?;
        let result = stmt.query_row(params![abs_path], |row| Ok((row.get(0)?, row.get(1)?)));
        match result {
            Ok(meta) => Ok(Some(meta)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(MagicError::Database(e)),
        }
    }

    pub fn get_all_files(&self) -> Result<Vec<(u64, String)>> {
        let mut stmt = self.conn.prepare("SELECT file_id, abs_path FROM file_registry")?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        let mut results = Vec::new();
        for r in rows { results.push(r?); }
        Ok(results)
    }

    pub fn scan_all_files<F>(&self, mut callback: F) -> Result<()>
    where F: FnMut(u64, String) -> Result<()> 
    {
        let mut stmt = self.conn.prepare("SELECT file_id, abs_path FROM file_registry")?;
        
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, u64>(0)?, row.get::<_, String>(1)?))
        })?;

        for row in rows {
            let (id, path) = row?;
            callback(id, path)?;
        }
        
        Ok(())
    }

    pub fn register_file(&self, abs_path: &str, inode: u64, mtime: u64, size: u64, is_dir: bool) -> Result<u64> {
        let mut stmt = self.conn.prepare(
            "INSERT INTO file_registry (abs_path, inode, mtime, size, is_dir)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(abs_path) DO UPDATE SET
                 mtime = excluded.mtime,
                 size = excluded.size,
                 updated_at = CURRENT_TIMESTAMP
             RETURNING file_id"
        )?;
        Ok(stmt.query_row(params![abs_path, inode, mtime, size, if is_dir { 1 } else { 0 }], |row| row.get(0))?)
    }

    pub fn delete_file(&self, abs_path: &str) -> Result<bool> {
        let rows = self.conn.execute("DELETE FROM file_registry WHERE abs_path = ?1", params![abs_path])
            .map_err(MagicError::Database)?;
        Ok(rows > 0)
    }

    pub fn delete_file_by_id(&self, file_id: u64) -> Result<()> {
        self.conn.execute("DELETE FROM vec_index WHERE file_id = ?1", params![file_id])?;
        self.conn.execute("DELETE FROM file_registry WHERE file_id = ?1", params![file_id])?;
        Ok(())
    }

    pub fn get_file_by_path(&self, abs_path: &str) -> Result<Option<crate::storage::FileRecord>> {
        let mut stmt = self.conn.prepare("SELECT file_id, abs_path, inode, mtime, size, is_dir, created_at, updated_at FROM file_registry WHERE abs_path = ?1")?;
        let result = stmt.query_row(params![abs_path], |row| {
            Ok(crate::storage::FileRecord {
                file_id: row.get(0)?, abs_path: row.get(1)?, inode: row.get(2)?,
                mtime: row.get(3)?, size: row.get(4)?, is_dir: row.get::<_, i32>(5)? != 0,
                created_at: row.get(6)?, updated_at: row.get(7)?,
            })
        });
        match result { Ok(r) => Ok(Some(r)), Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None), Err(e) => Err(MagicError::Database(e)) }
    }

    pub fn delete_embeddings_for_file(&self, file_id: u64) -> Result<()> {
        self.conn.execute("DELETE FROM vec_index WHERE file_id = ?1", params![file_id])?;
        Ok(())
    }

    // NEW: Batch Insertion for High Performance
    // Requires &mut self (and thus &mut Connection)
    pub fn insert_embeddings_batch(&mut self, file_id: u64, embeddings: Vec<Vec<f32>>) -> Result<()> {
        let tx = self.conn.transaction()?;
        {
            let mut stmt = tx.prepare("INSERT INTO vec_index (file_id, embedding) VALUES (?1, ?2)")?;
            for embedding in embeddings {
                let bytes: Vec<u8> = bytemuck::cast_slice(&embedding).to_vec();
                stmt.execute(params![file_id, bytes])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn insert_embedding(&self, file_id: u64, embedding: &[f32]) -> Result<()> {
        let bytes: Vec<u8> = bytemuck::cast_slice(embedding).to_vec();
        self.conn.execute("INSERT INTO vec_index (file_id, embedding) VALUES (?1, ?2)", params![file_id, bytes])?;
        Ok(())
    }

    pub fn search(&self, query_embedding: &[f32], limit: usize) -> Result<Vec<SearchResult>> {
        let embedding_bytes: Vec<u8> = bytemuck::cast_slice(query_embedding).to_vec();
        let sql = "SELECT fr.file_id, fr.abs_path, MIN(v.distance) as best_distance FROM (SELECT file_id, distance FROM vec_index WHERE embedding MATCH ? ORDER BY distance ASC LIMIT 100) v JOIN file_registry fr ON v.file_id = fr.file_id GROUP BY fr.file_id ORDER BY best_distance ASC LIMIT ?";
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map(params![embedding_bytes, limit], |row| {
            let abs_path: String = row.get(1)?;
            let distance: f32 = row.get::<_, f32>(2)?;
            let score = 1.0 - distance;
            let filename = Path::new(&abs_path).file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&abs_path)
                .to_string();
            Ok(SearchResult { file_id: row.get(0)?, abs_path, score, filename })
        })?;
        let mut results = Vec::new();
        for r in rows { results.push(r?); }
        Ok(results)
    }

    /// Checks if moving a tag would create a circular dependency.
    /// Returns true if circular.
    pub fn is_circular_dependency(&self, target_tag_id: u64, new_parent_id: u64) -> Result<bool> {
        let sql = "
            WITH RECURSIVE parent_chain(tag_id, parent_tag_id) AS (
                SELECT tag_id, parent_tag_id FROM tags WHERE tag_id = ?1
                UNION ALL
                SELECT t.tag_id, t.parent_tag_id
                FROM tags t
                JOIN parent_chain pc ON t.tag_id = pc.parent_tag_id
            )
            SELECT COUNT(*) FROM parent_chain WHERE tag_id = ?2
        ";
        let count: i64 = self.conn.query_row(sql, params![new_parent_id, target_tag_id], |r| r.get(0))?;
        Ok(count > 0)
    }

    pub fn create_tag(&self, name: &str, parent_id: Option<u64>) -> Result<u64> {
        let sql = "INSERT INTO tags (name, parent_tag_id) VALUES (?1, ?2)";
        self.conn.execute(sql, params![name, parent_id])?;
        Ok(self.conn.last_insert_rowid() as u64)
    }

    pub fn delete_tag(&self, tag_id: u64) -> Result<()> {
        // Check for children or files first to return ENOTEMPTY
        let children: i64 = self.conn.query_row("SELECT COUNT(*) FROM tags WHERE parent_tag_id = ?1", params![tag_id], |r| r.get(0))?;
        let files: i64 = self.conn.query_row("SELECT COUNT(*) FROM file_tags WHERE tag_id = ?1", params![tag_id], |r| r.get(0))?;

        if children > 0 || files > 0 {
            return Err(MagicError::State("Directory not empty".into()));
        }

        self.conn.execute("DELETE FROM tags WHERE tag_id = ?1", params![tag_id])?;
        Ok(())
    }

    pub fn rename_tag(&self, tag_id: u64, new_name: &str) -> Result<()> {
        self.conn.execute("UPDATE tags SET name = ?1 WHERE tag_id = ?2", params![new_name, tag_id])?;
        Ok(())
    }

    pub fn move_tag(&self, tag_id: u64, new_parent_id: u64, new_name: &str) -> Result<()> {
        if self.is_circular_dependency(tag_id, new_parent_id)? {
            return Err(MagicError::State("Circular dependency detected".into()));
        }

        // Check if destination already has a tag with the new name
        let exists_check: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM tags WHERE parent_tag_id = ?1 AND name = ?2 AND tag_id != ?3",
            params![new_parent_id, new_name, tag_id],
            |r| r.get(0)
        )?;

        if exists_check > 0 {
            return Err(MagicError::State("Tag exists".into()));
        }

        self.conn.execute(
            "UPDATE tags SET parent_tag_id = ?1, name = ?2 WHERE tag_id = ?3",
            params![new_parent_id, new_name, tag_id]
        )?;
        Ok(())
    }

    /// Moves a file from one tag to another (Retagging)
    pub fn move_file_between_tags(&mut self, file_id: u64, old_tag_id: u64, new_tag_id: u64, new_name: &str) -> Result<()> {
        let tx = self.conn.transaction()?;
        {
            // Check for name collision in destination
            let exists: i64 = tx.query_row(
                "SELECT COUNT(*) FROM file_tags WHERE tag_id = ?1 AND display_name = ?2",
                params![new_tag_id, new_name],
                |r| r.get(0)
            )?;

            if exists > 0 {
                return Err(MagicError::State("File exists".into()));
            }

            // Remove old link
            tx.execute("DELETE FROM file_tags WHERE file_id = ?1 AND tag_id = ?2", params![file_id, old_tag_id])?;

            // Create new link
            tx.execute(
                "INSERT INTO file_tags (file_id, tag_id, display_name) VALUES (?1, ?2, ?3)",
                params![file_id, new_tag_id, new_name]
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Renames a file within the same tag
    pub fn rename_file_in_tag(&self, file_id: u64, tag_id: u64, new_name: &str) -> Result<()> {
        // Check for conflicts
        let exists_check: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM file_tags WHERE tag_id = ?1 AND display_name = ?2 AND file_id != ?3",
            params![tag_id, new_name, file_id],
            |r| r.get(0)
        )?;

        if exists_check > 0 {
            return Err(MagicError::State("File exists".into()));
        }

        self.conn.execute(
            "UPDATE file_tags SET display_name = ?1 WHERE file_id = ?2 AND tag_id = ?3",
            params![new_name, file_id, tag_id]
        )?;
        Ok(())
    }

    /// Unlinks a file from a specific tag (Soft Delete).
    /// Does NOT delete the physical file or registry entry.
    pub fn unlink_file(&self, tag_id: u64, file_id: u64) -> Result<()> {
        let count = self.conn.execute(
            "DELETE FROM file_tags WHERE tag_id = ?1 AND file_id = ?2",
            params![tag_id, file_id],
        )?;

        if count == 0 {
            return Err(MagicError::State("Link not found".into()));
        }
        Ok(())
    }

    /// Get tag ID by name and parent
    pub fn get_tag_id_by_name(&self, name: &str, parent_id: Option<u64>) -> Result<Option<u64>> {
        let sql = if parent_id.is_none() {
            "SELECT tag_id FROM tags WHERE name = ?1 AND parent_tag_id IS NULL"
        } else {
            "SELECT tag_id FROM tags WHERE name = ?1 AND parent_tag_id = ?2"
        };

        let result = if let Some(pid) = parent_id {
            self.conn.query_row(sql, params![name, pid], |r| r.get(0))
        } else {
            self.conn.query_row(sql, params![name], |r| r.get(0))
        };

        match result {
            Ok(id) => Ok(Some(id)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(MagicError::Database(e)),
        }
    }

    /// Get file ID by tag and display name
    pub fn get_file_id_in_tag(&self, tag_id: u64, display_name: &str) -> Result<Option<u64>> {
        let result = self.conn.query_row(
            "SELECT file_id FROM file_tags WHERE tag_id = ?1 AND display_name = ?2",
            params![tag_id, display_name],
            |r| r.get(0)
        );

        match result {
            Ok(id) => Ok(Some(id)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(MagicError::Database(e)),
        }
    }

    /// Check if tag has any children
    pub fn has_child_tags(&self, tag_id: u64) -> Result<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM tags WHERE parent_tag_id = ?1",
            params![tag_id],
            |r| r.get(0)
        )?;
        Ok(count > 0)
    }

    /// Check if tag has any files
    pub fn has_files(&self, tag_id: u64) -> Result<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM file_tags WHERE tag_id = ?1",
            params![tag_id],
            |r| r.get(0)
        )?;
        Ok(count > 0)
    }

    /// War Mode: Toggle database performance settings
    ///
    /// # Arguments
    /// * `war_mode` - If true, enables maximum throughput settings (unsafe).
    ///                If false, enables safe, durable settings.
    pub fn set_performance_mode(&mut self, war_mode: bool) -> Result<()> {
        if war_mode {
            tracing::warn!("[Repository] 🔥 ENTERING WAR MODE (Max Performance)");
            // Maximum throughput: No disk sync, memory journal
            self.conn.execute("PRAGMA synchronous = OFF", [])?;
            self.conn.execute("PRAGMA journal_mode = MEMORY", [])?;
        } else {
            tracing::info!("[Repository] 🛡️ ENTERING PEACE MODE (Safe)");
            // Safe mode: Normal sync, WAL for concurrency
            // First checkpoint to flush memory journal to disk
            self.conn.execute("PRAGMA wal_checkpoint(TRUNCATE)", [])?;
            // Then switch to safe settings
            self.conn.execute("PRAGMA synchronous = NORMAL", [])?;
            self.conn.execute("PRAGMA journal_mode = WAL", [])?;
        }
        Ok(())
    }

    /// Scavenger: Find files that have NO tags (Orphans).
    /// Returns a vector of file_ids that are orphaned.
    pub fn get_orphans(&self, limit: usize) -> Result<Vec<u64>> {
        let mut stmt = self.conn.prepare(
            "SELECT fr.file_id FROM file_registry fr
             LEFT JOIN file_tags ft ON fr.file_id = ft.file_id
             WHERE ft.file_id IS NULL
             LIMIT ?1"
        )?;

        let rows = stmt.query_map(params![limit], |row| row.get(0))?;
        let mut orphans = Vec::new();
        for r in rows { orphans.push(r?); }
        Ok(orphans)
    }

    /// Helper to link a file to a tag (used by Scavenger).
    pub fn link_file(&self, file_id: u64, tag_id: u64, name: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO file_tags (file_id, tag_id, display_name) VALUES (?1, ?2, ?3)",
            params![file_id, tag_id, name]
        )?;
        Ok(())
    }

    /// Incinerator: Get files in trash that are older than specified threshold.
    /// Returns tuples of (file_id, display_name, added_at_timestamp).
    pub fn get_old_trash_files(&self, trash_tag_id: u64, older_than_seconds: i64) -> Result<Vec<(u64, String, i64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT ft.file_id, ft.display_name, ft.added_at
             FROM file_tags ft
             WHERE ft.tag_id = ?1 AND ft.added_at < ?2"
        )?;

        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let cutoff_time = current_time - older_than_seconds;

        let rows = stmt.query_map(params![trash_tag_id, cutoff_time], |row| {
            Ok((row.get::<_, u64>(0)?, row.get::<_, String>(1)?, row.get::<_, i64>(2)?))
        })?;

        let mut old_files = Vec::new();
        for r in rows { old_files.push(r?); }
        Ok(old_files)
    }
}
