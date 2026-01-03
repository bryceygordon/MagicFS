This is a very smart architectural discussion. You are touching on the concept of **"Modes of Operation."**

Most software treats "Startup/Bulk Import" differently from "Steady State/Listening."

* **War Mode (Startup):** Throughput is king. We don't care about latency (who cares if a query takes 200ms if the index isn't ready?). We accept slight risks (data loss on power cut just means we restart the index).
* **Peace Mode (Steady State):** Latency is king. We need instant responsiveness. Data integrity is paramount (don't lose the user's latest save).

Here is the new document outlining this strategy.

### 📄 New File: `PERFORMANCE.md`

```markdown
# ⚡ MagicFS Performance Strategy

> "Throughput during the storm, Latency during the calm."

This document outlines optimization strategies to handle the "Initial Build" (War Mode) vs "Incremental Updates" (Peace Mode).

## 1. The Low-Hanging Fruit: Per-File Batching (Safe & Universal)
**Status:** 🔲 Not Implemented
**Impact:** High (CPU & IO)
**Risk:** None

Currently, `Indexer` processes a file with 50 chunks as 50 separate events:
1.  Send Chunk 1 to AI -> Wait.
2.  Open DB Transaction -> Insert Chunk 1 -> Commit -> Wait.
3.  Repeat x50.

**The Fix:**
* **Batch Embedding:** Send `Vec<String>` (all 50 chunks) to the Embedding Actor in one call. `FastEmbed` and ONNX runtimes are optimized for batch inferences.
* **Batch Write:** Open **one** transaction. Insert all 50 vectors. Commit once.

**Why it applies everywhere:**
Whether starting up or editing a file, this reduces overhead by orders of magnitude. It also ensures "Atomic Visibility"—a file appears in search results fully formed, not chunk-by-chunk.

---

## 2. "War Mode": SQLite Tuning (Startup Only)
**Status:** 🔲 Not Implemented
**Impact:** Massive Write Speedup
**Risk:** Moderate (Corruption on power loss)

SQLite is paranoid by default. It waits for the physical disk to confirm a write. During an initial index of 10,000 files, this wait time accumulates to minutes.

**The Fix:**
* **Detect Startup:** The `Librarian` knows it is performing the initial scan.
* **Engage War Mode:** Run `PRAGMA synchronous = OFF;` and `PRAGMA journal_mode = MEMORY;`.
* **The Trade-off:** If the power cuts, the DB is corrupt. But since we are building the index from scratch anyway, *we don't care*. We just rebuild on reboot.
* **Disengage:** Once the initial scan queue is empty, switch back to `PRAGMA synchronous = NORMAL;` (WAL).

---

## 3. The Embedding Pipeline (Architecture)
**Status:** 🔲 Not Implemented
**Impact:** Maximize CPU/GPU usage
**Risk:** Medium (Complexity)

Currently, the `Indexer` waits for the AI model. The AI model waits for the DB.

**The Fix:**
* **Decouple:** Worker A reads/chunks files and pushes to an `EmbeddingQueue`.
* **Saturate:** The Embedding Actor pulls batches from the queue constantly.
* **Flush:** A generic `DbWriter` pulls finished vectors and blasts them to disk.

---

## 4. Hardware Acceleration
**Status:** 🟡 Partially Active (Depends on User Hardware)
**Impact:** 2x-5x speedup

* **Quantization:** Ensure `InitOptions` in `oracle.rs` explicitly requests quantized models if available (usually default in FastEmbed, but worth verifying).
* **M2/M3 Metal & CUDA:** FastEmbed supports ONNX execution providers. We can expose a config flag to enable GPU acceleration.

```

### Discussion: Do we need special startup logic?

**Yes.**

To safely implement Item 2 ("War Mode" SQLite settings), the `Librarian` needs a state machine.

**Current Flow:**

1. Start Watcher.
2. Scan Dirs -> Add to Queue.
3. Process Queue.

**Proposed "War Mode" Flow:**

1. **Phase 1 (Boot):**
* Set global flag `SystemState::Indexing`.
* Set SQLite to `synchronous = OFF`.
* Librarian performs full directory scan. Populates `files_to_index` with 5,000 files.


2. **Phase 2 (The Crunch):**
* Oracle sees `files_to_index > 0`.
* Process efficiently.


3. **Phase 3 (The Handover):**
* Librarian sees `files_to_index` hits 0.
* Set global flag `SystemState::Monitoring`.
* **Execute:** `PRAGMA synchronous = NORMAL;` (Commit the index to safety).
* **Force Checkpoint:** `PRAGMA wal_checkpoint(TRUNCATE);` (Flush WAL to main DB file).



### What should we do first?

I recommend we implement **Optimization #1 (Per-File Batching)** immediately.

* **Why?** It flows through the rest of the project (as you suspected). It makes *everything* faster and safer (Atomic Visibility).
* **How?**
1. Modify `src/engine/mod.rs` to accept `Vec<String>` for embeddings.
2. Modify `src/oracle.rs` (Actor) to process a vector of strings.
3. Modify `src/storage/repository.rs` to accept `insert_batch`.
4. Update `Indexer` to glue it together.



This doesn't require complex state machines yet, but it prepares the engine to handle the load when we *do* turn on "War Mode".

Shall we proceed with **Per-File Batching**?
