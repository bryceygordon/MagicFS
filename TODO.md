FILE: TODO.md

* [x] **Charter**: Establish `CHARTER.md` to guide architectural decisions.
* [x] **Roadmap**: Update `ROADMAP.md` with Phase 6 (Refactor) and Phase 7 (Hardening).
* [ ] **Phase 6.1: The Repository**:
* [ ] Create `src/storage/repository.rs`.
* [ ] Move `init_connection` and SQL schemas here.
* [ ] Move `file_registry` CRUD ops here.
* [ ] Move `search` SQL logic here.
* [ ] *Check:* Run `tests/run_suite.sh`.


* [ ] **Phase 6.2: The InodeStore**:
* [ ] Create `src/core/inode_store.rs`.
* [ ] Move `active_searches` and `search_results` from `state.rs`.
* [ ] Update `HollowDrive` to read from `InodeStore`.
* [ ] Update `Oracle` to write to `InodeStore`.
* [ ] *Check:* Run `tests/run_suite.sh`.


* [ ] **Phase 6.3: Engine Extraction**:
* [ ] Refactor `Oracle` struct to use `Repository` instead of raw connection.
* [ ] *Check:* Run `tests/run_suite.sh`.
