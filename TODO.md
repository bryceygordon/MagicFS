# MagicFS Roadmap

## 🚀 Priority: Phase 10 - AI & Content Intelligence
- [ ] **AI Model Upgrade**
    - [ ] Evaluate `BGE-M3` vs `Nomic-Embed` for better semantic resolution.
    - [ ] Update `oracle.rs` to load the larger model.
- [ ] **Context Injection**
    - [ ] Modify `indexer.rs` to prepend `Filename: [Name]\n` to every text chunk.
    - [ ] Fix "Semantic Dilution" where generic chunks lose context.
- [ ] **Search Scoring Improvements**
    - [ ] Implement Hybrid Scoring (Vector + Keyword).
    - [ ] Add SQL boost for filenames matching the query (`abs_path LIKE '%query%'`).
- [ ] **Binary File Support**
    - [ ] Integrate `pdf-extract` for PDFs.
    - [ ] Integrate `docx-rs` for Word docs.

## Archive (Completed)
- [x] **Phase 7: Passthrough Read/Write**
    - [x] Implement `read()` with `FileExt::read_at`.
    - [x] Implement `write()` and `setattr` for saving changes.
- [x] **Phase 8: Multi-Directory**
    - [x] Update CLI to accept list of paths.
    - [x] Update Librarian to watch multiple roots.
- [x] **Phase 9: Mirror Mode**
    - [x] Implement `/mirror` directory.
    - [x] Implement Inode mapping for browsing.
