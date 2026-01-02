# 📜 The MagicFS Charter & Manual

> "The filesystem is the interface."

MagicFS is a system primitive that turns **Chaos into API**. It aims to make semantic understanding of data—regardless of format—as native to the OS as `ls` or `cp`.

---

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

### 📄 The Universal Text Interface
**Everything is text.**
To the user, a PDF, a DOCX, and a JPG with text in it are just "information."
* **Rule:** MagicFS abstracts away file formats. If it contains words, it must be searchable via standard text tools.

---

## 2. Architecture: Service-Oriented

MagicFS uses a single-process architecture composed of strictly isolated services:

```text
┌───────────────────────────── Process Boundary ──────────────────────────────┐
│                                                                             │
│  ┌───────────────┐        ┌─────────────────┐        ┌───────────────────────┐  │
│  │ Hollow Drive  │◄────►│   Inode Store   │◄─────┤       Orchestrator    │  │
│  │ (FUSE Interface)      │ (Shared State)  │        │        (Oracle.rs)    │  │
│  └───────┬───────┘        └─────────────────┘        └───────────┬───────────┘  │
│          │ Signal:                                               │              │
│          │ .magic/refresh ──────────────────┐           ┌────────▼────────┐     │
│          │                                  │           │      Engine     │     │
│       Syscalls                              │           │ (Async Workers) │     │
│                                             │           └─┬─────────────┬─┘     │
│                                             │             │             │       │
│                                             │    ┌────────▼────────┐    │       │
│                                             │    │     Indexer     │    │Searcher│
│                                    ┌────────▼────┴───┐(Retry Logic)│    │       │
│                                    │    Librarian    │└────┬────────┴───┬───┘   │
│                                    │(Debounce/Watch) │     │            │       │
│                                    └────────┬────────┘     │            │       │
│                                             │              │            │       │
│                                    ┌─────▼─────────────▼────────┴────────────┐  │
│                                    │               Repository                │  │
│                                    │           (SQLite + Vector)             │  │
│                                    └─────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────┘
```

### The Organs
1.  **Hollow Drive (`src/hollow_drive.rs`)**: The dumb FUSE terminal. Never blocks. Checks `InodeStore`.
2.  **Inode Store (`src/core/inode_store.rs`)**: The In-Memory source of truth for "Virtual Files".
3.  **The Orchestrator (`src/oracle.rs`)**: Async event loop. Handles "Lockout/Tagout" safety to prevent race conditions.
4.  **The Engine**:
    * **Indexer**: Extraction, Chunking, Embedding.
    * **Searcher**: Query Embedding, DB Lookup.
5.  **The Librarian (`src/librarian.rs`)**: The background watcher. Handles thermal protection (debouncing) and manual overrides.

---

## 3. Maintenance & Testing

### 🔗 Version Control
* **Commit Frequently**: Small, atomic commits.
* **Sync**: Pull before starting work.

### 🧪 The Golden Rule
`tests/run_suite.sh` must pass before any merge.

```bash
# Run the full integration suite
tests/run_suite.sh

# Run specific unit tests
cargo test
```

### 📂 Key Test Cases
| Test | Purpose |
|------|---------|
| `test_00_stress` | Startup Storm & Zombie Check |
| `test_01_indexing` | Dynamic Indexing |
| `test_03_search` | End-to-End Search |
| `test_04_hardening` | Binary/Large file rejection |
| `test_05_chunking` | Semantic Dilution & Thresholds |
| `test_07_real_world` | Race Conditions & Permissions |
| `test_09_chatter` | Thermal Protection (Debounce) |
| `test_10_refresh` | Manual Override (`touch .magic/refresh`) |
| `test_14_mirror` | Mirror Mode Navigation |

---
*Adopted: Jan 2026*
*Version: 3.0 (The Unified Standard)*
