# Session Handover - Jan 1, 2026 (Session 2)

## 🛑 Current Status
- **System**: Stable. All tests passing (`tests/run_suite.sh`).
- **Features**: Read/Write/Search/Mirror/Multi-Root are LIVE.
- **Environment**: `./dev.sh` mounts `~/me` and `~/sync/vault`.

## 🧠 Context: The "Magic" Deficit
While the *plumbing* is perfect, the *intelligence* needs work.
1.  **Semantic Drift**: Searching "Beef" ranks "Chicken" too high (Model lacks nuance).
2.  **Title Blindness**: Filenames aren't weighted heavily enough.

## 🎯 Next Session Goals
**Theme: "Making it Smart"**

1.  **Upgrade AI Model**:
    * Switch from `BGESmallENV15` (Tiny) to **`BGE-M3`** or **`Nomic-Embed`** (State of the Art).
    * *Task*: Update `oracle.rs` and `Cargo.toml`.

2.  **Context Injection**:
    * Modify `src/storage/text_extraction.rs` or `indexer.rs`.
    * *Logic*: Prepend `File: {filename}\n` to every chunk before embedding.

3.  **Hybrid Scoring**:
    * Modify `src/storage/repository.rs`.
    * *Logic*: `SELECT ..., (1.0 - distance) + (CASE WHEN path LIKE %q% THEN 0.2 ELSE 0 END) as score`.

## 📝 Commands
- **Start**: `./dev.sh` (Auto-cleans DB, builds, mounts with sudo).
- **Test Suite**: `tests/run_suite.sh`
