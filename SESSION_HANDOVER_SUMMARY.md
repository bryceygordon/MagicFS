# 🛑 Session Handover: MagicFS Phase 6 (Chunking Refactor)

**Date**: 2025-12-29
**Status**: Critical Bug in Search Result Delivery
**Last Action**: Run `test_05_chunking.py` -> FAILED

## 📍 Where We Are
We are in **Phase 6: Hardening**. We successfully refactored the system from "One-File-One-Vector" to **"Sliding Window Chunking"** to solve the Semantic Dilution problem.

### ✅ What Works
1.  **Database Schema**: `vec_index` now supports multiple chunks per file (`file_id` is an auxiliary column).
2.  **Indexing Pipeline**: 
    * `text_extraction` correctly safeguards against binary files and large files (>10MB).
    * `text_extraction` correctly splits text into overlapping chunks.
    * `Oracle` correctly loops through chunks and inserts them into `sqlite-vec`.
3.  **Search Logic**:
    * The new **Aggregation Query** (Subquery + Group By) works.
    * **Logs confirm**: `[Oracle] Search returned 5 aggregated results`.

## 🐛 The Bug: "The Missing Handoff"
**Symptom**: 
`test_05_chunking.py` correctly indexes the file (`needle.txt` -> 5 chunks). It then queries "nuclear launch code". The Oracle runs the search, finds 5 results, but the Python test script (accessing via FUSE) sees an empty directory or `ENOENT`, eventually timing out.

**Evidence (Log Analysis)**:
1.  **Data Exists**: `[Oracle] Total chunks in vec_index before search: 6`
2.  **Query Works**: `[Oracle] Search returned 5 aggregated results`
3.  **FUSE Loops**: `[HollowDrive] lookup: parent=3, name="nuclear launch code"` repeats 20+ times.
4.  **Failure**: The FUSE layer never seems to "see" the results that the Oracle produced.

## 🕵️ Hypotheses for Next Session
The disconnect is between **Oracle** finishing the search and **HollowDrive** serving the directory.

1.  **Inode Mismatch**: 
    * `HollowDrive` generates a dynamic inode for the search query (hash of string).
    * `Oracle` retrieves the inode from `active_searches`.
    * *Check*: Is the `inode` the Oracle inserts into `GlobalState.search_results` the exact same `inode` the FUSE `readdir` is looking up?

2.  **DashMap Visibility**:
    * `Oracle` inserts into `search_results` (DashMap).
    * `HollowDrive` reads from `search_results`.
    * *Check*: Is `HollowDrive` checking `active_searches` before checking `search_results` and returning `EAGAIN` prematurely?

3.  **FUSE Cache/TTL**:
    * `HollowDrive` returns `TTL=1s`. 
    * If it returned `ENOENT` or an empty directory *once* before the Oracle finished, the kernel might be caching that negative result, causing the loop.

## 🛠️ Next Steps for New Chat
1.  **Instrument `HollowDrive`**: Add debug logs to `src/hollow_drive.rs` in `lookup` and `readdir` to print *exactly* which inode it is looking for and what it finds in `GlobalState.search_results`.
2.  **Verify Inode Integrity**: Log the inode ID in `Oracle::process_search_query` just before insertion.
3.  **Run Test**: `./tests/run_suite.sh` and compare the Inode IDs.

## 📂 Key Files Modified in This Session
- `src/storage/text_extraction.rs`: Added chunking & binary guards.
- `src/storage/connection.rs`: Updated schema for chunks.
- `src/storage/vec_index.rs`: Updated insert/delete logic.
- `src/oracle.rs`: Updated indexing loop and search aggregation query.
