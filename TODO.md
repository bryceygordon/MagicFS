# MagicFS Roadmap

## 🚀 Priority: Phase 7 - The Universal Reader & Navigator
- [ ] **Implement Passthrough Reading** (Immediate Priority)
    - [ ] Replace `read()` in `main.rs` to stream real file bytes.
    - [ ] Enable opening/editing files directly from virtual paths.
- [ ] **Multi-Directory Monitoring**
    - [ ] Update `Config` to accept a list of directories (e.g., `vec![PathBuf]`).
    - [ ] Update `Librarian` to watch multiple roots.
- [ ] **"Mirror Mode" (Navigation)**
    - [ ] Create a `/root` (or similar) folder inside MagicFS.
    - [ ] Map watched directories into this folder so they can be browsed normally.
- [ ] **Binary File Support**
    - [ ] Add `pdf-extract` for PDFs.
    - [ ] Add `docx-rs` for Word docs.

## Phase 8: Hardening
- [ ] Robust error handling for file permissions.
- [ ] Integration tests for the full search loop.

## Archive (Completed)
- [x] Phase 6: Arch Linux Packaging
- [x] Systemd Service (Nuked for Dev Mode)
- [x] "Nuclear" Cleanup Script (`dev.sh`)
