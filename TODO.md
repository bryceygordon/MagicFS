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
- [x] **Phase 12: The Persistence Kernel**
    - [x] **Schema v1:** `files`, `tags`, `file_tags` tables.
    - [x] **Inode Zoning:** High-Bit `PERSISTENT_FLAG` logic.
    - [x] **The Router:** `lookup/readdir` routing for Persistent vs Ephemeral inodes.
    - [x] **Basic Views:** Listing `/magic/tags` and `/magic/tags/[tag]`.
    - [x] **Write Ops:** `create` (Import), `rename` (Retagging/Aliasing).

---

## 🚧 Phase 14: The Smart Hierarchy (Active Focus)
**Ref:** `SPEC_PERSISTENCE.md` v2.0
**Goal:** Complete the translation of Unix Directory semantics into Graph Database operations.

### 1. Schema Upgrade (Spec v2.0)
- [ ] **Metadata Columns:** Update `tags` table with `color`, `icon`, `is_system`.
- [ ] **System Tags:** Ensure migration creates `Inbox` (ID 1) and `Trash` (ID 2).

### 2. Directory Structure Management (The Light Tree)
*Enabling users to organize knowledge via `mkdir` and `rmdir`.*
- [ ] **Hierarchical `mkdir`:** Implement handler in `HollowDrive`.
    - *Logic:* `INSERT INTO tags (name, parent_tag_id) VALUES (...)`.
    - *Constraint:* Prevent duplicate names in same parent.
- [ ] **Hierarchical `rmdir`:** Implement handler.
    - *Logic:* Check if empty (no sub-tags, no files). If safe, `DELETE FROM tags`.
- [ ] **Tag Reparenting (`mv folder folder`):**
    - *Logic:* Update `parent_tag_id` on the moved tag.
    - *Constraint:* Check for circular dependency.

### 3. The Inbox Workflow
- [ ] **Landing Zone Logic:** `create/cp` into `/magic/inbox` maps to `tag_id=1`.
- [ ] **Processing Logic:** `mv` from `/inbox` to `/finance` updates the `tag_id`.

---

## 🛠️ Phase 15: Safety & Garbage Collection
**Goal:** Ensure the "Permeable Garden" doesn't accumulate rot.
- [ ] **The Wastebin:** Implement `rm` -> Move to `@trash` logic.
- [ ] **The Incinerator:** Background job to physically delete files that have been in `@trash` > 30 days.
- [ ] **Orphan Collection:** Scan for `file_registry` entries with 0 tags and auto-tag as `@untagged`.

---

## 🖥️ Phase 16: The Metadata Sidecar (Future Thin Client)
**Goal:** Prepare the DB for concurrent read-access by the GUI Client.
- [ ] **Permission Hardening:** Ensure `index.db`, `.wal`, and `.shm` are readable by the user group (0664), even if Daemon runs as root.
- [ ] **Query Performance:** Add indices on `file_tags(tag_id)` and `tags(parent_tag_id)` for instant UI tree rendering.

---

## 📄 Phase 17: Universal Ingestion (Evernote Parity)
**Goal:** "Everything is a Note."
- [ ] **PDF Text Extraction:** Integrate `poppler` or `pdf-extract` into `Indexer`.
- [ ] **Image OCR:** (Long term) Integration for "Scan to Inbox".

---

## 🤖 Phase 18: Auto-Organization (The Magnet)
**Goal:** Implement "Magnetic Tags" and Semantic Gravity.
**Ref:** `SPEC_AUTO_ORGANIZATION.md`

### 1. Schema & State
- [ ] **Migration:** Add `centroid` (blob) and `description` to `tags`.
- [ ] **Migration:** Add `is_auto` to `file_tags`.
- [ ] **State:** Load Tag Centroids into memory (HashMap) on startup for fast comparison (avoiding SQL queries per file).

### 2. The Magnet Worker
- [ ] **Regex Engine:** Implement simple date/pattern scanner in `Indexer`.
- [ ] **Vector Math:** Implement Cosine Similarity check against all active Tag Centroids.
- [ ] **Auto-Linker:** Logic to `INSERT` into `file_tags` with `is_auto=1`.

### 3. The Learning Loop
- [ ] **Recalculator:** Trigger a Centroid update whenever a file is manually moved (`rename`) or confirmed.
- [ ] **Confirmation Logic:** Update `is_auto=0` on `open()` events in `HollowDrive`.
