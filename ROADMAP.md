# MagicFS Development Roadmap

## 🎯 System Philosophy: Fail First
We assume the filesystem is hostile. Files will be huge, permissions will be denied, encoding will be broken, and tools will be impatient. The system must fail safely (by skipping, logging, or deferring) rather than crashing or hanging.

---

## 🏗️ Architecture: Three-Organ System (Stable)

1. **Hollow Drive** (The Face): Sync FUSE loop. Enforces **The 10ms Law**.
2. **Oracle** (The Brain): Async. Handles Embeddings & SQLite.
3. **Librarian** (The Hands): Background watcher. Feeds the Oracle.

---

## 📜 Era 1: The Foundation (Completed)

### ✅ Phase 1: The Scaffold
- Basic Thread Harness & Shared State.
- FUSE Skeleton (EAGAIN handling).

### ✅ Phase 2: The Storage
- SQLite (WAL mode) & Schema.
- `file_registry` & `system_config` tables.

### ✅ Phase 3: The Brain
- FastEmbed Integration (BAAI/bge-small-en-v1.5).
- `sqlite-vec` Virtual Table integration.
- Async embedding pipeline.

### ✅ Phase 4: The Glue
- `/search/[query]` Virtual Directory Logic.
- Dynamic Inode generation.
- FUSE `read` handlers returning semantic content.

### ✅ Phase 5: The Watcher
- `notify` crate integration.
- Recursive directory scanning.
- Debounced event handling.
- Ignore rule processing (`.magicfsignore`).

---

## 🛡️ Era 2: Refinement & Hardening (Current)

### 🔄 Phase 6: Hardening & Resilience [ACTIVE]
**Goal:** Solve "Semantic Dilution" and prevent "The Slurp" (OOM crashes).

**Micro-Steps:**
1. [x] **Safety Guards**: Implement file size limits (10MB) and binary detection (null byte check).
2. [ ] **Chunking Architecture**: Refactor `text_extraction` and `vec_index` to support 1-to-Many relationship (1 File = N Chunks).
3. [ ] **Sliding Window Logic**: Implement text chunking (e.g., 512 tokens with 25% overlap).
4. [ ] **Search Aggregation**: Update SQL query to aggregate chunk scores into a single file score (e.g., Max Chunk Score strategy).
5. [ ] **Memory Guardrails**: Add strict limits to the `Oracle`'s embedding channel to prevent queue explosions.

### 🔮 Phase 7: Compatibility & Polish [FUTURE]
**Goal:** Make MagicFS behave nicely with standard Unix tools.

**Micro-Steps:**
1. [ ] **LRU Caching**: Evict old search results from RAM to keep footprint low.
2. [ ] **Extended File Support**: Add PDF/DOCX support (via `pdf-extract` or similar).
3. [ ] **Blocking Mode**: Optional CLI flag to make `lookup` block instead of EAGAIN (for dumb script compatibility).
4. [ ] **Daemon Mode**: Run as a background service.

---

## 📏 Critical Constraints

1. **The 10ms Law**: FUSE ops must never block >10ms.
2. **Memory Cap**: The system should never exceed ~500MB RAM for a medium repo.
3. **Graceful Degradation**: If a file can't be read/embedded, the system continues; the file is simply omitted from search.
