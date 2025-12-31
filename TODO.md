# MagicFS Task List

---

## 🛡️ Phase 6.9: The Safety Systems (Infrastructure Hardening) [COMPLETE]
**Goal:** Prevent self-destruction and infinite loops before adding new file formats.

* [x] **The Anti-Feedback Switch (Main):**
    * Panic if `watch_dir` starts with `mount_point`.
* [x] **Thermal Overload (Chatter Protection):**
    * Debounce window (2s) with "Final Promise" logic.
* [x] **Manual Override (Forced Sync):**
    * `touch .magic/refresh` triggers full scan.
* [x] **Race Condition Fixes:**
    * Retry on `PermissionDenied` in Indexer.
    * "Lockout/Tagout" queue management.

---

## 🚀 Phase 7: The Universal Reader [ACTIVE]

**Objective:** Break the format barrier. Support PDF, DOCX, and other rich media.

* [ ] **Dependencies**: Add `pdf-extract` (or `pdf-text`) and `docx-rs`.
* [ ] **Extractor Refactor**: Route file types to specific parsers in `src/storage/text_extraction.rs`.
* [ ] **Hardening**: Ensure corrupted PDFs do not crash the Indexer (Fail Small).
* [ ] **Test**: Enable `test_06_rich_media.py`.

## 🔮 Phase 8: Aggregation [PENDING]

* [ ] **Config**: `~/.config/magicfs/sources.json`.
* [ ] **Virtual Dirs**: `/sources` and `/saved` endpoints.
