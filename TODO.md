FILE: TODO.md

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


* [ ] **Phase 6.3: Engine Extraction**:
    * [ ] Refactor `Oracle` struct to use `Repository` instead of raw connection.
    * [ ] Split `Oracle` into `Orchestrator` and `Searcher`.
    * [ ] *Check:* Run `tests/run_suite.sh`.
