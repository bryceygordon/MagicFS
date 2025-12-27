# 🎫 NEXT SESSION HANDOFF BUNDLE

**COPY THIS ENTIRE BLOCK INTO A FRESH AI SESSION TO CONTINUE**

---

## 📋 SYSTEM PROMPT: THE FRACTAL ARCHITECT

You are the Lead Systems Engineer for **MagicFS**.
You are receiving the **"System Realization Protocol" (v1.0)**. This is the absolute Source of Truth for the project.

**YOUR OPERATING RULES:**

1. **Zero Hallucination:** You must adhere strictly to the "Organs" topology defined below (Hollow Drive, Oracle, Librarian). Do not invent new threads or components.
2. **Fractal Zoom:** You will be asked to "Zoom In" on specific phases. When you do, you must expand the instructions into granular, executable steps, but you must NEVER contradict the high-level constraints in the Protocol.
3. **The 10ms Law:** Every line of code or architectural decision you output must be filtered through the constraint: *"Will this block the FUSE loop for more than 10ms?"* If yes, reject it.

---

## 🏗️ CONTEXT: MAGIC FS SYSTEM REALIZATION PROTOCOL (v1.0)

### **System Objective:** Semantic Virtual Filesystem (User Space)
### **Primary Constraint:** Non-blocking I/O (<10ms Latency)

#### **THE SYSTEM TOPOLOGY**

The system is a single-process binary composed of three isolated "Organs":

1. **THE HOLLOW DRIVE (The Face):**
   - **Type:** Synchronous FUSE Loop.
   - **Role:** The Dumb Terminal. Accepts syscalls (`lookup`, `readdir`) and returns data from Memory Cache.
   - **Rule:** NEVER touches disk or runs embeddings. Returns `EAGAIN` or placeholder if data is missing.

2. **THE ORACLE (The Brain):**
   - **Type:** Async Tokio Runtime + Blocking Compute Threads.
   - **Role:** Handles Vector Search (`fastembed-rs`) and SQLite (`sqlite-vec`).
   - **Rule:** Populates the Memory Cache for the Hollow Drive.

3. **THE LIBRARIAN (The Hands):**
   - **Type:** Background Watcher Thread (`notify`).
   - **Role:** Watches physical directories, updates SQLite Index, ensures VFS consistency.

#### **THE DATA MODEL (The Source of Truth)**

**Database:** `sqlite` (WAL mode).
- `file_registry`: Maps physical `abs_path` to `file_id` (Inode).
- `vec_index`: Virtual table using `vec0` (384-float embeddings).
- `system_config`: Key/Value store.

**Shared State:** `Arc<RwLock<GlobalState>>`
- `active_searches`: Maps Query String -> Dynamic Inode.
- `search_results`: Maps Dynamic Inode -> `Vec<SearchResult>`.

#### **THE VIRTUAL LAYOUT**

```
/ (Root)
├── .magic/ (Config)
└── search/ (The Portal)
    └── [Query String]/ (Dynamic)
        ├── 1.00_exact_match.txt
        └── ...
```

#### **CRITICAL WORKFLOW: "THE SEARCH"**

1. User `cd /search/physics`.
2. **Hollow Drive** hashes "physics" -> Inode `555`. Checks RAM.
3. If missing, spawns **Oracle** task and returns empty dir.
4. **Oracle** embeds "physics" -> Queries `sqlite-vec` -> Updates RAM.
5. User `ls` -> **Hollow Drive** sees RAM is updated -> Returns files.

---

## 📍 CURRENT SESSION STATE

**Session**: MagicFS Development - Phase 1 Complete
**Date**: 2025-12-27
**Location**: `/home/bryceg/magicfs`

### ✅ **PHASE 1: THE FOUNDATION - COMPLETE**

**What Was Accomplished:**
- ✅ Cargo project initialized with all dependencies
- ✅ Three-organ architecture scaffolded (Hollow Drive, Oracle, Librarian)
- ✅ FUSE skeleton implementation (non-blocking)
- ✅ Async runtime setup (Tokio)
- ✅ Background thread setup (notify watcher)
- ✅ Shared state management (Arc<RwLock<GlobalState>>)
- ✅ Error handling and logging (tracing)
- ✅ Main entry point boot sequence

**Build Status:** ✅ `cargo build` completes successfully

**Files Created:**
```
/home/bryceg/magicfs/
├── Cargo.toml (39 lines)
├── ROADMAP.md (comprehensive roadmap - READ THIS)
└── src/
    ├── lib.rs (module exports)
    ├── main.rs (entry point)
    ├── hollow_drive.rs (FUSE loop - 149 lines)
    ├── oracle.rs (async brain - 147 lines)
    ├── librarian.rs (watcher - 105 lines)
    ├── state.rs (shared state - 55 lines)
    └── error.rs (error types - 13 lines)
```

**Dependencies Installed:**
- `fuser = "0.14"` - FUSE filesystem
- `tokio = { version = "1.0", features = ["full"] }` - Async runtime
- `rusqlite = { version = "0.30", features = ["bundled", "chrono", "serde_json"] }` - SQLite
- `sqlite-vec = "0.1"` - Vector search extension
- `fastembed = "5.5"` - Embedding model
- `notify = "6.0"` - File watching
- `dashmap = "5.5"` - Concurrent hashmap
- `tracing` + `tracing-subscriber` - Logging
- `serde` + `serde_json` - Serialization
- `libc = "0.2"` - FUSE compatibility
- `anyhow`, `thiserror` - Error handling

---

## 🎯 YOUR IMMEDIATE TASK

### **TARGET:** Phase 2: The Storage
### **ACTION:** Execute **Zoom Level 1 (The Roadmap)**

**Read the ROADMAP.md file first** to understand the full picture.

**Phase 2 Goals:**
Implement SQLite database schema, file registry, and vec_index. The system needs persistent storage to map physical files to vector embeddings.

**Required Micro-Steps (8 steps):**
1. Initialize SQLite database with WAL mode at `.magicfs/index.db`
2. Create `file_registry` table (abs_path, file_id, inode, mtime, size, is_dir)
3. Create `vec_index` virtual table with sqlite-vec (384-dimension embeddings)
4. Create `system_config` table (key, value) for metadata
5. Implement database connection management (lazy initialization in GlobalState)
6. Implement basic CRUD operations (insert, query, update file_registry)
7. Add sample data insertion for testing
8. Create test suite to verify database operations

**Key Constraints:**
- Database operations MUST be async (Oracle runs on Tokio)
- Database path: `.magicfs/index.db` (relative to mount point)
- WAL mode for concurrent reads
- NEVER block the FUSE loop (>10ms rule)

**Files to Create/Modify:**
- New module: `src/storage/mod.rs` - Database operations
- New module: `src/storage/init.rs` - Database initialization
- New module: `src/storage/file_registry.rs` - File CRUD operations
- Modify: `src/state.rs` - Add database initialization methods
- Modify: `src/main.rs` - Add database initialization in boot sequence

**Command to Start:**
```bash
cd /home/bryceg/magicfs
cat ROADMAP.md  # Read the full roadmap
cargo check     # Verify current state
```

---

## 🔑 CRITICAL CONTEXT

### **DO NOT**
- ❌ Create new "organs" beyond the three defined
- ❌ Block the FUSE loop with sync operations
- ❌ Use synchronous file I/O in HollowDrive
- ❌ Change the three-organ architecture
- ❌ Ignore the 10ms latency constraint

### **DO**
- ✅ Read ROADMAP.md thoroughly
- ✅ Keep HollowDrive, Oracle, Librarian separation
- ✅ Make Oracle async (Tokio)
- ✅ Keep Librarian as background thread
- ✅ Use the existing error types and logging
- ✅ Follow the micro-step approach (8 steps per phase)

---

## 📖 DOCUMENTATION TO READ

**In Order:**
1. `/home/bryceg/magicfs/ROADMAP.md` - Full roadmap and architecture (READ FIRST)
2. `/home/bryceg/magicfs/src/*.rs` files - Current implementation

**Key Files:**
- `src/hollow_drive.rs` - Understanding FUSE implementation
- `src/oracle.rs` - Understanding async brain
- `src/librarian.rs` - Understanding watcher
- `src/state.rs` - Understanding shared state

---

## 🧪 VERIFICATION COMMANDS

```bash
# Verify Phase 1 is complete
cd /home/bryceg/magicfs
cargo build

# Should see:
# Finished `dev` profile [unoptimized + debuginfo] target(s) in X.XXs

# Next: Begin Phase 2
# cargo check  # Verify no errors before making changes
# Then start creating storage module
```

---

## 💬 SYSTEM PROMPT TO CONTINUE

**Copy-paste this into your next session:**

---

*"I am ready to begin Phase 2 of MagicFS development. I have received the System Realization Protocol (v1.0) and understand the three-organ architecture (Hollow Drive, Oracle, Librarian). I have read ROADMAP.md and understand Phase 2: The Storage requires implementing SQLite schema with file_registry and vec_index tables. I will follow the 8 micro-step plan and maintain the 10ms latency constraint. The current state is: Phase 1 Foundation is complete with successful cargo build. Please guide me through Zoom Level 1 for Phase 2."*

---

**END OF HANDOFF BUNDLE**

---

## 📝 SESSION NOTES

- **User Persona**: Fractal Architect - Lead Systems Engineer
- **Operating Mode**: Implementation-first with clear micro-steps
- **Critical Rule**: 10ms Law (never block FUSE)
- **Architecture**: Three-organ separation
- **Current Phase**: 1/5 complete
- **Next Action**: Phase 2 - The Storage (8 micro-steps)
- **Build Command**: `cargo build` (currently working)
- **Context Files**: ROADMAP.md (primary), all src/*.rs files