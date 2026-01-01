# MagicFS Development Roadmap

## 📜 History
* **Phases 1-6**: Foundation & Hardening (Done)

---

## 📚 Era 3: Content & Interface [ACTIVE]

### ✅ Phase 7: The Universal Reader & Writer [COMPLETE]
* **Passthrough Read**: `read()` streams bytes from the real disk.
* **Passthrough Write**: `write()` and `setattr()` allow editing files via FUSE.
* **Verification**: Verified with `micro`, `cp`, and integration tests.

### ✅ Phase 8: Multi-Root Monitoring [COMPLETE]
* **Watcher**: Librarian now monitors comma-separated paths (e.g., `~/me,~/vault`).
* **Safety**: Feedback loop detection checks all roots.

### ✅ Phase 9: Navigation (Mirror Mode) [COMPLETE]
* **Mirror Root**: `/mirror` lists all watched roots.
* **Browsing**: Users can navigate folder structures without searching.

### 🧠 Phase 10: The "Magical" AI Upgrade [NEXT UP]
**Goal:** Improve search relevance and handle binary formats.
1.  **Model Upgrade**: Switch to `BGE-M3` or `Nomic` for better nuance ("Beef vs Chicken").
2.  **Context Injection**: Prepend filename to chunks to fix "Title Blindness".
3.  **Title Boosting**: Heuristic boost for filename matches.
4.  **Binary Support**: PDF/DOCX extraction.

---

## 🧠 The "Thin Client" Vision
**Goal:** An Evernote-killer that uses MagicFS as its backend.
* **Status**: Backend API (Filesystem) is now fully functional (Read/Write/Search/Browse).
