================================================
FILE: TODO.md
================================================
# MagicFS Task List

## ✅ Completed Phases
* [x] **Phases 1-5**: Foundation, FUSE Loop, Basic Search.
* [x] **Phase 6**: Hardening, Binary Safety, Chunking.

---

## ✅ Phase 6.5: The Foundation (Stability & Scalability) [COMPLETED]

**Objective:** Scale to 10k files and 1 week uptime without crashing or freezing.

### 6.5.1: Incremental Indexing (Stop the Storm)
* [x] **DB Update**: Ensure `mtime` is accurately stored in `file_registry`.
* [x] **Librarian Logic**:
    * [x] Modify `scan_directory_for_files` to query the DB for the file's current `mtime`.
    * [x] If `fs_mtime == db_mtime`, skip queuing.
    * [x] Log skipped files as `DEBUG` only (reduce log noise).
* [x] **Check**: Restarting the daemon on a watched folder should result in **0** embedding operations.

### 6.5.2: State Consistency (Kill Zombies)
* [x] **The Purge**:
    * [x] Implement `Repository::get_all_files()`.
    * [x] On startup, iterate all DB files. If `!Path::exists()`, delete from DB.
* [x] **Retroactive Ignore**:
    * [x] When `.magicfsignore` changes, trigger a scan.
    * [x] If a file currently in DB matches a *new* ignore rule, delete it from DB.

### 6.5.3: Memory Hygiene (LRU)
* [x] **InodeStore Refactor**:
    * [x] Replace `DashMap` for `results` with `Mutex<LruCache>`.
    * [x] Set capacity to ~50 active queries.
* [x] **Oracle Cache**:
    * [x] Use `LruCache` for `processed_queries` (Cap 1000).
    * [x] Ensure we don't track infinite history.

### 6.5.4: Stress Testing
* [x] **Script**: Create `tests/cases/test_00_stress.py`.
    * [x] Generate 50 files.
    * [x] Measure time to index.
    * [x] Restart daemon -> Verify 0 re-indexes.
    * [x] **Cache Thrashing**: Send 100 unique queries to verify LRU eviction stability.

---

## 🚀 Phase 7: The Universal Reader [ACTIVE]

**Objective:** Break the format barrier. Support PDF, DOCX, and other rich media.

* [ ] **Dependencies**: Add `pdf-extract`.
* [ ] **Extractor Refactor**: Route file types to specific parsers in `src/storage/text_extraction.rs`.
* [ ] **Test**: Create `tests/cases/test_06_rich_media.py`.

## 🔮 Phase 8: Aggregation [PENDING]

* [ ] **Config**: `~/.config/magicfs/sources.json`.
* [ ] **Virtual Dirs**: `/sources` and `/saved` endpoints.
