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

**Objective:** Enable MagicFS to read and index PDF and DOCX files.

* [ ] **7.1: Dependency Integration**
    * [ ] Add `pdf-extract` (or `lopdf`) to `Cargo.toml`.
    * [ ] Add `dotext` (or `zip` + `xml-rs`) to `Cargo.toml`.
    * [ ] *Check:* `cargo build` passes without massive bloat.

* [ ] **7.2: Refactor Extractor**
    * [ ] Modify `src/storage/text_extraction.rs` to route by extension.
    * [ ] Implement `extract_pdf(path: &Path) -> Result<String>`.
    * [ ] Implement `extract_office(path: &Path) -> Result<String>`.
    * [ ] Ensure "Fail First" checks (Size > 10MB) apply *before* parsing starts.

* [ ] **7.3: Testing**
    * [ ] Create `tests/cases/test_06_rich_media.py`.
    * [ ] Mock a PDF file (or check in a tiny test PDF).
    * [ ] Verify the search finds text inside the binary format.

## Phase 8: Persistence (On Deck)
* [ ] Implement `mkdir` support in `HollowDrive`.
* [ ] Implement `write` support for `.query` files.
