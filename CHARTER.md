================================================
FILE: CHARTER.md
================================================
# 📜 The MagicFS Charter

> "The filesystem is the interface."

MagicFS is a system primitive that turns **Chaos into API**. It aims to make semantic understanding of data—regardless of format—as native to the OS as `ls` or `cp`.

## 1. The Prime Directives

### ⏱️ The 10ms Law
**Latency is the enemy.**
MagicFS exposes itself via FUSE. If it blocks, the OS freezes.
* **Rule:** The FUSE thread (`fs/`) never blocks. It reads from memory (`InodeStore`) or returns `EAGAIN`.

### 🛡️ Fail Safe, Fail Small
**The filesystem is hostile.**
Users have 100GB logs, corrupted PDFs, and deep symlinks.
* **Rule:** A failure to process a single file must **never** crash the daemon.
* **Implementation:** Skip bad files, log the error, and move on. "Partial results are better than no filesystem."

## 2. Architectural Philosophy

### 📄 The Universal Text Interface
**Everything is text.**
To the user, a PDF, a DOCX, and a JPG with text in it are just "information."
* **Rule:** MagicFS abstracts away file formats. If it contains words, it must be searchable and retrievable via standard text tools.

### 🧱 Service Isolation (The "Organs")
1. **FS Layer**: The "Face". Dumb and fast.
2. **Engine**: The "Brain". Async, heavy, handles embeddings and parsing.
3. **Repository**: The "Memory". SQLite + Vector Store.

## 3. The User Experience

### 🪄 Zero Config
* No manual re-indexing commands.
* No complex YAML config files for standard usage.

### 🗃️ Composability
**The output is the input.**
MagicFS results are standard files. They must be compatible with `cp`, `grep`, `zip`, and scripts.

## 4. Maintenance
* **Test Driven:** `tests/run_suite.sh` matches the golden rule.
* **Refactoring:** We refactor in micro-steps.
* **Unabridged Output:** All code outputs must be complete and unabridged to ensure context is never lost.

---
*Adopted: Dec 2025*
*Version: 2.1 (The Universal Pivot)*
