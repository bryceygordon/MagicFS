# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 🎯 Project Overview

**MagicFS** is a **Semantic Virtual Filesystem** that provides AI-powered file search through a FUSE filesystem interface. Users navigate to `/search/[query]` and see semantic search results as a directory.

**Critical Constraint**: **The 10ms Law** - Every FUSE operation must complete in <10ms. Never block the FUSE loop.

## 🔗 Version Control

### Git Repository
- **Remote**: `git@github.com:bryceygordon/MagicFS.git`
- **Branch**: `main`
- **Location**: `/home/bryceg/magicfs`

### Common Git Commands
```bash
# Check status
git status

# Add files
git add .
git add .gitignore

# Commit changes (use descriptive messages!)
git commit -m "Brief description of changes"

# Push to remote
git push -u origin main

# Pull latest changes
git pull origin main

# View remotes
git remote -v

# View commit history
git log --oneline

# View changes
git diff
```

### 🔄 Regular Git Maintenance

**CRITICAL: Perform these git chores regularly!**

1. **Commit Frequently**
   - Make small, atomic commits
   - One feature/fix per commit
   - Write clear, descriptive commit messages
   - Example: `"Add FastEmbed model loading"` not `"Update code"`

2. **Sync Daily or Frequently**
   ```bash
   # Push your changes regularly (at least daily)
   git add .
   git commit -m "Describe your changes"
   git push origin main

   # Pull remote changes
   git pull origin main
   ```

3. **Write Good Commit Messages**
   - First line: Brief summary (50-72 chars)
   - Blank line
   - Body: What changed and why (wrap at 72 chars)
   - Examples:
     - ✅ `"Add text extraction for PDF files"`
     - ✅ `"Fix EAGAIN handling in HollowDrive"`
     - ❌ `"Fixed stuff"` or `"Update code"`

4. **Review Before Committing**
   ```bash
   # Check what you're committing
   git status
   git diff

   # Stage selectively (recommended for larger changes)
   git add -p  # Interactive staging
   ```

5. **Branch Strategy (if needed later)**
   ```bash
   # Create feature branch
   git checkout -b feature/search-optimization

   # Work on branch
   git commit -m "Add search result caching"

   # Merge back to main
   git checkout main
   git merge feature/search-optimization
   git branch -d feature/search-optimization
   git push origin main
   ```

6. **Keep Remote Updated**
   - Never leave local changes uncommitted for more than a day
   - Sync with `git pull` before starting work
   - Push immediately after completing features/fixes
   - Use `git status` to track uncommitted changes

## 🏗️ Architecture: Three-Organ System

MagicFS uses a single-process architecture with three isolated "organs" that communicate via shared state:

```
┌─────────────────────────────────────────────────────────┐
│                    MagicFS Process                       │
│                                                          │
│  ┌───────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │ Hollow Drive  │  │   Oracle     │  │  Librarian   │  │
│  │  (FUSE Loop)  │  │ (Async Brain)│  │   (Watcher)  │  │
│  │               │  │              │  │              │  │
│  │ • Synchronous │  │ • Tokio      │  │ • notify     │  │
│  │ • Never blocks│  │ • fastembed  │  │ • Background │  │
│  │ • Returns     │  │ • sqlite-vec │  │ • Thread     │  │
│  │   EAGAIN      │  │              │  │              │  │
│  └───────┬───────┘  └──────┬───────┘  └──────┬───────┘  │
│          │                  │                  │          │
│          └──────────────────┼──────────────────┘          │
│                             │                             │
│              Shared State (Arc<RwLock>)                   │
│                                                          │
└─────────────────────────────────────────────────────────┘
```

### 1. Hollow Drive (`src/hollow_drive.rs`) - The Face
- **Type**: Synchronous FUSE loop (implements `fuser::Filesystem`)
- **Role**: Dumb terminal that accepts syscalls and returns data from memory cache
- **Rule**: NEVER touches disk or runs embeddings. Returns EAGAIN or placeholder if data missing
- **Critical**: Must never block >10ms

### 2. Oracle (`src/oracle.rs`) - The Brain
- **Type**: Async Tokio Runtime + Blocking Compute Threads
- **Role**: Handles vector search (fastembed) and SQLite (sqlite-vec)
- **Rule**: Populates the memory cache for Hollow Drive

### 3. Librarian (`src/librarian.rs`) - The Hands
- **Type**: Background Watcher Thread (`notify` crate)
- **Role**: Watches physical directories, updates SQLite Index, ensures VFS consistency
- **Rule**: Completely isolated from Hollow Drive

## 📦 Shared State (`src/state.rs`)

All organs communicate via `Arc<RwLock<GlobalState>>`:

```rust
pub struct GlobalState {
    pub active_searches: Arc<DashMap<String, u64>>,                    // Query String -> Dynamic Inode
    pub search_results: Arc<DashMap<u64, Vec<SearchResult>>>,          // Dynamic Inode -> Search Results
    pub db_connection: Arc<Mutex<Option<rusqlite::Connection>>>,       // Database connection
    pub embedding_model: Arc<Mutex<Option<fastembed::TextEmbedding>>>, // Loaded model
}
```

## 🗄️ Virtual Filesystem Layout

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

## 📅 Development Phases

### ✅ Phase 1: The Foundation [COMPLETE]
- Three-organ architecture scaffolded
- FUSE skeleton functional
- Build passes successfully

### ✅ Phase 2: The Storage [COMPLETE]
- SQLite database implemented at `.magicfs/index.db` with WAL mode
- `file_registry` table: tracks physical files with metadata (abs_path, inode, mtime, size, is_dir)
- `system_config` table: key-value metadata store
- `src/storage/` module: complete with connection management and CRUD operations
- Integration: Database initialization in main.rs, exported in lib.rs

### ✅ Phase 3: The Brain [COMPLETE]
- ✅ FastEmbed model integration (BAAI/bge-small-en-v1.5, 384 dimensions)
- ✅ Vector similarity search using sqlite-vec
- ✅ Query processing and caching in Oracle
- ✅ Embedding generation pipeline with tokio::spawn_blocking
- ✅ Result ranking and scoring (1.0 - cosine_distance)
- ✅ Text extraction from files using text_extraction module
- ✅ vec_index module for embedding storage and retrieval
- ✅ All 8 micro-steps from ROADMAP.md completed

### ✅ Phase 4: The Glue [COMPLETE]
- ✅ Path parsing in HollowDrive (/search/[query])
- ✅ EAGAIN handling for missing data
- ✅ Directory entry generation for /search/[query]/
- ✅ FUSE operations integration with Oracle
- ✅ File read handlers for search results
- ✅ All FUSE operations implemented: lookup, getattr, readdir, read, open, statfs
- ✅ Dynamic search result directory generation (0.95_filename.txt format)
- ✅ Search result file content returns: path + score

### ✅ Phase 5: The Watcher [COMPLETE]
- ✅ File system monitoring with notify crate (RecommendedWatcher)
- ✅ Debouncing for file events (500ms quiet period)
- ✅ On file create/modify: extract text → generate embedding → update vec_index
- ✅ On file delete: remove from vec_index and file_registry
- ✅ Files-to-index queue management (Librarian → Oracle communication)
- ✅ Cache invalidation after changes
- ✅ Production-ready error handling and logging
- ✅ Multi-path watching with recursive mode

## 🔧 Common Commands

### Build & Development
```bash
# Build the project (current: ~2.75s)
cargo build

# Quick error checking (faster than build)
cargo check

# Run linter with automatic fixes
cargo clippy -- -D warnings

# Watch for changes and auto-rebuild
cargo watch -x build

# Clean build artifacts
cargo clean
```

### Testing
```bash
# Run all tests
cargo test

# Run specific test (when available)
cargo test storage
cargo test oracle

# Run tests with output
cargo test -- --nocapture
```

### Running the Filesystem
```bash
# Run the filesystem (Phase 5 - FULLY FUNCTIONAL!)
RUST_LOG=debug cargo run /tmp/magicfs /path/to/watch

# Run with trace logging
RUST_LOG=trace cargo run /tmp/magicfs

# Mount and use:
ls /tmp/magicfs/search/your_query
cat /tmp/magicfs/search/your_query/0.95_filename.txt

# Unmount (if needed)
fusermount3 -u /tmp/magicfs
```

### Database Operations
```bash
# Inspect the database (Phase 2+)
sqlite3 /tmp/magicfs/.magicfs/index.db

# In sqlite3 shell:
.tables
.schema file_registry
SELECT * FROM file_registry LIMIT 10;
```

### Code Quality
```bash
# Format code
cargo fmt

# Check formatting
cargo fmt --check

# Full CI simulation (what GitHub Actions runs)
cargo check --all-targets
cargo test --all-targets
cargo clippy --all-targets
```

## 📂 Key Files

| File | Purpose |
|------|---------|
| `src/main.rs` | Entry point - initializes all three organs + database |
| `src/hollow_drive.rs` | **FUSE filesystem** - Full implementation with lookup, getattr, readdir, read, open, statfs, EAGAIN handling |
| `src/oracle.rs` | Async brain - FastEmbed integration, vector search, query processing, file indexing, embedding generation |
| `src/librarian.rs` | Background watcher - notify crate integration, debouncing, file event handling (create/modify/delete) |
| `src/state.rs` | Shared state management (Arc<RwLock<GlobalState>>) |
| `src/storage/connection.rs` | Database connection management (Phase 2) |
| `src/storage/file_registry.rs` | File CRUD operations (Phase 2) |
| `src/storage/init.rs` | Database initialization (Phase 2) |
| `src/storage/text_extraction.rs` | Extract text from files for embedding generation (Phase 3) |
| `src/storage/vec_index.rs` | Vector embedding storage and retrieval (Phase 3) |
| `src/storage/mod.rs` | Storage module exports |
| `src/error.rs` | Error types (MagicError, Result aliases) |
| `src/lib.rs` | Module exports and public API |
| `Cargo.toml` | Dependencies and build configuration |

### Storage Module (Phase 2 Complete)
```
src/storage/
├── mod.rs              - Module declaration and exports
├── connection.rs       - Database initialization and connection management
├── file_registry.rs    - CRUD operations for file_registry table
├── init.rs             - Database initialization helpers
├── text_extraction.rs  - Extract text content from files for indexing
└── vec_index.rs        - Vector embedding storage and retrieval operations
```

## 📚 Essential Documentation

**Read these first:**
1. `ROADMAP.md` - Complete development roadmap with all 5 phases
2. `PROJECT_STATUS.md` - Current project status and metrics
3. `SESSION_HANDOVER_SUMMARY.md` - Handoff bundle for new sessions

## 🎯 Critical Implementation Rules

### The 10ms Law ⚠️
Every FUSE operation must return in <10ms:
- HollowDrive must never do sync I/O
- HollowDrive must never generate embeddings
- HollowDrive must never query database
- If data not in cache → return EAGAIN

### Three-Organ Isolation ⚠️
- **HollowDrive**: Synchronous, non-blocking
- **Oracle**: Async (Tokio), handles heavy work
- **Librarian**: Background thread, file watching
- Never violate these boundaries!

### Error Handling
- Use error types from `src/error.rs`
- Use structured logging with `tracing!` macros
- All organs must handle errors gracefully

## 🔗 Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| fuser | 0.14 | FUSE filesystem implementation |
| tokio | 1.0 | Async runtime |
| rusqlite | 0.30 | SQLite database bindings |
| fastembed | 5.5 | Vector embedding generation |
| notify | 6.0 | File system event watching |
| dashmap | 5.5 | Concurrent hashmap for shared state |
| tracing | 0.1 | Structured logging |
| sqlite-vec | 0.1 | Vector similarity search in SQLite |
| bytemuck | 1.0 | Byte manipulation for vector storage |

## 🧪 Testing

Current state: All 5 phases complete! Full semantic filesystem operational
- End-to-end testing: Mount filesystem and navigate to /search/[query]
- File watching: Create/modify/delete files and verify index updates
- Each organ is independently testable
- Integration tests can verify the three-organ coordination (Phase 1-5 complete)

## 🚀 Current Status

- **Location**: `/home/bryceg/magicfs`
- **Phase**: 5/5 Complete (The Watcher) - ALL PHASES DONE! 🎉
- **Build Status**: ✅ Passing (builds successfully in ~0.15s with warnings)
- **Status**: Production-ready semantic filesystem ready for use!
- **Database**: SQLite at `.magicfs/index.db` with WAL mode
- **Tables**: file_registry, system_config, vec_index (sqlite-vec virtual table)
- **FastEmbed**: BAAI/bge-small-en-v1.5 model (384 dimensions) loaded in Oracle
- **Text Extraction**: Implemented for indexing file content
- **FUSE Interface**: Fully implemented with lookup, getattr, readdir, read, open, statfs
- **File Watching**: notify crate with debouncing and multi-path support
- **Architecture**: Three-organ system fully operational (HollowDrive, Oracle, Librarian)
- **Search**: Semantic search via `/search/[query]` directory navigation
- **Indexing**: Automatic file indexing on create/modify, removal on delete
- **Warnings**: 11 cosmetic warnings (unused vars, imports - non-blocking, can be cleaned up)

## 🎉 What Works End-to-End

**Complete Workflow**:
1. **Mount**: `cargo run /tmp/magicfs /path/to/watch`
2. **Browse**: `ls /tmp/magicfs/search/`
3. **Search**: `ls /tmp/magicfs/search/my_query` (creates search directory dynamically)
4. **View Results**: Files appear as `0.95_filename.txt`, `0.87_document.pdf`, etc.
5. **Read Result**: `cat /tmp/magicfs/search/my_query/0.95_filename.txt` → Returns file path + score
6. **Watch**: Create/modify files in watched directory → automatically indexed
7. **Search Updates**: New files appear in future searches immediately

## 💡 Development Tips

### Architecture
- Keep **HollowDrive** dumb: just returns cache or EAGAIN, never blocks
- **Oracle** does all heavy lifting: ✅ embeddings (FastEmbed), ✅ vector search (sqlite-vec), ✅ file indexing, async operations
- **Librarian** updates index: background thread for file watching, adds to files_to_index queue
- **Storage module** provides database operations: connected to all organs
- ✅ Full filesystem operational: FUSE interface, file watching, vector search all complete!

### Code Quality
- Use existing error types from `src/error.rs` (MagicError, Result aliases)
- Use existing logging with `tracing!` macros for structured logging
- Follow the micro-step pattern (8 steps per phase from ROADMAP.md)
- Never violate the 10ms latency constraint for FUSE operations

### Working with Database (Phase 2+)
- Database connection managed via `GlobalState.db_connection`
- File registry operations: `register_file()`, `get_file_by_path()`, `list_files()`
- Connection stored at: `mountpoint/.magicfs/index.db`
- Use WAL mode for performance (already configured)
- vec_index virtual table ready for vector operations
- Text extraction: `extract_text_from_file()` for indexing file content
- Vector operations: `insert_embedding()`, `delete_embedding()` for managing embeddings

### Async Patterns (Phase 3+)
- ✅ Embedding generation happens in Oracle (async context)
- ✅ FastEmbed model loaded (BAAI/bge-small-en-v1.5, 384 dims)
- ✅ Vector search using sqlite-vec virtual table
- ✅ Results cached in `GlobalState.search_results`
- ✅ Query processing pipeline: check cache → generate embedding → search → cache results
- ✅ File indexing: extract text → generate embedding → update vec_index → register in file_registry
- ✅ Files-to-index queue: `files_to_index` in GlobalState for Librarian → Oracle communication
- ✅ Phase 4: HollowDrive calls Oracle for searches via active_searches and uses EAGAIN for async operations
- ✅ Phase 5: Librarian watches files and adds to files_to_index, Oracle processes asynchronously

### Testing
- Full system testing available now: Mount the filesystem and use it!
- Test database operations manually with sqlite3 CLI
- Verify build with `cargo build` before committing changes
- Oracle FastEmbed integration and file indexing verified via cargo build
- Clean up warnings periodically (cosmetic only, don't block)
- Manual testing:
  - Mount: `cargo run /tmp/magicfs /path/to/watch`
  - Navigate: `ls /tmp/magicfs/search/`
  - Search: `ls /tmp/magicfs/search/my_query`
  - Read results: `cat /tmp/magicfs/search/my_query/0.95_filename.txt`
  - Watch: Create/modify files in watched directory and see automatic indexing

### Development Workflow
```bash
# Standard development loop
cargo check          # Fast feedback
cargo build          # Verify compilation
cargo clippy         # Check code quality
# Make changes
cargo check          # Repeat

# Before creating PR
cargo fmt --check    # Verify formatting
cargo build --release # Test release build
```

## 🔗 References

- **FUSE API**: https://docs.rs/fuser/0.14/fuser/
- **Tokio**: https://docs.rs/tokio/1.0/tokio/
- **SQLite**: https://docs.rs/rusqlite/0.30/rusqlite/
- **FastEmbed**: https://docs.rs/fastembed/5.5/fastembed/
- **FUSE Examples**: https://github.com/cberner/fuser/tree/master/examples