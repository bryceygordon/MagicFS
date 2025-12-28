# Session Handoff: FastEmbed Segmentation Fault Investigation

**Date**: 2025-12-28
**Session Goal**: Fix Model Disappearance Bug (Arc<Mutex<Option<TextEmbedding>>> causing segfaults)

---

## 🎯 The Bug

### Original Problem
- **Location**: `src/oracle.rs` - Model disappears after load due to `.take()` pattern
- **Symptom**: "Model not ready, skipping file indexing" repeating every 100ms
- **Impact**: Oracle can't generate embeddings, semantic search fails

### Current State (After Fix Attempts)
- **Symptom**: **SEGMENTATION FAULT** during file indexing (spawn_blocking context)
- **When**: After model loads successfully, during `model.embed()` call
- **Files**: Multiple test files being indexed concurrently trigger the crash
- **Status**: **UNRESOLVED** - segfault persists through all attempted fixes

---

## 🔬 Root Cause Analysis

### The FastEmbed Library
**Research Findings**:
- FastEmbed uses ONNX Runtime + HuggingFace tokenizers under the hood
- Uses `rayon` for internal parallelism
- `TextEmbedding` implements `Send` and `Sync` (thread-safe)
- **BUT**: Tokenizers library has known concurrency issues in Python ecosystem
- Multiple threads calling `embed()` concurrently can cause race conditions

### The Real Problem
**Multiple `spawn_blocking` tasks run CONCURRENTLY on different threads**:
1. Oracle spawns 9 blocking tasks (one per file)
2. All 9 tasks try to call `model.embed()` simultaneously
3. This creates concurrent access to FastEmbed's internal tokenizer/ONNX runtime
4. **Result**: Race condition → memory corruption → segmentation fault

---

## 🛠️ Solutions Attempted

### Attempt 1: Arc::clone() + RwLock (FAILED)
**Code Pattern**:
```rust
Arc::clone(&state_guard.embedding_model)
let mut model_guard = model_arc.write()?;
let model_ref = model_guard.as_mut()?;
model_ref.embed(...)
```

**Result**: Initial implementation caused segfault, improved with lock ordering fixes but still segfaulted

### Attempt 2: std::sync::Mutex Serialization (CURRENT - STILL FAILING)
**Changes Made**:
1. **`src/state.rs`**:
   ```rust
   // Changed from:
   pub embedding_model: Arc<RwLock<Option<TextEmbedding>>>
   // To:
   pub embedding_model: Arc<std::sync::Mutex<Option<TextEmbedding>>>
   ```

2. **`src/oracle.rs`** - Updated 3 functions:
   - `perform_vector_search()` (line 253-265)
   - `generate_file_embedding()` (line 292-312)
   - `generate_embedding_for_content()` (line 430-448)
   - `init_embedding_model()` (line 175-202)
   - `run_task()` readiness check (line 70)

**Code Pattern**:
```rust
tokio::task::spawn_blocking(move || {
    let state_guard = state.read()?;
    let mut model_lock = state_guard.embedding_model.lock()?;
    let model_ref = model_lock.as_mut()?;
    model_ref.embed(vec![content.as_str()], None)  // ← STILL SEGFAULTS HERE
})
```

**Result**: **STILL SEGFAULTING** - Mutex serializes access but doesn't prevent the issue

### Lock Ordering Fix (Applied in Attempt 1, Retained in Attempt 2)
**Problem**: Inner RwLock held while outer RwLock acquired = deadlock
**Solution**: Release outer lock before acquiring inner lock
```rust
let model_arc = {
    let state_guard = state.read()?;
    Arc::clone(&state_guard.embedding_model)
};
// state_guard dropped, outer lock released
let mut model_guard = model_arc.write()?;
// Now safely acquire inner lock
```

---

## 🎯 Remaining Solutions to Try

### Option 1: Dedicated Single-Threaded Actor ⭐ RECOMMENDED
**Approach**: Single owning thread handles ALL embedding requests
```rust
pub struct EmbeddingActor {
    sender: mpsc::Sender<EmbedRequest>,
    handle: JoinHandle<()>
}

enum EmbedRequest {
    Query { text: String, respond_to: oneshot::Sender<Vec<f32>> },
    File { content: String, respond_to: oneshot::Sender<Vec<f32>> }
}

// Oracle sends requests via channel
// Actor owns the TextEmbedding and processes serially
```

**Pros**: Eliminates concurrency issues entirely
**Cons**: Slower (serial processing), more complex architecture

### Option 2: Multiple Model Instances (Pool)
**Approach**: Create N model instances, assign tasks round-robin
```rust
pub struct ModelPool {
    models: Vec<TextEmbedding>,
    current: AtomicUsize
}
```

**Pros**: Some parallelism
**Cons**: Memory intensive (N×384×model_size), complex pool management

### Option 3: Batch Processing
**Approach**: Queue embeddings, process in batches
```rust
// Oracle collects all files to index
// Spawns ONE blocking task with BATCH of files
// Model processes all embeddings in one call
```

**Pros**: Minimal architecture change
**Cons**: Requires refactoring indexing pipeline

### Option 4: Async Model Alternative
**Research async embedding libraries**:
- ` candle ` - May have better async support
- ` tokenizers ` - Direct async usage
- ` llm-chain ` - Alternative embedding approach

---

## 📁 Current Code State

### Files Modified
1. **`src/state.rs`** - Changed embedding_model type to `Arc<Mutex<Option<TextEmbedding>>>`
2. **`src/oracle.rs`** - Updated all embedding functions (5 locations)

### Build Status
```
$ cargo build
✓ Compiles successfully
⚠  15 warnings (cosmetic - unused imports, variables)
```

### Test Status
```
$ sudo RUST_LOG=debug cargo run /tmp/magicfs /tmp/magicfs-test-files
✓ Model loads successfully (BAAI/bge-small-en-v1.5, 384 dims)
✓ FUSE mounts successfully
✓ All files discovered (9 test files)
✓ Indexing begins...
✗ SEGFAULT during model.embed() call
```

### Test Files Created
Location: `/tmp/magicfs-test-files/`
- `python_script.py` - Python data analysis script
- `document.txt` - Database systems overview
- `notes.txt` - AI/ML notes
- `config.json` - Configuration settings
- `config.yaml` - YAML config file
- `code_sample.java` - Java code sample
- `shell_script.sh` - Shell script
- `report.txt` - Performance report
- `readme.md` - Testing guide

---

## 🔍 Debugging Information

### Last Successful Output
```
2025-12-28T05:33:25.069615Z  INFO magicfs::oracle: [Oracle] Indexing file: /tmp/magicfs-test-files/shell_script.sh
2025-12-28T05:33:25.069703Z  INFO magicfs::oracle: [Oracle] Indexing file: /tmp/magicfs-test-files/config.json
[1]    134360 segmentation fault
```

### GDB Backtrace (if available)
Run: `gdb -ex "run" -ex "bt" --args cargo run ...` (requires debug symbols)

### Environment
- OS: Linux 6.12.63-1-lts
- Rust: (current toolchain)
- FastEmbed: 5.5.0
- Tokio: 1.48.0

---

## 📝 Next Steps for Next Person

### Immediate Actions
1. **DO NOT** repeat the Arc::clone() or Mutex approaches (already proven not to work)
2. **Research** FastEmbed concurrency limitations
3. **Implement Option 1** (Dedicated Actor) or **Option 3** (Batch Processing)

### Recommended Priority Order
1. **Option 3: Batch Processing** - Easiest to implement, minimal architecture change
2. **Option 1: Dedicated Actor** - Most robust long-term solution
3. **Option 2: Model Pool** - If batch processing insufficient
4. **Option 4: Async Alternative** - Last resort, major refactor

### Commands to Verify Fix
```bash
# Clean start
sudo rm -rf /tmp/.magicfs
sudo RUST_LOG=debug cargo run /tmp/magicfs /tmp/magicfs-test-files

# Wait 10 seconds for indexing
# Should see: "Inserted embedding for file_id: X" (9 times)

# Test search
ls /tmp/magicfs/search/python
ls /tmp/magicfs/search/data
cat /tmp/magicfs/search/python/*

# Verify database
sudo sqlite3 /tmp/.magicfs/index.db "SELECT COUNT(*) FROM file_registry;"  # Should be 9
sudo sqlite3 /tmp/.magicfs/index.db "SELECT COUNT(*) FROM vec_index;"      # Should be 9
```

### Success Criteria
- [ ] No segmentation fault during file indexing
- [ ] All 9 test files indexed successfully
- [ ] Embeddings stored in vec_index table
- [ ] Semantic search returns results (e.g., `ls /tmp/magicfs/search/python` shows results)
- [ ] Model persists across indexing (no "Model not ready" spam)

---

## 📚 References

### FastEmbed Documentation
- https://docs.rs/fastembed/5.5.0/fastembed/
- GitHub: https://github.com/PascalJedi/fastembed-rs

### Research Links
- **Thread Safety**: FastEmbed uses ONNX Runtime + tokenizers (FFI boundaries)
- **Concurrency**: Multiple spawn_blocking tasks = multiple threads = race conditions
- **Best Practice**: CPU-bound libraries should use dedicated threads/actors

### Previous Session Handoffs
- `SESSION_HANDOFF_2025-12-28_INODE_AND_SEARCH_FIXES.md` - Original bug context
- `CLAUDE.md` - Full project architecture and status

---

## 🤝 Questions for Next Session

1. **Should we abandon FastEmbed** for a more async-friendly embedding library?
2. **Is the actor model** (Option 1) worth the architectural complexity?
3. **Can we batch all embeddings** in a single spawn_blocking call?
4. **Should we create a model pool** for controlled parallelism?

---

**Status**: 🔴 CRITICAL - System non-functional due to segfault
**Priority**: 🚨 URGENT - Blocks all semantic search functionality
**Owner**: Next available engineer
**Estimated Time**: 4-8 hours (depending on chosen solution)

Good luck! 🍀
