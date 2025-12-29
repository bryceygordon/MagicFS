// FILE: src/storage/repository.rs
use rusqlite::{Connection, params};
use crate::error::{Result, MagicError};
use crate::state::SearchResult;
use bytemuck;
use std::path::Path;

pub struct Repository<'a> {
    conn: &'a Connection,
}

impl<'a> Repository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    pub fn initialize(&self) -> Result<()> {
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

        let has_vec_index: i32 = self.conn.query_row(
            "SELECT count(*) FROM sqlite_master WHERE name='vec_index'", 
            [], 
            |r| r.get(0)
        ).unwrap_or(0);

        if has_vec_index == 0 {
             match self.conn.execute_batch(r#"
                CREATE VIRTUAL TABLE IF NOT EXISTS vec_index USING vec0(
                    file_id INTEGER,
                    embedding float[384] distance_metric=cosine
                )
            "#) {
                Ok(_) => tracing::info!("[Repository] Created vec_index table"),
                Err(e) => tracing::warn!("[Repository] Failed to create vec_index: {}", e),
            }
        }
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
}
