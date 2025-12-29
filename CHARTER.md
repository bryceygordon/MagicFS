FILE: CHARTER.md

# 📜 The MagicFS Charter

> "The filesystem is the interface."

MagicFS is not just a search tool; it is a system primitive. It aims to make semantic understanding of data as native to the OS as `ls` or `cat`. This document outlines the non-negotiable principles that guide development, refactoring, and contribution.

## 1. The Prime Directives

### ⏱️ The 10ms Law

**Latency is the enemy.**
MagicFS exposes itself to the kernel via FUSE. If a filesystem operation blocks for more than a few milliseconds, the entire user interface freezes.

* **Rule:** The FUSE thread (`fs/`) must **never** perform blocking I/O, SQL queries, or model inference.
* **Implementation:** It must strictly read from memory (`InodeStore`) or return `EAGAIN`.

### 🛡️ Fail Safe, Fail Small

**The filesystem is hostile.**
Users have binary files, 100GB logs, corrupted encoding, and deep symlink cycles. MagicFS must survive them all.

* **Rule:** A failure to process a single file must **never** crash the daemon or unmount the filesystem.
* **Implementation:** Skip bad files, log the error, and move on. "Partial results are better than no filesystem."

## 2. Architectural Philosophy

### 🧱 Service Isolation (The "Organs")

The system is composed of distinct, isolated services. They communicate via strictly typed interfaces, not shared global hacks.

1. **The FS Layer (`fs/`)**: The "Face". Dumb, fast, and synchronous. It strictly reads state; it never mutates business logic.
2. **The Engine (`engine/`)**: The "Brain". Async and heavy. Handles the complexity of embeddings and vector search.
3. **The Watcher (`watcher/`)**: The "Eyes". Feeds the Engine with file events.
4. **The Repository (`storage/`)**: The "Memory". The **only** place where SQL or raw bytes are touched.

### 💉 Strong Typing Over Stringly Typing

* **Bad:** Passing `inode: u64` and `query: String` loosely between functions.
* **Good:** Using `FileId`, `SearchQuery`, and `Inode` wrapper types to prevent mixing up data.
* **Rule:** Logic boundaries must be enforced by the type system, not just convention.

## 3. The User Experience

### 🪄 Zero Config

MagicFS should "just work."

* No complex YAML config files for standard usage.
* No manual re-indexing commands (the Watcher handles it).
* Standard tools (`ls`, `grep`, `cp`) are the UI. We do not build custom CLI tools if a standard file operation can achieve the goal.

### 🔍 Truthful Representation

The filesystem should not lie.

* If a file is returned in a search, it must exist.
* If a score is presented (e.g., `0.95_file.txt`), it must be accurate to the underlying vector distance.

## 4. Maintenance & Contribution

### 🧪 Test Driven Stability

* **The Golden Rule:** `tests/run_suite.sh` must pass before any merge.
* **Refactoring:** We refactor in micro-steps. We never leave the system in a broken state between commits.
* **Dependencies:** We are skeptical of heavy dependencies. Each crate added must justify its weight.

---

*Adopted: Dec 2025*
*Version: 1.0*

================================================



