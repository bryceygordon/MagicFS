# MagicFS Task List

## 🛡️ Phase 6.9: The Safety Systems (Infrastructure Hardening) [COMPLETE]
**Goal:** Prevent self-destruction and infinite loops before adding new file formats.

* [x] **The Anti-Feedback Switch:** Panic if mounting inside watch dir.
* [x] **Memory Leak Fix:** Replaced `DashMap` with `LruCache` (Cap: 1000).
* [x] **Startup Scalability:** Implemented Streaming Iterator for `Repository`.
* [x] **Manual Override (The Kick Button):**
    * [x] **Trigger:** `touch .magic/refresh` in Watch Root.
    * [x] **Logic:** Rescans directory and updates if `mtime` OR `size` differs.

---

## 🚀 Phase 7: The Universal Reader [NEXT UP]
**Objective:** Break the format barrier. Support PDF, DOCX, and other rich media.

* [ ] **Dependencies**: Add `pdf-extract`.
* [ ] **Extractor Refactor**: Route file types to specific parsers in `src/storage/text_extraction.rs`.
* [ ] **Error Isolation**: Wrap external parsers in `catch_unwind` or separate threads.
