FILE: ROADMAP.md

# MagicFS Development Roadmap

## 🎯 System Philosophy: Fail First

We assume the filesystem is hostile. Files will be huge, permissions will be denied, encoding will be broken, and tools will be impatient. The system must fail safely (by skipping, logging, or deferring) rather than crashing or hanging.

---

## 🏗️ Architecture: Evolution to Service-Based

We are transitioning from the "Three-Organ Prototype" (HollowDrive/Oracle/Librarian) to a "Service-Oriented Architecture" (FS/Engine/Watcher/Storage) to improve maintainability and strictly enforce separation of concerns.

---

## 📜 Era 1: The Foundation (Completed)

### ✅ Phase 1: The Scaffold
### ✅ Phase 2: The Storage
### ✅ Phase 3: The Brain
### ✅ Phase 4: The Glue
### ✅ Phase 5: The Watcher

---

## 🛠️ Era 2: Architecture 2.0 (The Refactor) [ACTIVE]

### 🏗️ Phase 6: Structural Maturity [IN PROGRESS]

**Goal:** Decouple "Data Access", "Business Logic", and "FUSE Interface". Move from monolithic files to domain-specific modules. This prepares the codebase for advanced features like PDF support and caching without creating spaghetti code.

**Micro-Steps:**

1. [x] **The Repository Pattern**
    * Create `src/storage/repository.rs`.
    * Migrate raw SQL from `oracle.rs` (search queries) and `file_registry.rs` into `Repository` methods.
    * *Benefit:* Centralized SQL logic; strict typing for DB interactions.


2. [x] **The Inode Store**
    * Create `src/core/inode_store.rs`.
    * Extract `active_searches` and `search_results` maps from `GlobalState`.
    * Centralize the `hash_to_inode` logic here (removing it from `hollow_drive.rs`).
    * *Benefit:* Guarantees Inode consistency between FUSE and the Engine.


3. [ ] **Engine Decomposition**
    * Split `src/oracle.rs` into:
        * `orchestrator.rs`: Manages the Tokio runtime/Actor channels.
        * `indexer.rs`: Pure logic for reading files, chunking, and embedding.
        * `searcher.rs`: Pure logic for executing vector searches.
    * *Benefit:* Testable business logic isolated from threading complexity.


4. [ ] **Watcher Standardization**
    * Refactor `src/librarian.rs` to `src/watcher/mod.rs`.
    * Ensure strict typing for events entering the indexing queue.


5. [ ] **Directory Restructure**
    * Move files into `src/fs/`, `src/engine/`, `src/storage/`, `src/api/`.

---

## 🛡️ Era 3: Resilience & Features

### 🔒 Phase 7: Hardening & Resilience

**Goal:** Solve "Semantic Dilution" and ensure production-grade stability.

1. [x] **Safety Guards**: Implement file size limits (10MB) and binary detection.
2. [x] **Chunking Architecture**: Refactor schema to support 1-to-Many relationship.
3. [x] **Sliding Window Logic**: Implement text splitting logic.
4. [x] **Basic Aggregation**: SQL query to group chunks by file.
5. [ ] **Refined Aggregation**: Experiment with "Max Score" vs "Avg Score" strategies for better relevance.
6. [ ] **Memory Guardrails**: strict limits on `Oracle`'s embedding channel to prevent queue explosions.

### 🔮 Phase 8: Compatibility & Polish [FUTURE]

1. [ ] **LRU Caching**: Evict old search results from RAM to keep footprint low.
2. [ ] **Rich File Support**: Add PDF/DOCX support (via `pdf-extract` or similar).
3. [ ] **Blocking Mode**: Optional CLI flag to make `lookup` block instead of EAGAIN.
4. [ ] **Daemon Mode**: Run as a background service.

---

## 📏 Critical Constraints

1. **The 10ms Law**: FUSE ops must never block >10ms.
2. **Memory Cap**: The system should never exceed ~500MB RAM for a medium repo.
3. **Graceful Degradation**: If a file can't be read/embedded, the system continues.
