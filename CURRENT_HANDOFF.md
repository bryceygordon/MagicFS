Here is the comprehensive handover file. The project is in a much stronger state than when we started—the logic is solid, and we have isolated the final flakiness to a classic "read-after-write" race condition.

---

# 🔄 MagicFS Handoff: The "Empty Read" Race

**Date**: 2025-12-28 22:35 AWST
**Status**: ⚠️ **2/3 Tests Passing** (Indexing ✅, Ignore Rules ✅, Search ❌)

## 🏆 Key Achievements (Session Fixes)

1. **Fixed "Silent Watcher"**: Reverted `fs::canonicalize` in `src/librarian.rs`. The watcher now correctly tracks the test directory without path resolution mismatches.
2. **Fixed "Ignore Rule" Race**: Implemented **Two-Pass Batch Processing** in `src/librarian.rs`. The Librarian now prioritizes processing `.magicfsignore` updates *before* processing file events in the same batch. `test_02_dotfiles.py` now passes consistently!

## 🚨 Current Failure: "The Empty Read"

`test_03_search.py` is failing with a timeout because the Oracle refuses to index the file `projects/ai.rs`.

**The Log Evidence:**

```text
DEBUG magicfs::librarian: [Librarian] RAW EVENT: Event { kind: Create(File), paths: [".../ai.rs"] ... }
DEBUG magicfs::librarian: [Librarian] RAW EVENT: Event { kind: Modify(Data(Any)), paths: [".../ai.rs"] ... }
INFO  magicfs::librarian: [Librarian] Queuing file for index: .../ai.rs
INFO  magicfs::oracle: [Oracle] Indexing file: .../ai.rs
WARN  magicfs::oracle: [Oracle] File has no text content: .../ai.rs  <-- HERE IS THE BUG

```

**Diagnosis:**
The Oracle is too fast. It picks up the file from the queue and attempts to read it immediately after the `notify` event.

* **Scenario:** Python script writes file -> OS triggers Event -> Librarian queues file -> Oracle opens file.
* **Result:** The file handle might still be flushing, or the read operation sees 0 bytes because the write hasn't fully committed to the filesystem view the Oracle sees.

## 🛠️ Next Steps for New Session

**Goal**: Ensure `text_extraction` reliably reads content.

**Proposed Solution**:
Modify `src/oracle.rs` -> `index_file` function.

1. **Add Retry Logic**: If `extract_text_from_file` returns empty string, but `fs::metadata` says size > 0, wait 50ms and try again (up to 3 times).
2. **Verify Metadata**: Check `file_len` before reading.

**Code Location to Fix (`src/oracle.rs`):**

```rust
// Current Code:
let text_content = tokio::task::spawn_blocking(move || {
    crate::storage::extract_text_from_file(...)
}).await...;

if text_content.trim().is_empty() {
    // BUG: It gives up immediately!
    tracing::warn!("[Oracle] File has no text content: {}", file_path);
    return Ok(());
}

```

## 📂 Critical Files State

* **`src/librarian.rs`**: **STABLE**. Robust two-pass logic is working perfectly. Do not touch unless necessary.
* **`src/oracle.rs`**: **UNSTABLE**. Needs the read-retry logic described above.
* **`tests/run_suite.sh`**: **STABLE**. Good for running the loop.

## 🧪 How to Resume

1. **Verify Failure**: Run `./tests/run_suite.sh` to see `test_03` fail with the "File has no text content" warning.
2. **Implement Fix**: Add the retry loop to `src/oracle.rs`.
3. **Verify Success**: Run the suite again. All 3 tests should pass.
