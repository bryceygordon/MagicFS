# MagicFS Development Roadmap

## 🎯 System Overview

**MagicFS** is a **Semantic Virtual Filesystem** that provides AI-powered file search through a FUSE filesystem interface. Users navigate to `/search/[query]` and see semantic search results as a directory.

**System Constraint**: Never block the FUSE loop for >10ms (The 10ms Law)

## 📋 Architecture: Three-Organ System

### 1. **Hollow Drive** (The Face)
- **Type**: Synchronous FUSE loop (implements `fuser::Filesystem`)
- **Role**: Dumb terminal that accepts syscalls and returns data from memory cache
- **Rule**: NEVER touches disk or runs embeddings. Returns EAGAIN or placeholder if data missing
- **Critical**: Must never block >10ms

### 2. **Oracle** (The Brain)
- **Type**: Async Tokio Runtime + Blocking Compute Threads
- **Role**: Handles vector search (fastembed) and SQLite (sqlite-vec)
- **Rule**: Populates the memory cache for Hollow Drive
- **Location**: `src/oracle.rs`

### 3. **Librarian** (The Hands)
- **Type**: Background Watcher Thread (`notify` crate)
- **Role**: Watches physical directories, updates SQLite Index, ensures VFS consistency
- **Rule**: Completely isolated from Hollow Drive
- **Location**: `src/librarian.rs`

---

## 🗄️ Data Model (The Source of Truth)

### Database: SQLite (WAL mode)
- **Location**: `.magicfs/index.db`
- **Tables**:
  - `file_registry`: Maps physical `abs_path` to `file_id` (Inode)
  - `vec_index`: Virtual table using `sqlite-vec` for 384-float embeddings
  - `system_config`: Key/Value store

### Shared State: `Arc<RwLock<GlobalState>>`
- `active_searches`: `DashMap<String, u64>` - Query String -> Dynamic Inode
- `search_results`: `DashMap<u64, Vec<SearchResult>>` - Dynamic Inode -> Search Results
- `db_connection`: `Arc<Mutex<Option<rusqlite::Connection>>>` - Database connection
- `embedding_model`: `Arc<Mutex<Option<fastembed::TextEmbedding>>>` - Loaded model

---

## 🗂️ Virtual Layout

```
/ (Root, Inode 1)
├── .magic/ (Config, Inode 2)
│   └── config.db (SQLite database)
└── search/ (The Portal, Inode 3)
    └── [Query String]/ (Dynamic Inodes)
        ├── 1.00_exact_match.txt
        ├── 0.95_file1.txt
        ├── 0.87_file2.txt
        └── ...
```

---

## 📅 Development Phases

### ✅ Phase 1: The Foundation [COMPLETE]
**Status**: ✅ COMPLETE - All 8 micro-steps executed successfully
**Goal**: Basic scaffolding and thread harness

**What Was Built**:
- ✅ Cargo.toml with all dependencies (fuser, tokio, rusqlite, fastembed, notify)
- ✅ Project structure with modular organization (lib.rs, hollow_drive.rs, oracle.rs, librarian.rs, state.rs, error.rs)
- ✅ HollowDrive FUSE skeleton (returns EAGAIN, never blocks)
- ✅ Oracle async runtime with task spawning
- ✅ Librarian background thread skeleton
- ✅ Shared state management (Arc<RwLock<GlobalState>>)
- ✅ Error handling and logging infrastructure
- ✅ Main entry point initializing all three organs

**Build Status**: ✅ Compiles successfully with `cargo build`

---

### 🔄 Phase 2: The Storage [NEXT]
**Status**: 🔄 NEXT UP - Ready to begin
**Goal**: SQLite schema, file registry, and vec_index

**Required Micro-Steps**:
1. Initialize SQLite database with WAL mode
2. Create file_registry table (abs_path, file_id, inode, mtime, size)
3. Create vec_index virtual table with sqlite-vec
4. Create system_config table (key, value)
5. Implement database connection management (lazy initialization)
6. Implement basic CRUD operations for file_registry
7. Add sample data insertion for testing
8. Verify database operations work correctly

**Key Files to Modify**:
- `src/state.rs` - Add database initialization methods
- New: `src/storage/` module with database operations
- `src/main.rs` - Add database initialization step

---

### 🔄 Phase 3: The Brain [FUTURE]
**Status**: 🔄 PENDING - After Phase 2
**Goal**: FastEmbed integration and vector search

**Required Micro-Steps**:
1. Initialize fastembed::TextEmbedding model (e.g., BAAI/bge-small-en-v1.5)
2. Implement embedding generation pipeline
3. Batch embedding processing for efficiency
4. Query processing in Oracle (hash query -> check cache -> if missing, process)
5. Vector similarity search using sqlite-vec
6. Result ranking and scoring
7. Update search_results cache in GlobalState
8. Integration test with actual embeddings

**Key Files to Modify**:
- `src/oracle.rs` - Add embedding model loading and querying
- `src/lib.rs` - Export new storage module
- `src/state.rs` - Add embedding model field

---

### 🔄 Phase 4: The Glue [FUTURE]
**Status**: 🔄 PENDING - After Phase 3
**Goal**: Path parsing, state management, result synthesis

**Required Micro-Steps**:
1. Implement virtual path parsing in HollowDrive
2. Integrate HollowDrive -> Oracle communication
3. Implement EAGAIN handling (when search not ready yet)
4. Directory entry generation for /search/[query]/
5. File name generation (score_filename.txt)
6. Lookup handlers for search result files
7. Read handlers to return file contents (paths + scores)
8. Performance optimization (caching, prefetching)

**Key Files to Modify**:
- `src/hollow_drive.rs` - Full FUSE implementation with cache integration
- `src/oracle.rs` - Enhanced with embedding + search integration
- `src/state.rs` - Result synthesis logic

---

### 🔄 Phase 5: The Watcher [FUTURE]
**Status**: 🔄 PENDING - After Phase 4
**Goal**: File system monitoring and index consistency

**Required Micro-Steps**:
1. Integrate notify crate with actual file watching
2. Debouncing for file events (avoid too frequent updates)
3. On file create/modify: extract text, generate embedding, update vec_index
4. On file delete: remove from vec_index and file_registry
5. Batch database updates for efficiency
6. Cache invalidation strategies
7. Handle edge cases (moved files, permission errors)
8. Production-ready error handling and logging

**Key Files to Modify**:
- `src/librarian.rs` - Full notify integration
- `src/storage/` - File registration and deletion logic
- New: text extraction module (for file content)

---

## 🎯 Critical Success Criteria

1. **The 10ms Law**: Every FUSE operation must return in <10ms
   - Verified with: `strace` on FUSE operations
   - Test: `time ls /search/query/`

2. **Three-Organ Isolation**: Each organ must be independently testable
   - HollowDrive never blocks on disk or embeddings
   - Oracle runs async and doesn't block FUSE
   - Librarian runs on separate thread

3. **Semantic Search Quality**:
   - Similar files should score >0.8
   - Query time should be <100ms for common searches
   - Results should be consistent across sessions

4. **Production Readiness**:
   - Clean shutdown (no zombie threads)
   - Graceful handling of unmount
   - Error recovery and logging

---

## 📦 Build & Test Commands

```bash
# Build the project
cargo build

# Run with logging
RUST_LOG=debug cargo run /tmp/magicfs

# Test Phase 1 (no errors expected)
cargo check

# Next phases will add:
# cargo test storage  # For Phase 2
# cargo test oracle   # For Phase 3
# cargo test glue     # For Phase 4
# cargo test watcher  # For Phase 5
```

---

## 🔍 Current State Summary

**Working Directory**: `/home/bryceg/magicfs`
**Git Status**: Not a git repo (yet)
**Last Successful Build**: ✅ cargo build completes
**Current Phase**: Phase 1 Complete, ready for Phase 2

**Files Created**:
- `Cargo.toml` - Dependencies and build config
- `src/lib.rs` - Module exports
- `src/main.rs` - Entry point
- `src/hollow_drive.rs` - FUSE skeleton (128 lines)
- `src/oracle.rs` - Async brain (149 lines)
- `src/librarian.rs` - Watcher thread (103 lines)
- `src/state.rs` - Shared state (56 lines)
- `src/error.rs` - Error types (13 lines)

**Dependencies Installed**:
- fuser 0.14 (FUSE filesystem)
- tokio 1.0 (async runtime)
- rusqlite 0.30 (SQLite bindings)
- fastembed 5.5 (vector embeddings)
- notify 6.0 (file watching)
- dashmap 5.5 (concurrent hashmap)
- tracing (structured logging)

**Known Warnings** (non-blocking):
- Unused variables (clean up later)
- Unused imports (ReplyEmpty)
- Minor linting items

---

## 📚 References

- **FUSE API**: https://docs.rs/fuser/0.14/fuser/
- **FastEmbed**: https://docs.rs/fastembed/5.5/fastembed/
- **SQLite-vec**: Vector similarity search in SQLite
- **Rust FUSE Examples**: https://github.com/cberner/fuser/tree/master/examples

---

**END OF ROADMAP**