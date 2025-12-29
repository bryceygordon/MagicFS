# MagicFS Task List

## ✅ Completed Phases
* [x] **Phases 1-5**: Foundation, FUSE Loop, Basic Search.
* [x] **Phase 6**: Hardening, Binary Safety, Chunking.

---

## 🚧 Phase 6.5: The Foundation [ACTIVE]

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
* [ ] **InodeStore Refactor**:
    * [ ] Replace `DashMap` for `results` with `moka` or a custom `Mutex<LruCache>`.
    * [ ] Set capacity to ~50-100 queries.
* [ ] **Oracle Cache**:
    * [ ] Ensure `processed_queries` set doesn't grow infinitely (clear it periodically or use LRU).

### 6.5.4: Stress Testing
* [ ] **Script**: Create `tests/cases/test_00_stress.py`.
    * [ ] Generate 1,000 small text files.
    * [ ] Measure time to index.
    * [ ] Restart daemon.
    * [ ] Measure time to "ready" (should be near instant).
    * [ ] Delete 500 files.
    * [ ] Verify DB size decreases.

---

## 🔮 Phase 7: The Universal Reader [PENDING]

* [ ] **Dependency Integration**: `pdf-extract`, `dotext`.
* [ ] **Refactor Extractor**: Route by extension.
* [ ] **Snippet Generation**: `_CONTEXT.md` generation logic.

## 🔮 Phase 8: Aggregation [PENDING]

* [ ] **Config**: `~/.config/magicfs/sources.json`.
* [ ] **Virtual Dirs**: `/sources` and `/saved` endpoints.
