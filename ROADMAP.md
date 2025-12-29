# MagicFS Development Roadmap

## 🎯 System Philosophy: Fail First

We assume the filesystem is hostile. Files will be huge, permissions will be denied, and tools will be impatient. The system must fail safely rather than crashing or hanging.

---

## 🏗️ Architecture: Service-Based (Current)

The system has evolved from a simple prototype to a Service-Oriented Architecture:

1.  **Hollow Drive (`fs/`)**: The "Face". Dumb, synchronous FUSE loop.
2.  **The Orchestrator (`oracle.rs`)**: The "Manager". Routes messages, manages the Actor.
3.  **The Engine (`engine/`)**: The "Worker".
    * **Indexer**: Handles file reading, chunking, and database writes.
    * **Searcher**: Handles embedding generation and database reads.
4.  **The Librarian (`librarian.rs`)**: The "Eyes". Watches for file events.
5.  **The Repository (`storage/`)**: The "Memory". Encapsulates all SQL logic.

---

## 📜 History

### ✅ Era 1: The Foundation (Phases 1-5)
* Established basic FUSE loop, SQLite storage, and FastEmbed integration.
* Implemented "Three-Organ" prototype.

### ✅ Era 2: Architecture 2.0 (Phase 6)
* **Refactor**: Split monolithic `Oracle` into `Engine` modules.
* **Hardening**: Added binary detection, 10MB limits, and sliding window chunking.
* **Consistency**: Introduced `InodeStore` to guarantee valid file handles.

---

## 🛡️ Era 3: Production Readiness [ACTIVE]

### 🔮 Phase 7: Polish & Compatibility

**Goal:** Transform MagicFS from a "functional prototype" to a "usable daily tool".

1.  [ ] **LRU Caching**:
    * `InodeStore` currently grows forever.
    * Implement an eviction policy (remove search results not accessed in X minutes).
2.  [ ] **Daemonization**:
    * Allow running `magicfs mountpoint &` properly.
3.  [ ] **Rich Media Support**:
    * Integrate `pdf-extract` for PDF parsing.
    * Integrate `docx-rs` for Word documents.
4.  [ ] **Installation**:
    * Create a simple `install.sh` script and systemd service file.

---

## 📏 Critical Constraints

1.  **The 10ms Law**: FUSE ops must never block >10ms.
2.  **Memory Cap**: The system should never exceed ~500MB RAM.
