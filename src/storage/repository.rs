// FILE: src/storage/repository.rs
use rusqlite::{Connection, params};
use crate::error::{Result, MagicError};
use crate::state::SearchResult;
use bytemuck;

/// The Repository abstracts all database interaction.
/// It wraps a borrowed Connection, ensuring we never hold locks longer than necessary.
pub struct Repository<'a> {
    conn: &'a Connection,
}

impl<'a> Repository<'a> {
    /// Create a new Repository wrapper around an open connection
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    // ==================================================================================
    // SCHEMA & INIT
    // ==================================================================================

    /// Initialize the database schema (Tables + Virtual Tables)
    pub fn initialize(&self) -> Result<()> {
        // 1. Core Tables
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

            CREATE TABLE IF NOT EXISTS system_config (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );
        "#).map_err(MagicError::Database)?;

        // 2. Vector Index (sqlite-vec)
        // Note: we drop and recreate if the schema doesn't match our expectations during dev
        // For now, we assume if it exists, it's correct, but we migrated to cosine in Phase 6.
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
                Err(e) => tracing::warn!("[Repository] Failed to create vec_index (extension might be missing): {}", e),
            }
        }

        Ok(())
    }

    // ==================================================================================
    // FILE REGISTRY OPERATIONS
    // ==================================================================================

    pub fn register_file(&self, abs_path: &str, inode: u64, mtime: u64, size: u64, is_dir: bool) -> Result<u64> {
        let mut stmt = self.conn.prepare(
            "INSERT INTO file_registry (abs_path, inode, mtime, size, is_dir)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(abs_path) DO UPDATE SET
                 inode = excluded.inode,
                 mtime = excluded.mtime,
                 size = excluded.size,
                 is_dir = excluded.is_dir,
                 updated_at = CURRENT_TIMESTAMP
             RETURNING file_id"
        )?;

        let file_id = stmt.query_row(
            params![abs_path, inode, mtime, size, if is_dir { 1 } else { 0 }],
            |row| row.get::<_, u64>(0)
        ).map_err(MagicError::Database)?;

        Ok(file_id)
    }

    pub fn get_file_by_path(&self, abs_path: &str) -> Result<Option<crate::storage::FileRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT file_id, abs_path, inode, mtime, size, is_dir, created_at, updated_at
             FROM file_registry WHERE abs_path = ?1"
        )?;

        let result = stmt.query_row(params![abs_path], |row| {
            Ok(crate::storage::FileRecord {
                file_id: row.get(0)?,
                abs_path: row.get(1)?,
                inode: row.get(2)?,
                mtime: row.get(3)?,
                size: row.get(4)?,
                is_dir: row.get::<_, i32>(5)? != 0,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            })
        });

        match result {
            Ok(record) => Ok(Some(record)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(MagicError::Database(e)),
        }
    }

    pub fn delete_file(&self, abs_path: &str) -> Result<bool> {
        let rows = self.conn.execute("DELETE FROM file_registry WHERE abs_path = ?1", params![abs_path])
            .map_err(MagicError::Database)?;
        Ok(rows > 0)
    }

    pub fn list_files(&self) -> Result<Vec<crate::storage::FileRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT file_id, abs_path, inode, mtime, size, is_dir, created_at, updated_at FROM file_registry ORDER BY abs_path"
        )?;
        
        let iter = stmt.query_map([], |row| {
            Ok(crate::storage::FileRecord {
                file_id: row.get(0)?,
                abs_path: row.get(1)?,
                inode: row.get(2)?,
                mtime: row.get(3)?,
                size: row.get(4)?,
                is_dir: row.get::<_, i32>(5)? != 0,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            })
        })?;

        let mut records = Vec::new();
        for r in iter { records.push(r.map_err(MagicError::Database)?); }
        Ok(records)
    }

    // ==================================================================================
    // VECTOR OPERATIONS
    // ==================================================================================

    pub fn delete_embeddings_for_file(&self, file_id: u64) -> Result<()> {
        self.conn.execute("DELETE FROM vec_index WHERE file_id = ?1", params![file_id])
            .map_err(MagicError::Database)?;
        Ok(())
    }

    pub fn insert_embedding(&self, file_id: u64, embedding: &[f32]) -> Result<()> {
        let bytes: Vec<u8> = bytemuck::cast_slice(embedding).to_vec();
        self.conn.execute(
            "INSERT INTO vec_index (file_id, embedding) VALUES (?1, ?2)",
            params![file_id, bytes]
        ).map_err(MagicError::Database)?;
        Ok(())
    }

    // ==================================================================================
    // SEMANTIC SEARCH (Business Logic)
    // ==================================================================================

    pub fn search(&self, query_embedding: &[f32], limit: usize) -> Result<Vec<SearchResult>> {
        let embedding_bytes: Vec<u8> = bytemuck::cast_slice(query_embedding).to_vec();
        
        // The Aggregation Query (Logic from Phase 6)
        // Groups chunks by file_id and picks the best score.
        let sql = r#"
            SELECT 
                fr.file_id, 
                fr.abs_path, 
                MIN(v.distance) as best_distance
            FROM (
                SELECT file_id, distance 
                FROM vec_index 
                WHERE embedding MATCH ?
                ORDER BY distance ASC
                LIMIT 100
            ) v
            JOIN file_registry fr ON v.file_id = fr.file_id
            GROUP BY fr.file_id
            ORDER BY best_distance ASC
            LIMIT ?
        "#;

        let mut stmt = self.conn.prepare(sql).map_err(MagicError::Database)?;
        
        let rows = stmt.query_map(params![embedding_bytes, limit], |row| {
            let abs_path: String = row.get("abs_path")?;
            let distance: f32 = row.get("best_distance")?;
            let score = 1.0 - distance;

            let filename = std::path::Path::new(&abs_path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&abs_path)
                .to_string();

            Ok(SearchResult {
                file_id: row.get("file_id")?,
                abs_path,
                score,
                filename,
            })
        })?;

        let mut results = Vec::new();
        for r in rows {
            results.push(r.map_err(MagicError::Database)?);
        }

        Ok(results)
    }
}
