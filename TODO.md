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

- [ ] **Daemonize Properly:** Ensure `systemd` service handles suspend/wake cycles correctly.
- [ ] **The "Nuke" Protocol:** A robust way to clear the cache when the AI model changes (Version pinning).
- [ ] **Log Rotation:** Prevent `magicfs.log` from eating the disk during long runs.
