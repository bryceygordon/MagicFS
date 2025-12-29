* [x] **Charter**: Establish `CHARTER.md` to guide architectural decisions.
* [x] **Roadmap**: Update `ROADMAP.md` with Phase 6 (Refactor) and Phase 7 (Hardening).
* [x] **Phase 6.1: The Repository**:
    * [x] Create `src/storage/repository.rs`.
    * [x] Move `init_connection` and SQL schemas here.
    * [x] Move `file_registry` CRUD ops here.
    * [x] Move `search` SQL logic here.
    * [x] *Check:* Run `tests/run_suite.sh`.

* [x] **Phase 6.2: The InodeStore**:
    * [x] Create `src/core/inode_store.rs`.
    * [x] Move `active_searches` and `search_results` from `state.rs`.
    * [x] Update `HollowDrive` to read from `InodeStore`.
    * [x] Update `Oracle` to write to `InodeStore`.
    * [x] *Check:* Run `tests/run_suite.sh`.

* [x] **Phase 6.3: Engine Extraction**:
    * [x] Create `src/engine/` module.
    * [x] Extract `Indexer` (File I/O -> Chunking -> DB).
    * [x] Extract `Searcher` (Embedding -> DB -> InodeStore).
    * [x] Refactor `Oracle` to be a lightweight Orchestrator.
    * [x] *Check:* Run `tests/run_suite.sh`.



## Phase 7: The Universal Reader [ACTIVE]

**Objective:** Enable MagicFS to read PDF/DOCX files and expose "Why it matched" snippets.

* [ ] **7.1: Dependency Integration**
    * [ ] Add `pdf-extract` (or `lopdf`) to `Cargo.toml`.
    * [ ] Add `dotext` (or `zip` + `xml-rs`) to `Cargo.toml`.
    * [ ] *Check:* `cargo build` passes without massive bloat.

* [ ] **7.2: Refactor Extractor**
    * [ ] Modify `src/storage/text_extraction.rs` to route by extension.
    * [ ] Implement `extract_pdf(path: &Path) -> Result<String>`.
    * [ ] Implement `extract_office(path: &Path) -> Result<String>`.
    * [ ] Ensure "Fail First" checks (Size > 10MB) apply *before* parsing starts.

* [ ] **7.3: Contextual Snippets (`_CONTEXT.md`)**
    * [ ] Update `SearchResult` struct in `state.rs` to include `snippets: Vec<String>`.
    * [ ] Update `Repository::search` SQL query to return the text content of the best matching chunks (not just the file ID).
    * [ ] Update `HollowDrive::lookup` to handle `_CONTEXT.md`.
    * [ ] Update `HollowDrive::read` to generate the Markdown report on the fly.

* [ ] **7.4: Testing**
    * [ ] Create `tests/cases/test_06_rich_media.py`.
    * [ ] Mock a PDF file (or check in a tiny test PDF).
    * [ ] Verify the search finds text inside the binary format.
    * [ ] Verify `_CONTEXT.md` exists and contains the expected snippets.

## Phase 8: Aggregation & Persistence (Planned)

**Objective:** Support multi-root watching and XDG-compliant state persistence.

* [ ] **8.1: State Management (The "Precious" Data)**
    * [ ] Create `src/config.rs` to handle loading/saving `sources.json` and `views.json`.
    * [ ] Implement XDG Base Directory logic (`~/.config/magicfs` vs `~/.cache/magicfs`).
    * [ ] Ensure `index.db` is moved to the cache directory.

* [ ] **8.2: The Librarian Upgrade**
    * [ ] Refactor `Librarian` to hold a `HashMap<PathBuf, Watcher>` instead of a single watcher.
    * [ ] Implement `add_source(path)` and `remove_source(path)` methods.
    * [ ] Ensure existing index entries are purged when a source is removed.

* [ ] **8.3: The Virtual Interface**
    * [ ] Update `HollowDrive` to expose `/sources` (Virtual Directory).
    * [ ] Implement `symlink` syscall in `HollowDrive` to trigger `Librarian::add_source`.
    * [ ] Implement `unlink` syscall in `HollowDrive` to trigger `Librarian::remove_source`.
    * [ ] Implement `mkdir` in `/saved` to trigger View creation.

* [ ] **8.4: Testing Persistence**
    * [ ] Create `tests/cases/test_07_multiroot.py`.
    * [ ] Verify adding a source via symlink starts indexing immediately.
    * [ ] Verify restarting the daemon restores the sources from config.
