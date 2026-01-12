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
- [x] **Phase 12: The Persistence Kernel**
    - [x] **Schema v1:** `files`, `tags`, `file_tags` tables.
    - [x] **Inode Zoning:** High-Bit `PERSISTENT_FLAG` logic.
- [x] **Phase 14: The Smart Hierarchy (Persistence v2.0)**
    - [x] **Graph Logic:** `mkdir`, `rmdir`, `mv`.
- [x] **Phase 19: Performance Engineering**
    - [x] **War Mode:** Unsafe SQLite optimizations during bulk scan.
    - [x] **Handover Protocol:** Safe `WAL` checkpointing before steady state.
- [x] **Phase 40: Identity & Ownership (Robin Hood Mode)**
    - [x] **Identity Masquerade:** `getattr` returns User UID/GID instead of Root.
    - [x] **Stable Inodes:** FNV-1a hashing guarantees consistent addressing.
    - [x] **Inbox Deletion:** `unlink` support for system inbox files.

---

## 🚧 Phase 41: System Isolation & Hygiene (Current Priority)
**Goal:** Decouple MagicFS from user watch directories and fix "Zombie" transient files.

### 1. The "Archive" Directory (Structural Isolation)
* **Problem:** Files moved from Inbox → Tag are physically moved to `[WatchDir]/_moved_from_inbox`. This pollutes user data and fails if no watch directory exists.
* **Fix:**
    * Create `~/.local/share/magicfs/archive/` (sibling to `inbox`).
    * Update `hollow_drive.rs::rename()`: When promoting from Inbox, move the physical file to `archive/` instead of `_moved_from_inbox`.
    * **Result:** A self-contained "Data Lake" (Inbox + Archive) that works even with 0 watch directories.

### 2. The Lazy Reaper (Fixing Ghosts)
* **Problem:** `.part` (Firefox) and `.lock` (Kate) files persist in the DB after they vanish from disk.
* **Constraint:** We **must not** filter these files from the indexer (they are valid files while they exist).
* **Fix:** "Verify at the Point of View."
    * **Mechanism:** In `readdir()` for Tag Views, iterate the DB results.
    * **Check:** Perform `std::fs::metadata(phys_path)` for each entry.
    * **Action:** If `ENOENT` (File Not Found), immediately delete the record from the DB and exclude it from the listing.
    * **Result:** The view is self-healing. Ghosts vanish the moment you look at them.

### 3. Database Cleanup
* **Task:** Startup migration to purge existing `.part` and `.lock` entries from `file_registry` that no longer exist on disk.

---

## 📄 Phase 17: Universal Ingestion (Evernote Parity)
**Goal:** "Everything is a Note."
- [x] **First-Class Inbox:** `/inbox` is now a writable root directory.
- [x] **The Polite Inbox:** `INODE_INBOX` mirrors physical disk; Indexer yields to active writers.
- [ ] **PDF Text Extraction:** Integrate `poppler` or `pdf-extract` into `Indexer`.
- [ ] **Image OCR:** (Long term) Integration for "Scan to Inbox".

## 🤖 Phase 18: Auto-Organization (The Magnet)
**Goal:** Implement "Magnetic Tags" and Semantic Gravity.
**Ref:** `SPEC_AUTO_ORGANIZATION.md`
- [ ] **Migration:** Add `centroid` and `description` to `tags`.
- [ ] **The Magnet Worker:** Auto-link files based on vector similarity or Regex patterns.
- [ ] **The Learning Loop:** Update centroids when users manually move files.

## 🔮 Future Horizons
- [ ] **Phase 42: The Lens (GUI)** - Native Rust UI for search.
- [ ] **Phase 43: OCR Integration** - Tesseract/Apple Vision for images.
