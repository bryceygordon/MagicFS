# MagicFS Development Roadmap

## 📜 History
* **Phases 1-5**: Foundation (Basic Indexing, FUSE, Librarian).
* **Phase 6**: Architecture Refactor & Hardening.
    * *Phase 6.5*: Concurrency & Flow Control.
    * *Phase 6.9*: Lockout/Tagout, Race Condition Fixes, and Test Isolation.

---

## 📚 Era 3: Content [ACTIVE]

### 🚀 Phase 7: The Universal Reader [CURRENT FOCUS]
**Goal:** Make the filesystem "Format Agnostic."
* **Concept:** To the user, a PDF is just text. MagicFS must make it scriptable.
* **Tasks:**
    1.  Integrate `pdf-extract` for PDF parsing.
    2.  Integrate `docx-rs` for Word documents.
    3.  Refactor `text_extraction.rs` into a modular "Converter" system.
    4.  **Hardening:** Ensure corrupted PDFs do not crash the Indexer (Fail Small).

### 🔮 Phase 8: Persistence & Aggregation [NEXT UP]
**Goal:** Make MagicFS a permanent tool, not just a session toy.
1.  **Saved Views:** `mkdir .magic/saved/taxes` creates a persistent SQL view.
2.  **Multi-Root:** Watch `~/Documents` AND `/mnt/nas/photos` simultaneously.

---

## 🧠 The "Thin Client" Vision
**Goal:** An Evernote-killer that uses MagicFS as its backend.
* **Design:** The client has no database. It just reads the file system.
* **Manual Sync:** The client will use the `touch .magic/refresh` API to force updates.
