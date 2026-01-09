#!/bin/bash
# Create test database manually using SQLite CLI
# This simulates the current repository.rs initialization WITHOUT the new indices

DB_PATH="/tmp/.magicfs_nomic/index.db"
rm -f "$DB_PATH"

sqlite3 "$DB_PATH" << 'EOF'
PRAGMA foreign_keys = ON;

-- file_registry
CREATE TABLE file_registry (
    file_id INTEGER PRIMARY KEY AUTOINCREMENT,
    abs_path TEXT NOT NULL UNIQUE,
    inode INTEGER NOT NULL,
    mtime INTEGER NOT NULL,
    size INTEGER NOT NULL DEFAULT 0,
    is_dir INTEGER NOT NULL DEFAULT 0,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- tags
CREATE TABLE tags (
    tag_id INTEGER PRIMARY KEY AUTOINCREMENT,
    parent_tag_id INTEGER,
    name TEXT NOT NULL,
    color TEXT,
    icon TEXT,
    UNIQUE(parent_tag_id, name),
    FOREIGN KEY(parent_tag_id) REFERENCES tags(tag_id) ON DELETE CASCADE
);

-- file_tags
CREATE TABLE file_tags (
    file_id INTEGER NOT NULL,
    tag_id INTEGER NOT NULL,
    display_name TEXT,
    added_at INTEGER DEFAULT (unixepoch()),
    PRIMARY KEY (file_id, tag_id),
    FOREIGN KEY (file_id) REFERENCES file_registry(file_id) ON DELETE CASCADE,
    FOREIGN KEY (tag_id) REFERENCES tags(tag_id) ON DELETE CASCADE
);

-- vec_index (virtual table, can't be created without the extension, skip for now)
EOF

echo "✅ Test database created at: $DB_PATH"
echo "Schema created WITHOUT the Phase 16 indices."
echo ""
echo "Current indices:"
sqlite3 "$DB_PATH" "SELECT name FROM sqlite_master WHERE type='index';"