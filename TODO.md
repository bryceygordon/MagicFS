# MagicFS Roadmap & Charter

> "The goal is not just to search; it is to maintain the illusion of a physical, infinite drive."

## 🏆 Completed Milestones
- [x] **Phase 1: The Foundation**
    - [x] FUSE Mount & Passthrough
    - [x] In-Memory Inode Store
    - [x] Async Orchestrator (The Oracle)
- [x] **Phase 10: The AI Engine**
    - [x] Nomic Embed v1.5 Integration (768 dims)
    - [x] Structure-Aware Chunking (No sentence splicing)
- [x] **Phase 11: The Illusion of Physicality**
    - [x] The Ephemeral Promise (Lazy Loading to fix Autosuggest)
    - [x] The Bouncer (Noise filtering policy)
    - [x] Typewriter & Backspace suppression (via Lazy Loading)

---

## 🚧 Phase 12: Organization & The Semantic Graph (Current Focus)
**Ref:** `SPEC_PERSISTENCE.md`
**Goal:** Implement the "Inode Router" and "Object Store" model.

### 1. Housekeeping
- [x] **Lower Memory Test:** Reduce `test_09_memory_leak` queries from 2000 to 500 (Completed in chat, needs commit).

### 2. The Foundation (Schema & Routing)
- [x] **Schema Migration:** Implement `files`, `tags`, and `file_tags` tables in SQLite.
- [x] **Inode Zoning:** Implement `PERSISTENT_OFFSET` (High-Bit logic) in `InodeStore`.
- [x] **The Router:** Modify `InodeStore::get_inode` to route High-Bit IDs to SQLite and Low-Bit IDs to RAM.

### 3. The Logical Views (Read)
- [x] **Root Generation:** Expose `/magic/tags` and `/magic/inbox` via FUSE.
- [x] **Tag Listing:** `readdir` on `/tags` queries the `tags` table.
- [x] **File Listing:** `readdir` on `/tags/foo` queries the `file_tags` table.
- [x] **Collision Resolution:** Implement the "Smart Contextual Aliasing" logic (Tag/Origin suffixes) for duplicate filenames in a view.

### 4. Semantic Operations (Write)
- [x] **Tagging (CP):** Implement `create/link` logic to insert into `file_tags`.
- [x] **Retagging (MV):** Implement `rename` logic to update `tag_id`.
- [x] **Aliasing (MV):** Implement `rename` logic to update `display_name` when source dir == dest dir.
- [ ] **The Wastebin:** Implement `@trash` tag logic and override queries to hide trashed items.

---

## 🛠️ Phase 13: Developer Experience & Stability
**Goal:** Solidify the binary for daily driving.
_this is paused until the completion of phase 14 and 15 atleast_
- [ ] **Daemonize Properly:** Ensure `systemd` service handles suspend/wake cycles correctly.
- [ ] **The "Nuke" Protocol:** A robust way to clear the cache when the AI model changes (Version pinning).
- [ ] **Log Rotation:** Prevent `magicfs.log` from eating the disk during long runs.


## 🚧 Phase 14: The Smart Hierarchy (Active Development)
**Goal:** Implement the "Tags as Folders" architecture defined in `SPEC_PERSISTENCE.md`.

### 1. Structure Management (The Light Tree)
This enables persistent directory organization.
- [ ] **Hierarchical `mkdir`:** Implement `create_directory` handler in HollowDrive.
    - Must resolve `parent` inode to `tag_id`.
    - Must `INSERT INTO tags` with `parent_tag_id`.
- [ ] **Hierarchical `readdir`:** Update `readdir` loop for `INODE_TAGS` and persistent tags.
    - *Query:* Must fetch `SELECT * FROM tags WHERE parent_tag_id = ?` (Sub-folders).
    - *Merge:* Must append these results to the file list.
- [ ] **Hierarchical `lookup`:** Update `lookup` logic.
    - Priority 1: Check for Sub-Tag (Folder).
    - Priority 2: Check for File (Existing logic).
- [ ] **Hierarchical `rmdir`:** Implement `rmdir` handler.
    - Check for children/files. If empty, `DELETE FROM tags`.

### 2. Semantic Relevance (The AI Boost)
- [ ] **Payload Decoration:** Modify `src/engine/indexer.rs`.
    - Update `index_file` to prepend `Filename: X\nTags: Y\n---\n` to the text content before chunking/embedding.
    - *Note:* This requires re-indexing. Update `CRUCIAL_LEARNINGS` about cache invalidation.

### 3. The Dark Graph (The Filter View)
*Postponed until Structure is stable.*
- [ ] **Ephemeral Inode Resolution:** Update `lookup` to handle "Global Tag" resolution if child lookup fails.
- [ ] **Intersection Context:** Create an in-memory `LruCache` mapping `EphemeralInode -> Vec<TagID>`.
- [ ] **Filtered `readdir`:** Implement `readdir` for Ephemeral Inodes that runs `INTERSECT` queries.

---

## 🗑️ Phase 15: Safety & Cleanup (Next)
- [ ] **The Wastebin:** Implement `@trash` tag logic.
- [ ] **The Untag Logic:** `unlink` removes `file_tags` row.
