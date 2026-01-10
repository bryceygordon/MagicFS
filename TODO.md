IOIOIOIO# MagicFS Roadmap & Charter

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
- [x] **Phase 14: The Smart Hierarchy (Persistence v2.0)**
    - [x] **Schema Upgrade:** Metadata columns added.
    - [x] **Graph Logic:** `mkdir` (Create Node), `rmdir` (Prune Leaf).
    - [x] **Graph Integrity:** Circular dependency detection during `mv`.
    - [x] **Architectural Refactor:** Separated FUSE interface from SQL logic.
    - [x] **Safety Patch:** Fixed `unwrap()` panic in `readdir`.
- [x] **Phase 19: Performance Engineering**
    - [x] **War Mode:** Unsafe SQLite optimizations (`synchronous=OFF`) during initial bulk scan.
    - [x] **Batching:** Vectorized embedding (Vec<String>) and transactional inserts.
    - [x] **State Machine:** `Booting` -> `Indexing` -> `Monitoring` transitions.
    - [x] **Handover Protocol:** Safe `WAL` checkpointing before steady state.

---

## ✅ Phase 15: Safety & Garbage Collection (COMPLETE)
**Goal:** Ensure the "Permeable Garden" doesn't accumulate rot.
- [x] **The Wastebin:** Implement `rm` on a file -> Move to `@trash` logic (Soft Delete).
- [x] **The Scavenger:** Librarian job to scan for orphaned files (0 tags) and link them to `@trash`.
- [x] **The Incinerator:** Background job (Librarian) to physically delete files in `@trash` > 30 days.
- [x] **Broken Link Detection:** Librarian check for DB entries pointing to non-existent physical files.
    - [x] **Offline Detection:** `purge_orphaned_records()` runs on startup (Scenario A ✅)
    - [x] **Real-time Detection:** `handle_file_event(Remove)` → `Indexer::remove_file()` via inotify (Scenario B ✅)
    - [x] **Test Coverage:** `tests/cases/test_32_broken_links.py` validates both mechanisms

## 🔒 Phase 16: The Wastebin (Completed)
**Goal:** Implement unlink with full virtual alias support and soft delete.
- [x] **Safety Check:** Persistent tag boundaries enforced.
- [x] **Resolution:** Virtual aliases ("file (1).txt") handled correctly.
- [x] **Soft Delete:** Semantic link removed, data preserved.
- [x] **Protection:** Non-tag views (`/search`, `/mirror`) protected.

---

## 🖥️ Phase 16: The Metadata Sidecar (Future Thin Client)
**Goal:** Prepare the DB for concurrent read-access by the GUI Client.
- [x] **Permission Hardening:** Ensure `index.db`, `.wal`, and `.shm` are readable by the user group (0664), even if Daemon runs as root.
- [x] **Query Performance:** Add indices on `file_tags(tag_id)` and `tags(parent_tag_id)` for instant UI tree rendering.

---

## ✅ Phase 20: End-to-End Verification (COMPLETE)
**Goal:** Validate "Illusion of Physicality" via manual user journey testing.
- [x] **Manual User Journey:** Validated Mirror, Search, Tagging, and Soft Delete workflows.
- [x] **Stability Check:** Verified War Mode/Peace Mode transitions during live usage.
- [x] **Semantic Search Validation:** Confirmed vector-based retrieval with high relevance scores.
- [x] **Safety Guarantees:** Verified Soft Delete preserves physical data.
- [x] **Bouncer Validation:** Confirmed noise filtering and read-only enforcement.

---

## 📄 Phase 17: Universal Ingestion (Evernote Parity)
**Goal:** "Everything is a Note."
- [x] **First-Class Inbox:** `/inbox` is now a writable root directory.
- [x] **Landing Zone:** Files dropped in Inbox are auto-tagged with ID 1 and physically stored.
- [x] **The Black Hole Inbox:** System-managed ingestion zone with auto-tagging (Tag ID 1).
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

## ✅ Phase 21: Flattening the Hierarchy (UX Architecture)
- [x] **First-Class Citizens:** Moved `/inbox` and `/tags` to filesystem root for zero-click access.

## ✅ Phase 22: Stability & Harness Hardening
- [x] **WAL-Safe Testing:** Converted all Python tests to use `subprocess` for DB reads.
- [x] **Omnibus Repair:** Fixed permissions, paths, and logic gaps in the new root hierarchy.

- [x] **Phase 23: Transient File Suppression:** Librarian filters `.part/.tmp`, Indexer handles vanished files gracefully.
- [x] **Phase 24: Zero-Byte Citizenship:** Removed retry loop for empty files; treated as valid content.
- [x] **Phase 24.1: Hardening Regression:** Restored Bouncer logic to reject non-empty files yielding empty text (binaries).
- [x] **Phase 25: The Polite Inbox:** `INODE_INBOX` mirrors physical disk; Indexer yields to active writers.
- [x] **Regression Fix:** Investigate `SQLITE_BUSY` errors in integration tests caused by concurrent Daemon/Test DB access.
- [x] **Phase 26: Unlocking Taxonomy:** Enabled `0o755` permissions on `/magic/tags` to allow GUI `mkdir`.
- [ ] **Harness Repair:** Fix `SQLITE_BUSY` in integration tests by adding timeouts to CLI invocations.
